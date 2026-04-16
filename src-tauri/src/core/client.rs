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

                        // Enviar Hello com nonce que inclui derivação da PSK
                        let nonce = hex::encode(rand::random::<[u8; 16]>());
                        let hmac = compute_hmac(&psk_hex, &nonce);

                        // Reutilizar o campo nonce para enviar o HMAC (compatível com servidor)
                        let hello = Message::Hello {
                            version: PROTOCOL_VERSION,
                            hostname,
                            nonce: hmac,
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

            // Verificar clipboard periodicamente e sincronizar se mudou
            _ = clipboard_check.tick() => {
                if let Some(text) = crate::clipboard::sync::read_clipboard() {
                    let changed = last_clipboard.as_ref().map_or(true, |prev| prev != &text);
                    if changed {
                        last_clipboard = Some(text.clone());
                        let msg = Message::ClipboardData {
                            mime: "text/plain".to_string(),
                            data: text.into_bytes(),
                        };
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
                        if let Message::ClipboardData { ref data, .. } = *msg {
                            last_clipboard = String::from_utf8(data.clone()).ok();
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
                    Ok(Message::Pong) => {}
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

fn compute_hmac(psk_hex: &str, nonce: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(psk_hex.as_bytes());
    hasher.update(b":");
    hasher.update(nonce.as_bytes());
    hex::encode(hasher.finalize())
}
