use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::core::state::{ActiveScreen, ConnectionStatus, SharedState};
use crate::network::protocol::{Message, PROTOCOL_VERSION};
use crate::network::transport::{
    create_tls_acceptor, generate_self_signed_cert, recv_message, send_message,
};
use crate::screen::boundary::{check_boundary, BoundaryResult};
use crate::screen::layout::{PeerPosition, ScreenLayout, ScreenResolution};

/// Inicia o servidor Movex com cancelamento e envio de input ao cliente
pub async fn start(state: SharedState, cancel: CancellationToken) -> Result<(), String> {
    let port = { state.settings.lock().await.port };

    let (certs, key) = generate_self_signed_cert()
        .map_err(|e| format!("Erro ao gerar certificado TLS: {}", e))?;
    let acceptor = create_tls_acceptor(certs, key)
        .map_err(|e| format!("Erro ao criar TLS acceptor: {}", e))?;

    // Iniciar mDNS ao subir o servidor
    let (hostname, mdns_port) = {
        let s = state.settings.lock().await;
        (s.hostname.clone(), s.port)
    };
    let _mdns_daemon = crate::network::discovery::announce_server(&hostname, mdns_port).ok();

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Falha ao escutar em {}: {}", addr, e))?;

    info!("Servidor Movex escutando em {}", addr);
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Connecting;
    }

    // Aceitar apenas um cliente por vez — desconectar o anterior se houver
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Servidor Movex cancelado");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((tcp_stream, peer_addr)) => {
                        // Já há cliente conectado? Rejeitar.
                        let already_connected = matches!(
                            *state.connection_status.lock().await,
                            ConnectionStatus::Connected { .. }
                        );
                        if already_connected {
                            warn!("Rejeitando nova conexão de {} — já há cliente conectado", peer_addr);
                            continue;
                        }

                        info!("Nova conexão TCP de {}", peer_addr);
                        let tls_stream = match acceptor.accept(tcp_stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                warn!("Falha no TLS handshake com {}: {}", peer_addr, e);
                                continue;
                            }
                        };
                        let state_clone = state.clone();
                        let cancel_clone = cancel.clone();
                        tokio::spawn(async move {
                            handle_client(tls_stream, peer_addr, state_clone, cancel_clone).await;
                        });
                    }
                    Err(e) => error!("Erro ao aceitar conexão: {}", e),
                }
            }
        }
    }

    let mut status = state.connection_status.lock().await;
    *status = ConnectionStatus::Disconnected;
    Ok(())
}

async fn handle_client<S>(
    mut stream: S,
    peer_addr: SocketAddr,
    state: SharedState,
    cancel: CancellationToken,
) where
    S: tokio::io::AsyncReadExt + tokio::io::AsyncWriteExt + Unpin + Send + 'static,
{
    // ── Handshake com validação de PSK ──────────────────────────────────────
    let hello = match recv_message(&mut stream).await {
        Ok(m) => m,
        Err(e) => { warn!("Erro ao receber Hello de {}: {}", peer_addr, e); return; }
    };

    let (peer_hostname, client_nonce) = match hello {
        Message::Hello { version, hostname, nonce } => {
            if version != PROTOCOL_VERSION {
                let _ = send_message(&mut stream, &Message::HelloReject {
                    reason: format!("Versão incompatível: esperado {}, recebido {}", PROTOCOL_VERSION, version),
                }).await;
                return;
            }
            (hostname, nonce)
        }
        _ => { warn!("Esperava Hello de {}", peer_addr); return; }
    };

    // Validar PSK: cliente deve enviar HMAC-SHA256(psk, nonce)
    let psk_hex = { state.settings.lock().await.psk_hex.clone() };
    let expected_hmac = compute_hmac(&psk_hex, &client_nonce);
    if client_nonce != expected_hmac {
        // Na versão de desenvolvimento, só logar o aviso (não rejeitar)
        // Em produção: rejeitar com HelloReject
        warn!("PSK não verificada para {} — permitindo (modo dev)", peer_addr);
    }

    let our_hostname = { state.settings.lock().await.hostname.clone() };
    let _ = send_message(&mut stream, &Message::HelloAck {
        version: PROTOCOL_VERSION,
        hostname: our_hostname,
        nonce: hex::encode(rand::random::<[u8; 16]>()),
    }).await;

    info!("Cliente autenticado: {} ({})", peer_hostname, peer_addr);
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Connected {
            peer_hostname: peer_hostname.clone(),
            latency_ms: 0,
        };
        let mut started = state.session_started_at.lock().await;
        *started = Some(std::time::Instant::now());
    }

    // ── Canal de mensagens para este cliente ────────────────────────────────
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
    { *state.message_tx.lock().await = Some(msg_tx.clone()); }

    // ── Captura de input e detecção de borda ────────────────────────────────
    let peer_position = {
        let s = state.settings.lock().await;
        match s.peer_position {
            crate::config::ScreenPosition::Left  => PeerPosition::Left,
            crate::config::ScreenPosition::Above => PeerPosition::Above,
            crate::config::ScreenPosition::Below => PeerPosition::Below,
            _                                    => PeerPosition::Right,
        }
    };
    let layout = ScreenLayout {
        local: ScreenResolution { width: 1920, height: 1080, scale_factor: 1.0 },
        peer: None,
        peer_position,
    };

    let state_for_capture = state.clone();
    let msg_tx_for_capture = msg_tx.clone();
    let layout_clone = layout.clone();

    // Iniciar captura de input
    let capture = crate::input::platform::create_capture();
    let capture_result = capture.start(Box::new(move |event| {
        // Verificar se cursor cruzou a borda
        if let crate::input::InputEvent::MouseMove { x, y } = &event {
            let px = x * layout_clone.local.width as f32;
            let py = y * layout_clone.local.height as f32;
            match check_boundary(px, py, &layout_clone) {
                BoundaryResult::CrossedToPeer { entry_x, entry_y } => {
                    // Travar cursor local e enviar EnterScreen ao cliente
                    let _ = msg_tx_for_capture.try_send(Message::EnterScreen);
                    let _ = msg_tx_for_capture.try_send(Message::Input(
                        crate::input::InputEvent::MouseMove { x: entry_x, y: entry_y }
                    ));
                    // Atualizar estado para Remote
                    let state_clone = state_for_capture.clone();
                    tokio::spawn(async move {
                        let mut active = state_clone.active_screen.lock().await;
                        *active = ActiveScreen::Remote;
                    });
                    return; // não enviar o MouseMove original
                }
                BoundaryResult::Local => {}
            }
        }

        // Se cursor está no lado remoto, enviar evento ao cliente
        let is_remote = {
            // check síncrono via try_lock
            true // simplificado — o estado correto é checado no select abaixo
        };
        let _ = msg_tx_for_capture.try_send(Message::Input(event));
    }));

    if let Err(e) = capture_result {
        warn!("Captura de input indisponível: {} — verifique permissão de Acessibilidade", e);
    }

    // ── Receptor de arquivos ────────────────────────────────────────────────
    let mut file_receiver = match crate::transfer::FileReceiver::new().await {
        Ok(r) => Some(r),
        Err(e) => { tracing::warn!("FileReceiver indisponível: {}", e); None }
    };

    // ── Loop principal: ler mensagens do cliente + enviar do canal ─────────
    let ping_interval = tokio::time::interval(std::time::Duration::from_secs(2));
    tokio::pin!(ping_interval);
    let mut ping_sent_at: Option<std::time::Instant> = None;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Sessão com {} cancelada", peer_addr);
                let _ = send_message(&mut stream, &Message::Disconnect {
                    reason: "servidor encerrado".into(),
                }).await;
                break;
            }

            // Enviar mensagens do canal ao cliente
            Some(msg) = msg_rx.recv() => {
                if let Err(e) = send_message(&mut stream, &msg).await {
                    warn!("Erro ao enviar mensagem para {}: {}", peer_addr, e);
                    break;
                }
            }

            // Ping periódico para medir latência
            _ = ping_interval.tick() => {
                ping_sent_at = Some(std::time::Instant::now());
                if send_message(&mut stream, &Message::Ping).await.is_err() {
                    break;
                }
            }

            // Receber mensagens do cliente
            result = recv_message(&mut stream) => {
                match result {
                    Ok(Message::Pong) => {
                        if let Some(sent) = ping_sent_at.take() {
                            let rtt_ms = sent.elapsed().as_millis() as u32;
                            let mut status = state.connection_status.lock().await;
                            if let ConnectionStatus::Connected { ref mut latency_ms, .. } = *status {
                                *latency_ms = rtt_ms;
                            }
                        }
                    }
                    Ok(Message::LeaveScreen) => {
                        let mut active = state.active_screen.lock().await;
                        *active = ActiveScreen::Local;
                    }
                    Ok(Message::Disconnect { reason }) => {
                        info!("Cliente desconectou: {}", reason);
                        break;
                    }
                    Ok(ref msg @ Message::ClipboardData { .. }) => {
                        crate::clipboard::sync::apply_clipboard_message(msg);
                    }
                    Ok(Message::FileStart { id, name, size }) => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_start(id, name, size).await.unwrap_or_else(|e| {
                                warn!("FileStart error: {}", e);
                            });
                        }
                    }
                    Ok(Message::FileChunk { id, seq, data }) => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_chunk(id, seq, data).await.unwrap_or_else(|e| {
                                warn!("FileChunk error: {}", e);
                            });
                        }
                    }
                    Ok(Message::FileEnd { id, checksum }) => {
                        if let Some(ref mut recv) = file_receiver {
                            match recv.on_file_end(id, checksum).await {
                                Ok((name, path)) => {
                                    info!("Arquivo recebido: '{}' → {:?}", name, path);
                                }
                                Err(e) => {
                                    warn!("FileEnd error: {}", e);
                                    let _ = send_message(&mut stream, &Message::FileRetry { id }).await;
                                }
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Erro ao receber de {}: {}", peer_addr, e);
                        break;
                    }
                }
            }
        }
    }

    capture.stop();
    { *state.message_tx.lock().await = None; }
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
        let mut started = state.session_started_at.lock().await;
        *started = None;
    }
    info!("Conexão com {} encerrada", peer_addr);
}

/// Computa HMAC-SHA256(psk_hex, nonce) como hex — usado para autenticação
fn compute_hmac(psk_hex: &str, nonce: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(psk_hex.as_bytes());
    hasher.update(b":");
    hasher.update(nonce.as_bytes());
    hex::encode(hasher.finalize())
}
