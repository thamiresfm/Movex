use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::core::state::{ActiveScreen, ConnectionStatus, SharedState};
use crate::input::inject::inject_event;
use crate::network::protocol::{Message, PROTOCOL_VERSION};
use crate::network::reconnect::ReconnectPolicy;
use crate::network::transport::{create_tls_connector, recv_message, send_message};

/// Conecta ao servidor com reconexão automática e suporte a cancelamento
pub async fn connect(state: SharedState, cancel: CancellationToken) {
    let policy = ReconnectPolicy::default();

    loop {
        let (server_addr, port) = {
            let s = state.settings.lock().await;
            (
                s.server_addr.clone().unwrap_or_else(|| "127.0.0.1".to_string()),
                s.port,
            )
        };

        let addr = format!("{}:{}", server_addr, port);
        info!("Conectando ao servidor Movex em {}...", addr);
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Connecting;
        }

        let connect_result = tokio::select! {
            _ = cancel.cancelled() => {
                info!("Cliente Movex cancelado durante conexão");
                return;
            }
            r = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                TcpStream::connect(&addr)
            ) => r
        };

        match connect_result {
            Ok(Ok(tcp)) => {
                let connector = create_tls_connector();
                let domain = ServerName::try_from("movex.local").expect("domínio inválido");

                match connector.connect(domain, tcp).await {
                    Ok(mut tls) => {
                        let (hostname, psk_hex) = {
                            let s = state.settings.lock().await;
                            (s.hostname.clone(), s.psk_hex.clone())
                        };

                        // 1. Receber ServerChallenge com nonce
                        let challenge = match recv_message(&mut tls).await {
                            Ok(Message::ServerChallenge { version, server_nonce, .. }) => {
                                if version != PROTOCOL_VERSION {
                                    warn!("Versão incompatível do servidor");
                                    continue;
                                }
                                server_nonce
                            }
                            Ok(_) => { warn!("Esperava ServerChallenge"); continue; }
                            Err(e) => { warn!("Erro ao receber ServerChallenge: {}", e); continue; }
                        };

                        // 2. Computar HMAC(psk, server_nonce) e enviar Hello
                        let hmac = crate::core::auth::compute_hmac(&psk_hex, &challenge);
                        let hello = Message::Hello {
                            version: PROTOCOL_VERSION,
                            hostname,
                            hmac,
                        };

                        if let Err(e) = send_message(&mut tls, &hello).await {
                            warn!("Erro ao enviar Hello: {}", e);
                            continue;
                        }

                        // Pode receber ConnectionPending antes do HelloAck
                        let first_msg = recv_message(&mut tls).await;

                        // Tratar ConnectionPending — aguardar aprovação
                        let first_msg = match first_msg {
                            Ok(Message::ConnectionPending { hostname: server_name }) => {
                                info!("Aguardando aprovação do servidor '{}'...", server_name);
                                {
                                    let mut status = state.connection_status.lock().await;
                                    *status = ConnectionStatus::Connecting;
                                }
                                // Aguardar próxima mensagem (aprovado ou rejeitado)
                                recv_message(&mut tls).await
                            }
                            other => other,
                        };

                        match first_msg {
                            Ok(Message::ConnectionRejected { reason }) => {
                                warn!("Conexão rejeitada pelo servidor: {}", reason);
                                {
                                    let mut status = state.connection_status.lock().await;
                                    *status = ConnectionStatus::Disconnected;
                                }
                                // Não tentar reconectar — foi rejeitado explicitamente
                                return;
                            }
                            Ok(Message::ConnectionApproved) => {
                                // Aprovado — agora vem o HelloAck
                                info!("Conexão aprovada pelo servidor!");
                            }
                            // Pode ser que o servidor envie HelloAck diretamente (sem aprovação)
                            _ => {
                                // Tratar abaixo como HelloAck
                            }
                        }

                        // Agora receber HelloAck
                        match recv_message(&mut tls).await {
                            Ok(Message::HelloAck { hostname: peer, .. }) => {
                                info!("Conectado ao servidor: {}", peer);
                                policy.reset();
                                {
                                    let mut status = state.connection_status.lock().await;
                                    *status = ConnectionStatus::Connected {
                                        peer_hostname: peer,
                                        latency_ms: 0,
                                    };
                                    let mut started = state.session_started_at.lock().await;
                                    *started = Some(std::time::Instant::now());
                                }

                                // Canal para enviar mensagens ao servidor (ex: clipboard)
                                let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
                                { *state.message_tx.lock().await = Some(msg_tx); }

                                run_session(&mut tls, state.clone(), &mut msg_rx, cancel.clone()).await;

                                { *state.message_tx.lock().await = None; }
                            }
                            Ok(Message::HelloReject { reason }) => {
                                warn!("Rejeitado pelo servidor: {}", reason);
                            }
                            Ok(_) => warn!("Resposta inesperada ao Hello"),
                            Err(e) => warn!("Erro ao receber HelloAck: {}", e),
                        }
                    }
                    Err(e) => warn!("Falha no TLS handshake: {}", e),
                }
            }
            Ok(Err(e)) => warn!("Falha ao conectar em {}: {}", addr, e),
            Err(_)     => warn!("Timeout ao conectar em {}", addr),
        }

        // Verificar cancelamento antes de aguardar retry
        if cancel.is_cancelled() { return; }

        let attempt = policy.attempt();
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Reconnecting { attempt };
            let mut started = state.session_started_at.lock().await;
            *started = None;
        }
        let wait = policy.next_delay();
        info!("Reconectando em {}s...", wait.as_secs());

        tokio::select! {
            _ = cancel.cancelled() => { info!("Cliente cancelado durante espera de retry"); return; }
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

async fn run_session<S>(
    stream: &mut S,
    state: SharedState,
    msg_rx: &mut mpsc::Receiver<Message>,
    cancel: CancellationToken,
) where
    S: tokio::io::AsyncReadExt + tokio::io::AsyncWriteExt + Unpin,
{
    let mut last_clipboard: Option<String> = None;
    let mut clipboard_check = tokio::time::interval(std::time::Duration::from_millis(500));
    // Ping periódico para medir latência no lado cliente
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(3));
    let mut ping_sent_at: Option<std::time::Instant> = None;

    let mut file_receiver = match crate::transfer::FileReceiver::new().await {
        Ok(r) => Some(r),
        Err(e) => { warn!("FileReceiver indisponível: {}", e); None }
    };

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = send_message(stream, &Message::Disconnect {
                    reason: "cliente encerrado".into(),
                }).await;
                break;
            }

            // Enviar mensagens pendentes ao servidor (ex: clipboard)
            Some(msg) = msg_rx.recv() => {
                if let Err(e) = send_message(stream, &msg).await {
                    warn!("Erro ao enviar para servidor: {}", e);
                    break;
                }
            }

            // Ping periódico para medir latência
            _ = ping_interval.tick() => {
                ping_sent_at = Some(std::time::Instant::now());
                if send_message(stream, &Message::Ping).await.is_err() { break; }
            }

            // Verificar clipboard periodicamente — texto E imagens (se habilitado)
            _ = clipboard_check.tick() => {
                // Ler flag sem lock pesado (settings é verificado fora do hot-path)
                let sync_enabled = {
                    if let Ok(s) = state.settings.try_lock() {
                        s.clipboard_sync_enabled
                    } else {
                        false // respeitar preferência do usuário na dúvida — não enviar
                    }
                };
                if !sync_enabled { continue; }
                if let Some(msg) = crate::clipboard::sync::create_clipboard_message() {
                    // Hash CRC32 do conteúdo para detectar mudanças reais (não só tamanho)
                    let key = match &msg {
                        Message::ClipboardData { mime, data } => {
                            let hash = crate::core::utils::crc32(data);
                            format!("{}:{}", mime.split(';').next().unwrap_or(mime), hash)
                        }
                        _ => continue,
                    };
                    let changed = last_clipboard.as_ref().map_or(true, |prev| prev != &key);
                    if changed {
                        last_clipboard = Some(key);
                        if send_message(stream, &msg).await.is_err() { break; }
                    }
                }
            }

            // Receber mensagens do servidor
            result = recv_message(stream) => {
                match result {
                    Ok(Message::EnterScreen) => {
                        let mut active = state.active_screen.lock().await;
                        *active = ActiveScreen::Remote;
                        info!("Cursor entrou nesta máquina");
                        // A borda luminosa é gerenciada pelo frontend via set_screen_border
                        // Aqui apenas emitimos o estado para o frontend reagir
                    }
                    Ok(Message::LeaveScreen) => {
                        let mut active = state.active_screen.lock().await;
                        *active = ActiveScreen::Local;
                    }
                    Ok(Message::Input(event)) => {
                        inject_event(event);
                    }
                    Ok(ref msg @ Message::ClipboardData { .. }) => {
                        crate::clipboard::sync::apply_clipboard_message(msg);
                        // Atualizar cache com CRC32 (igual ao que o remetente calcula)
                        // Evita reenvio imediato após receber imagem (from_utf8 falhava para PNG)
                        if let Message::ClipboardData { ref mime, ref data } = *msg {
                            let hash = crate::core::utils::crc32(data);
                            last_clipboard = Some(format!("{}:{}", mime.split(';').next().unwrap_or(mime), hash));
                        }
                    }
                    Ok(Message::FileStart { id, name, size }) => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_start(id, name, size).await.unwrap_or_else(|e| warn!("FileStart: {}", e));
                        }
                    }
                    Ok(Message::FileChunk { id, seq, data }) => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_chunk(id, seq, data).await.unwrap_or_else(|e| warn!("FileChunk: {}", e));
                        }
                    }
                    Ok(Message::FileEnd { id, checksum }) => {
                        if let Some(ref mut recv) = file_receiver {
                            match recv.on_file_end(id, checksum).await {
                                Ok((name, path)) => info!("Arquivo recebido: '{}' → {:?}", name, path),
                                Err(e) => {
                                    warn!("FileEnd: {}", e);
                                    let _ = send_message(stream, &Message::FileRetry { id }).await;
                                }
                            }
                        }
                    }
                    Ok(Message::FileRetry { id }) => {
                        warn!("Peer solicitou reenvio do arquivo id={}", id);
                    }
                    Ok(Message::Ping) => {
                        let _ = send_message(stream, &Message::Pong).await;
                    }
                    Ok(Message::Pong) => {
                        // Medir latência RTT no cliente
                        if let Some(sent) = ping_sent_at.take() {
                            let rtt = sent.elapsed().as_millis() as u32;
                            let mut status = state.connection_status.lock().await;
                            if let ConnectionStatus::Connected { ref mut latency_ms, .. } = *status {
                                *latency_ms = rtt;
                            }
                        }
                    }
                    Ok(Message::Disconnect { reason }) => {
                        info!("Servidor desconectou: {}", reason);
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Erro na sessão: {}", e);
                        break;
                    }
                }
            }
        }
    }

    let mut status = state.connection_status.lock().await;
    *status = ConnectionStatus::Disconnected;
    let mut started = state.session_started_at.lock().await;
    *started = None;
}


