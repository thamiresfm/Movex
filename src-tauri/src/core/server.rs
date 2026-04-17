use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::core::state::{ActiveScreen, ConnectionStatus, SharedState};
use crate::core::stats::get_primary_screen_size;
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
                        // Bloquear nova conexão se já há uma ativa ou em aprovação
                        let busy = matches!(
                            *state.connection_status.lock().await,
                            ConnectionStatus::Connected { .. } | ConnectionStatus::PendingApproval { .. }
                        );
                        if busy {
                            warn!("Rejeitando nova conexão de {} — já há cliente conectado ou aprovação pendente", peer_addr);
                            continue;
                        }
                        // Marcar como PendingApproval atomicamente para evitar race condition
                        {
                            let mut status = state.connection_status.lock().await;
                            *status = ConnectionStatus::PendingApproval { peer_hostname: peer_addr.to_string() };
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
    // ── Handshake com PSK correta ────────────────────────────────────────────
    // 1. Servidor envia ServerChallenge com nonce aleatório
    let server_nonce = hex::encode(rand::random::<[u8; 32]>());
    let our_hostname_for_challenge = { state.settings.lock().await.hostname.clone() };
    if let Err(e) = send_message(&mut stream, &Message::ServerChallenge {
        version: PROTOCOL_VERSION,
        hostname: our_hostname_for_challenge,
        server_nonce: server_nonce.clone(),
    }).await {
        warn!("Erro ao enviar ServerChallenge para {}: {}", peer_addr, e);
        return;
    }

    // 2. Receber Hello com HMAC do cliente
    let hello = match recv_message(&mut stream).await {
        Ok(m) => m,
        Err(e) => { warn!("Erro ao receber Hello de {}: {}", peer_addr, e); return; }
    };

    let peer_hostname = match hello {
        Message::Hello { version, hostname, hmac } => {
            if version != PROTOCOL_VERSION {
                let _ = send_message(&mut stream, &Message::HelloReject {
                    reason: format!("Versão incompatível: esperado {}, recebido {}", PROTOCOL_VERSION, version),
                }).await;
                return;
            }
            // 3. Validar HMAC
            let psk_hex = { state.settings.lock().await.psk_hex.clone() };
            if !crate::core::auth::verify_hmac(&psk_hex, &server_nonce, &hmac) {
                warn!("PSK incorreta de {} — rejeitando conexão", peer_addr);
                let _ = send_message(&mut stream, &Message::HelloReject {
                    reason: "Chave de segurança incorreta".to_string(),
                }).await;
                return;
            }
            hostname
        }
        _ => { warn!("Esperava Hello de {}", peer_addr); return; }
    };

    // ── Solicitar aprovação do usuário ──────────────────────────────────────
    let our_hostname = { state.settings.lock().await.hostname.clone() };

    // Avisar o cliente que está aguardando aprovação
    let _ = send_message(&mut stream, &Message::ConnectionPending {
        hostname: our_hostname.clone(),
    }).await;

    // Registrar no estado para a UI exibir o modal
    let (approval_tx, approval_rx) = tokio::sync::oneshot::channel::<bool>();
    {
        *state.pending_approval.lock().await = Some(peer_hostname.clone());
        *state.approval_tx.lock().await = Some(approval_tx);
    }

    info!("Aguardando aprovação do usuário para conectar: {} ({})", peer_hostname, peer_addr);
    state.send_notification(
        "Movex — Solicitação de Conexão",
        &format!("{} quer controlar este computador", peer_hostname),
    ).await;

    // Aguardar decisão com timeout de 60 segundos
    let approved = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        approval_rx,
    ).await;

    // Limpar estado pendente
    {
        *state.pending_approval.lock().await = None;
        *state.approval_tx.lock().await = None;
    }

    match approved {
        Ok(Ok(true)) => {
            // Aprovado — enviar HelloAck
            info!("Conexão aprovada: {} ({})", peer_hostname, peer_addr);
            let _ = send_message(&mut stream, &Message::ConnectionApproved).await;
        }
        Ok(Ok(false)) => {
            // Rejeitado pelo usuário
            warn!("Conexão rejeitada pelo usuário: {} ({})", peer_hostname, peer_addr);
            let _ = send_message(&mut stream, &Message::ConnectionRejected {
                reason: "Conexão recusada pelo usuário do servidor".to_string(),
            }).await;
            return;
        }
        _ => {
            // Timeout ou canal fechado
            warn!("Timeout na aprovação de conexão: {} ({})", peer_hostname, peer_addr);
            let _ = send_message(&mut stream, &Message::ConnectionRejected {
                reason: "Tempo de aprovação esgotado (60s)".to_string(),
            }).await;
            return;
        }
    }

    // Enviar HelloAck — conexão estabelecida
    let _ = send_message(&mut stream, &Message::HelloAck {
        version: PROTOCOL_VERSION,
        hostname: our_hostname,
    }).await;

    info!("Cliente autenticado: {} ({})", peer_hostname, peer_addr);
    // Notificação de conexão estabelecida
    state.send_notification(
        "Movex — Conectado",
        &format!("Controlando: {}", peer_hostname),
    ).await;
    // Adicionar ao histórico de peers recentes
    {
        let mut s = state.settings.lock().await;
        s.add_recent_peer(&peer_hostname, &peer_addr.ip().to_string(), peer_addr.port());
        let _ = s.save();
    }
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
    // Detectar resolução real do monitor (usa multi-monitor se disponível)
    let monitors = crate::screen::layout::detect_monitors();
    let primary = monitors.monitors.iter()
        .find(|m| m.is_primary)
        .or_else(|| monitors.monitors.first());
    let (screen_w, screen_h, scale) = primary
        .map(|m| (m.width, m.height, m.scale_factor))
        .unwrap_or_else(|| { let (w, h) = get_primary_screen_size(); (w, h, 1.0) });

    let layout = ScreenLayout {
        local: ScreenResolution { width: screen_w, height: screen_h, scale_factor: scale },
        peer: None,
        peer_position,
    };

    // Resolução do servidor enviada ao cliente via SyncInfo (quando implementado)
    // Por ora apenas loga as dimensões detectadas
    tracing::info!("Monitor local: {}x{} scale={:.1}", screen_w, screen_h, scale);

    let state_for_capture = state.clone();
    let msg_tx_for_capture = msg_tx.clone();
    let layout_clone = layout.clone();

    // Iniciar captura de input
    let capture = std::sync::Arc::new(crate::input::platform::create_capture());
    let capture_ref_for_boundary = std::sync::Arc::clone(&capture);

    // Unlock cursor quando LeaveScreen é recebido (cursor volta ao servidor)
    // isso é feito dentro do loop principal abaixo

    let capture_result = capture.start(Box::new(move |event| {
        // Verificar se cursor cruzou a borda
        if let crate::input::InputEvent::MouseMove { x, y } = &event {
            let px = x * layout_clone.local.width as f32;
            let py = y * layout_clone.local.height as f32;
            match check_boundary(px, py, &layout_clone) {
                BoundaryResult::CrossedToPeer { entry_x, entry_y } => {
                    // Travar cursor fisicamente na borda (lock_cursor)
                    capture_ref_for_boundary.lock_cursor();
                    // Enviar EnterScreen + posição de entrada ao cliente
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

        // Verificar modo lock — bloquear transição
        let locked = state_for_capture.lock_mode.load(std::sync::atomic::Ordering::Relaxed);

        // Só enviar evento ao cliente se o cursor estiver na tela remota e lock desativado
        let is_remote = !locked && state_for_capture.active_screen
            .try_lock()
            .map(|s| *s == ActiveScreen::Remote)
            .unwrap_or(false);

        if is_remote {
            let _ = msg_tx_for_capture.try_send(Message::Input(event));
            state_for_capture.stats.inc_event_sent();
        }
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
                        // Cursor voltou ao servidor — liberar o travamento físico
                        capture.unlock_cursor();
                        let mut active = state.active_screen.lock().await;
                        *active = ActiveScreen::Local;
                    }
                    Ok(Message::Disconnect { reason }) => {
                        info!("Cliente desconectou: {}", reason);
                        break;
                    }
                    // Responder Pong ao Ping enviado pelo cliente (para ele medir latência)
                    Ok(Message::Ping) => {
                        let _ = send_message(&mut stream, &Message::Pong).await;
                    }
                    // Message::Pong já tratado no arm acima (linha ~357) — não duplicar
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
                                Ok((name, _path)) => {
                                    info!("Arquivo recebido: '{}'", name);
                                    state.send_notification(
                                        "Movex — Arquivo Recebido",
                                        &format!("📁 {}", name),
                                    ).await;
                                    state.stats.inc_file_received();
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

    capture.unlock_cursor(); // garantir que o cursor não fique preso ao desconectar
    capture.stop();
    { *state.message_tx.lock().await = None; }
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
        let mut started = state.session_started_at.lock().await;
        *started = None;
    }
    state.send_notification(
        "Movex — Desconectado",
        &format!("Sessão encerrada com {}", peer_hostname),
    ).await;
    info!("Conexão com {} encerrada", peer_addr);
}

