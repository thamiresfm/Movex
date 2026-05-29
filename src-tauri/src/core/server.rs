use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::core::state::{ActiveScreen, ConnectionStatus, SharedState};
use crate::network::protocol::{Message, PROTOCOL_VERSION};
use crate::network::transport::{
    create_tls_acceptor, load_or_generate_server_cert, recv_message, recv_message_counted,
    send_message,
};
use crate::screen::boundary::{check_boundary, BoundaryResult};
use crate::screen::layout::{PeerPosition, ScreenLayout, ScreenResolution};

/// Volta a aceitar clientes (após falha de handshake ou fim de sessão).
async fn server_resume_listening(state: &SharedState) {
    {
        let mut st = state.connection_status.lock().await;
        *st = ConnectionStatus::Listening;
    }
    {
        let mut started = state.session_started_at.lock().await;
        *started = None;
    }
    crate::ipc::emit_status_to_main(state).await;
}

/// Inicia o servidor Movex com cancelamento e envio de input ao cliente.
pub async fn start(state: SharedState, cancel: CancellationToken) -> Result<(), String> {
    let port = { state.settings.lock().await.port };

    let (certs, key) = load_or_generate_server_cert()
        .map_err(|e| format!("Erro ao carregar certificado TLS: {}", e))?;
    let acceptor = create_tls_acceptor(certs, key)
        .map_err(|e| format!("Erro ao criar TLS acceptor: {}", e))?;

    let (screen_name, mdns_port) = {
        let s = state.settings.lock().await;
        (s.screen_name.clone(), s.port)
    };
    let _mdns_daemon = crate::network::discovery::announce_server(&screen_name, mdns_port).ok();

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Falha ao escutar em {}: {}", addr, e))?;

    info!("Servidor Movex escutando em {}", addr);
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Listening;
    }
    crate::ipc::emit_status_to_main(&state).await;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Servidor Movex cancelado");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((tcp_stream, peer_addr)) => {
                        // Substituir sessão stale por nova conexão.
                        //
                        // Antes rejeitávamos qualquer Connected/Connecting, mas
                        // isso bloqueava reconexões legítimas após timeout TCP:
                        // o read loop demora segundos para detectar o cliente
                        // morto, e durante esse intervalo o estado fica
                        // "Connected" mas a sessão não responde. Cliente
                        // tentando reconectar era cuspido fora.
                        //
                        // Comportamento Barrier/Synergy: nova conexão sempre
                        // ganha; pedimos disconnect gracioso ao peer antigo
                        // e limpamos o estado antes de aceitar a nova.
                        {
                            let stale = {
                                let status = state.connection_status.lock().await;
                                matches!(
                                    *status,
                                    ConnectionStatus::Connected { .. } | ConnectionStatus::Connecting
                                )
                            };
                            if stale {
                                warn!(
                                    "Substituindo sessão stale por nova conexão de {}",
                                    peer_addr
                                );
                                // Pedir disconnect ao peer antigo (best-effort) e limpar canal na mesma aquisição
                                {
                                    let mut tx = state.message_tx.lock().await;
                                    if let Some(t) = tx.as_ref() {
                                        let _ = t.try_send(crate::network::protocol::Message::Disconnect {
                                            reason: "substituído por nova conexão".into(),
                                        });
                                    }
                                    *tx = None;
                                }
                                *state.connection_status.lock().await = ConnectionStatus::Listening;
                                crate::ipc::emit_status_to_main(&state).await;
                            }
                        }

                        info!("Nova conexão TCP de {}", peer_addr);
                        // TCP keepalive: previne que NATs/routers derrubem a conexão por inatividade
                        let tcp_stream = {
                            use socket2::{Socket, TcpKeepalive};
                            match tcp_stream.into_std() {
                                Ok(std_s) => {
                                    let sock = Socket::from(std_s);
                                    let ka = TcpKeepalive::new()
                                        .with_time(std::time::Duration::from_secs(5))
                                        .with_interval(std::time::Duration::from_secs(2));
                                    let _ = sock.set_tcp_keepalive(&ka);
                                    match tokio::net::TcpStream::from_std(sock.into()) {
                                        Ok(s) => s,
                                        Err(e) => { warn!("Falha ao recriar TcpStream: {}", e); continue; }
                                    }
                                }
                                Err(e) => { warn!("Falha ao converter TcpStream: {}", e); continue; }
                            }
                        };
                        let tls_stream = match acceptor.accept(tcp_stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                warn!("Falha no TLS handshake com {}: {}", peer_addr, e);
                                continue;
                            }
                        };

                        {
                            let mut status = state.connection_status.lock().await;
                            *status = ConnectionStatus::Connecting;
                        }
                        crate::ipc::emit_status_to_main(&state).await;

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
    // Handshake: enviar nonce → receber Hello (HMAC enviado por compatibilidade; não é obrigatório coincidir entre PCs)
    let server_nonce = hex::encode(rand::random::<[u8; 32]>());
    let our_screen_for_challenge = { state.settings.lock().await.screen_name.clone() };
    if let Err(e) = send_message(&mut stream, &Message::ServerChallenge {
        version: PROTOCOL_VERSION,
        hostname: our_screen_for_challenge,
        server_nonce: server_nonce.clone(),
    }).await {
        warn!("Erro ao enviar ServerChallenge para {}: {}", peer_addr, e);
        server_resume_listening(&state).await;
        return;
    }

    let hello = match recv_message(&mut stream).await {
        Ok(m) => m,
        Err(e) => {
            warn!("Erro ao receber Hello de {}: {}", peer_addr, e);
            server_resume_listening(&state).await;
            return;
        }
    };

    let peer_hostname = match hello {
        Message::Hello { version, hostname, hmac } => {
            if version != PROTOCOL_VERSION {
                let _ = send_message(&mut stream, &Message::HelloReject {
                    reason: format!("Versão incompatível: esperado {}, recebido {}", PROTOCOL_VERSION, version),
                }).await;
                server_resume_listening(&state).await;
                return;
            }
            let psk = { state.settings.lock().await.psk_hex.clone() };
            if !crate::core::auth::verify_hmac(&psk, &server_nonce, &hmac) {
                warn!("Handshake rejeitado de {}: HMAC inválido — oferecendo PSK sync via TLS", peer_addr);
                // Envia o PSK actual pela camada TLS já estabelecida e autenticada (TOFU).
                // O cliente só aceita se já conhecia o certificado deste servidor.
                let _ = send_message(&mut stream, &Message::HelloPskSync {
                    new_psk_hex: psk,
                }).await;
                server_resume_listening(&state).await;
                return;
            }
            // Truncar para evitar hostname arbitrariamente longo em notificações/logs.
            let h = hostname.trim().chars().take(64).collect::<String>();
            if h.is_empty() {
                let _ = send_message(&mut stream, &Message::HelloReject {
                    reason: "Nome de ecrã vazio.".into(),
                }).await;
                server_resume_listening(&state).await;
                return;
            }
            h
        }
        _ => {
            warn!("Esperava Hello de {}", peer_addr);
            server_resume_listening(&state).await;
            return;
        }
    };

    if let Some(ref expected) = state.settings.lock().await.expected_client_screen_name {
        let exp = expected.trim();
        if !exp.is_empty() && peer_hostname.trim() != exp {
            warn!(
                "Nome de ecrã do cliente '{}' não coincide com o esperado '{}'",
                peer_hostname, exp
            );
            let _ = send_message(&mut stream, &Message::HelloReject {
                reason: format!(
                    "Nome de ecrã do cliente («{}») não coincide com o configurado no servidor («{}»). Ajuste em Configurações (estilo Barrier).",
                    peer_hostname.trim(),
                    exp
                ),
            }).await;
            server_resume_listening(&state).await;
            return;
        }
    }

    // Sem passo de aprovação no servidor: após TLS + Hello válido, aceita de imediato.
    let our_screen_name = { state.settings.lock().await.screen_name.clone() };

    let _ = send_message(&mut stream, &Message::HelloAck {
        version: PROTOCOL_VERSION,
        hostname: our_screen_name,
    }).await;

    info!("Cliente autenticado: {} ({})", peer_hostname, peer_addr);
    state.send_notification(
        "Movex — Conectado",
        &format!("Controlando: {}", peer_hostname),
    ).await;
    {
        let mut s = state.settings.lock().await;
        s.add_recent_peer(&peer_hostname, &peer_addr.ip().to_string(), peer_addr.port());
        let _ = s.save();
    }
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Connected {
            peer_hostname: peer_hostname.clone(),
            peer_addr: peer_addr.to_string(),
            latency_ms: 0,
        };
        let mut started = state.session_started_at.lock().await;
        *started = Some(std::time::Instant::now());
    }
    crate::ipc::emit_status_to_main(&state).await;

    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
    { *state.message_tx.lock().await = Some(msg_tx.clone()); }

    let peer_position = {
        let s = state.settings.lock().await;
        match s.peer_position {
            crate::config::ScreenPosition::Left  => PeerPosition::Left,
            crate::config::ScreenPosition::Above => PeerPosition::Above,
            crate::config::ScreenPosition::Below => PeerPosition::Below,
            _                                    => PeerPosition::Right,
        }
    };
    // Borda KMS na área **virtual total** — não apenas no rect do primário.
    let monitors = crate::screen::layout::detect_monitors();
    let (_, _, bbox_w, bbox_h) = monitors.bounding_box();
    let primary = monitors
        .monitors
        .iter()
        .find(|m| m.is_primary)
        .or_else(|| monitors.monitors.first());
    let scale = primary.map(|m| m.scale_factor).unwrap_or(1.0_f32);

    let layout = ScreenLayout {
        local: ScreenResolution {
            width: bbox_w,
            height: bbox_h,
            scale_factor: scale,
        },
        peer: None,
        peer_position,
    };

    tracing::info!(
        "Área desktop virtual (bounding box KMS): {}x{} scale_primário={:.1}",
        bbox_w,
        bbox_h,
        scale
    );

    let state_for_capture = state.clone();
    let msg_tx_for_capture = msg_tx.clone();
    let layout_clone = layout.clone();

    let capture = std::sync::Arc::new(crate::input::platform::create_capture());
    let capture_ref_for_boundary = std::sync::Arc::clone(&capture);

    let capture_result = capture.start(Box::new(move |event| {
        // lock_mode impede transição de cursor — verificar antes de qualquer lógica
        let locked = state_for_capture.lock_mode.load(std::sync::atomic::Ordering::Relaxed);

        // Leitura atómica do estado actual — usada tanto na detecção de borda como
        // no encaminhamento. Ler uma única vez para consistência dentro do callback.
        let already_remote = state_for_capture.active_screen_remote
            .load(std::sync::atomic::Ordering::Acquire);

        // Só detectar borda quando estamos em modo local (cursor no ecrã deste PC).
        // Se já estivermos remotos, nunca re-disparar EnterScreen em loop — esse era
        // o bug que impedia o encaminhamento de eventos ao cliente.
        if !locked && !already_remote {
            if let crate::input::InputEvent::MouseMove { x, y } = &event {
                let px = x * layout_clone.local.width as f32;
                let py = y * layout_clone.local.height as f32;
                match check_boundary(px, py, &layout_clone) {
                    BoundaryResult::CrossedToPeer { entry_x, entry_y } => {
                        tracing::info!(
                            "Borda cruzada — enviando EnterScreen ao cliente (entry={:.3},{:.3})",
                            entry_x, entry_y
                        );
                        // Passar entry_x/entry_y ao lock para que o cursor virtual
                        // no macOS comece na posição correcta do ecrã remoto.
                        capture_ref_for_boundary.lock_cursor(entry_x, entry_y);
                        state_for_capture.active_screen_remote
                            .store(true, std::sync::atomic::Ordering::Release);
                        let _ = msg_tx_for_capture.try_send(Message::EnterScreen);
                        let _ = msg_tx_for_capture.try_send(Message::Input(
                            crate::input::InputEvent::MouseMove { x: entry_x, y: entry_y }
                        ));
                        return;
                    }
                    BoundaryResult::Local => {}
                }
            }
        }

        // Encaminhar eventos ao cliente quando o cursor está no ecrã remoto.
        // (já_remote lido acima; re-verificar após o bloco de borda para apanhar
        //  o caso em que outra thread mudou o estado entretanto — improvável mas seguro)
        let is_remote = !locked && (already_remote || state_for_capture.active_screen_remote
            .load(std::sync::atomic::Ordering::Acquire));

        if is_remote {
            let _ = msg_tx_for_capture.try_send(Message::Input(event));
            state_for_capture.stats.inc_event_sent();
        }
    }));

    if let Err(e) = capture_result {
        warn!("Captura de input indisponível: {}", e);
        // Visível ao utilizador (toast + log no painel) — sem isto a sessão fica
        // "ligada" mas nada cruza. Mensagem cobre ambas as plataformas.
        state.user_visible_connection_error(
            "Movex — Captura indisponível",
            &format!(
                "Não foi possível instalar a captura de mouse/teclado neste PC: {}. \
                 macOS: Ajustes → Privacidade → Acessibilidade (ativar Movex e reabrir). \
                 Windows: feche outro KVM (Barrier/Synergy/Deskflow) ou execute como Administrador.",
                e
            ),
        ).await;
    } else if !crate::permissions::macos_accessibility_trusted() {
        // macOS: a tap pode ter sido criada mas o sistema descarta os eventos sem
        // permissão de Acessibilidade. Aviso explícito antes de o utilizador tentar.
        state.user_visible_connection_error(
            "Movex — Permissão de Acessibilidade",
            "Sem Acessibilidade no macOS o Movex não captura o seu mouse/teclado neste PC. Abra Ajustes do Sistema → Privacidade e Segurança → Acessibilidade, ative Movex e feche/reabra o app.",
        ).await;
    }

    let mut file_receiver = match crate::transfer::FileReceiver::new().await {
        Ok(r) => Some(r),
        Err(e) => { tracing::warn!("FileReceiver indisponível: {}", e); None }
    };

    let ping_interval = tokio::time::interval(std::time::Duration::from_secs(1));
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

            Some(msg) = msg_rx.recv() => {
                match send_message(&mut stream, &msg).await {
                    Ok(n) => { state.stats.add_sent(n as u64); }
                    Err(e) => {
                        warn!("Erro ao enviar mensagem para {}: {}", peer_addr, e);
                        break;
                    }
                }
            }

            _ = ping_interval.tick() => {
                ping_sent_at = Some(std::time::Instant::now());
                if send_message(&mut stream, &Message::Ping).await.is_err() {
                    break;
                }
            }

            result = recv_message_counted(&mut stream) => {
                let (msg, recv_bytes) = match result {
                    Ok(pair) => pair,
                    Err(e) => {
                        warn!("Erro ao receber de {}: {}", peer_addr, e);
                        break;
                    }
                };
                state.stats.add_received(recv_bytes as u64);
                match msg {
                    Message::Pong => {
                        if let Some(sent) = ping_sent_at.take() {
                            let rtt_ms = sent.elapsed().as_millis() as u32;
                            let mut status = state.connection_status.lock().await;
                            if let ConnectionStatus::Connected { ref mut latency_ms, .. } = *status {
                                *latency_ms = rtt_ms;
                            }
                            drop(status);
                            crate::ipc::emit_status_to_main(&state).await;
                        }
                    }
                    Message::LeaveScreen => {
                        capture.unlock_cursor();
                        state.active_screen_remote.store(false, std::sync::atomic::Ordering::Release);
                        let mut active = state.active_screen.lock().await;
                        *active = ActiveScreen::Local;
                    }
                    Message::Disconnect { reason } => {
                        info!("Cliente desconectou: {}", reason);
                        break;
                    }
                    Message::Ping => {
                        let _ = send_message(&mut stream, &Message::Pong).await;
                    }
                    ref msg @ Message::ClipboardData { .. } => {
                        crate::clipboard::sync::apply_clipboard_message(msg);
                    }
                    Message::FileStart { id, name, size } => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_start(id, name, size).await.unwrap_or_else(|e| {
                                warn!("FileStart error: {}", e);
                            });
                        }
                    }
                    Message::FileChunk { id, seq, data } => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_chunk(id, seq, data).await.unwrap_or_else(|e| {
                                warn!("FileChunk error: {}", e);
                            });
                        }
                    }
                    Message::FileEnd { id, checksum } => {
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
                    _ => {}
                }
            }
        }
    }

    capture.unlock_cursor();
    capture.stop();
    // Garantir que o modo remoto é desligado ao terminar a sessão — sem isto,
    // se a conexão cair com o cursor no PC remoto, active_screen_remote fica
    // preso em true e a detecção de borda nunca dispara na próxima sessão.
    state.active_screen_remote.store(false, std::sync::atomic::Ordering::Release);
    {
        let mut active = state.active_screen.lock().await;
        *active = crate::core::state::ActiveScreen::Local;
    }
    { *state.message_tx.lock().await = None; }
    state.stats.reset();
    server_resume_listening(&state).await;
    state.send_notification(
        "Movex — Desconectado",
        &format!("Sessão encerrada com {}", peer_hostname),
    ).await;
    info!("Conexão com {} encerrada", peer_addr);
}
