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

/// Quem está em papel **Cliente** não abre a porta TCP: não adianta apontar o IP do Cliente a partir do Servidor.
const HINT_TOPOLOGY: &str = "Quem está só como Cliente não aceita conexões nesta porta — ligue a partir do Cliente para o IP do computador em Servidor (ou ative Servidor e Conectar no outro PC).";

async fn connection_failed_client(state: &SharedState) {
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }
    crate::emit_status_to_main(state).await;
}

/// Conecta a um endereço específico (descoberto via mDNS) sem alterar settings persistidas.
/// Não tenta reconectar — apenas uma tentativa.
pub async fn connect_to_addr(
    state: SharedState,
    addr: String,
    port: u16,
    cancel: CancellationToken,
) {
    let target = format!("{}:{}", addr, port);
    info!("Conectando (sessão) a {}...", target);
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Connecting;
    }

    let tcp = tokio::select! {
        _ = cancel.cancelled() => {
            connection_failed_client(&state).await;
            return;
        }
        r = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            TcpStream::connect(&target)
        ) => match r {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                warn!("Falha ao conectar em {}: {}", target, e);
                state.user_visible_connection_error(
                    "Movex — Conexão",
                    &format!(
                        "Não foi possível alcançar {}. Verifique IP e firewall (porta {}). {} No PC em Servidor: na primeira ligação aceite o UAC para o Firewall (ou Configurações → Aplicar regras).",
                        target, port, HINT_TOPOLOGY
                    ),
                ).await;
                connection_failed_client(&state).await;
                return;
            }
            Err(_) => {
                warn!("Timeout ao conectar em {}", target);
                state.user_visible_connection_error(
                    "Movex — Conexão",
                    &format!("Tempo esgotado ao conectar a {}. Rede ou firewall podem estar bloqueando.", target),
                ).await;
                connection_failed_client(&state).await;
                return;
            }
        }
    };

    let known_fp = state.settings.lock().await.server_cert_fingerprint.clone();
    let (connector, tofu_verifier) = create_tls_connector(known_fp);
    let domain = ServerName::try_from("movex.local").expect("domínio inválido");

    let mut tls = match connector.connect(domain, tcp).await {
        Ok(s) => s,
        Err(e) => {
            warn!("Falha TLS ao conectar em {}: {}", target, e);
            state.user_visible_connection_error(
                "Movex — TLS",
                "Falha no handshake TLS. Se o servidor foi reinstalado, apague o arquivo de confiança: em Configurações use «Resetar» ou remova server_cert em ~/.movex nas duas máquinas.",
            ).await;
            connection_failed_client(&state).await;
            return;
        }
    };

    if let Some(observed_fp) = tofu_verifier.take_observed() {
        let mut settings = state.settings.lock().await;
        if settings.server_cert_fingerprint.is_none() {
            settings.server_cert_fingerprint = Some(observed_fp);
            let _ = settings.save();
        }
    }

    let (screen_name, psk_hex) = {
        let s = state.settings.lock().await;
        (s.screen_name.clone(), s.psk_hex.clone())
    };

    if let Ok(peer_hostname) = do_handshake(&mut tls, &screen_name, &psk_hex, &state, &cancel).await {
        info!("Conectado (sessão) a: {}", peer_hostname);
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Connected {
                peer_hostname: peer_hostname.clone(),
                peer_addr: target.clone(),
                latency_ms: 0,
            };
            let mut started = state.session_started_at.lock().await;
            *started = Some(std::time::Instant::now());
        }
        crate::emit_status_to_main(&state).await;
        state
            .user_visible_connection_success(&format!("Ligado a «{peer_hostname}» ({target})."))
            .await;
        let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
        { *state.message_tx.lock().await = Some(msg_tx); }
        run_session(&mut tls, state.clone(), &mut msg_rx, cancel).await;
        { *state.message_tx.lock().await = None; }
    } else {
        state.user_visible_connection_error(
            "Movex — Conexão",
            "Handshake falhou: confira versão do Movex e rede (TLS).",
        ).await;
        connection_failed_client(&state).await;
    }

    { *state.session_server_addr.lock().await = None; }
}

/// Executa o handshake HMAC com o servidor e retorna o nome de ecrã do peer (servidor).
/// Retorna `Err` para qualquer falha — o chamador decide se reconecta ou não.
async fn do_handshake<S>(
    stream: &mut S,
    screen_name: &str,
    psk_hex: &str,
    state: &SharedState,
    cancel: &CancellationToken,
) -> Result<String, ()>
where
    S: tokio::io::AsyncReadExt + tokio::io::AsyncWriteExt + Unpin,
{
    let challenge = match recv_message(stream).await {
        Ok(Message::ServerChallenge { version, server_nonce, .. }) => {
            if version != PROTOCOL_VERSION {
                warn!("Versão incompatível do servidor");
                return Err(());
            }
            server_nonce
        }
        Ok(_) => { warn!("Esperava ServerChallenge"); return Err(()); }
        Err(e) => { warn!("Erro ao receber ServerChallenge: {}", e); return Err(()); }
    };

    let hmac = crate::core::auth::compute_hmac(psk_hex, &challenge);
    if let Err(e) = send_message(stream, &Message::Hello {
        version: PROTOCOL_VERSION,
        hostname: screen_name.to_string(),
        hmac,
    }).await {
        warn!("Erro ao enviar Hello: {}", e);
        return Err(());
    }

    let first_msg = recv_message(stream).await;

    let resolved = match first_msg {
        Ok(Message::ConnectionPending { hostname: server_name }) => {
            info!("Aguardando aprovação do servidor '{}'...", server_name);
            {
                let mut status = state.connection_status.lock().await;
                *status = ConnectionStatus::Connecting;
            }
            if cancel.is_cancelled() { return Err(()); }
            recv_message(stream).await
        }
        other => other,
    };

    match resolved {
        Ok(Message::HelloReject { reason }) => {
            warn!("Servidor rejeitou o handshake: {}", reason);
            state
                .user_visible_connection_error("Movex — Nome do ecrã", &reason)
                .await;
            Err(())
        }
        Ok(Message::ConnectionRejected { reason }) => {
            warn!("Conexão rejeitada pelo servidor: {}", reason);
            state
                .user_visible_connection_error("Movex — Conexão recusada", &reason)
                .await;
            {
                let mut status = state.connection_status.lock().await;
                *status = ConnectionStatus::Disconnected;
            }
            Err(())
        }
        Ok(Message::HelloAck { hostname: peer, .. }) => Ok(peer),
        Ok(Message::ConnectionApproved) => {
            info!("Conexão aprovada pelo servidor!");
            match recv_message(stream).await {
                Ok(Message::HelloAck { hostname: peer, .. }) => Ok(peer),
                Ok(Message::HelloReject { reason }) => {
                    warn!("Rejeitado após aprovação: {}", reason);
                    Err(())
                }
                Ok(_) => { warn!("Esperava HelloAck após aprovação"); Err(()) }
                Err(e) => { warn!("Erro ao receber HelloAck: {}", e); Err(()) }
            }
        }
        Ok(other) => {
            warn!("Mensagem inesperada durante handshake: {:?}", other);
            Err(())
        }
        Err(e) => {
            warn!("Erro durante handshake: {}", e);
            Err(())
        }
    }
}

/// Conecta ao servidor com reconexão automática e suporte a cancelamento.
pub async fn connect(state: SharedState, cancel: CancellationToken) {
    let policy = ReconnectPolicy::default();
    // Falhas seguidas só de TCP (timeout / recusado) — notificação na 3.ª falha.
    let mut tcp_unreachable_streak: u32 = 0;

    loop {
        let (maybe_addr, port) = {
            let s = state.settings.lock().await;
            (
                s.server_addr.as_ref().and_then(|a| {
                    let t = a.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                }),
                s.port,
            )
        };

        let Some(server_addr) = maybe_addr else {
            warn!("Cliente sem IP do servidor — não use 127.0.0.1 implícito (defina nas opções ou conecte pelo cartão na rede)");
            state
                .user_visible_connection_error(
                    "Movex",
                    "Defina o IP do servidor nas configurações ou toque num PC na lista (Rede → Atualizar).",
                )
                .await;
            connection_failed_client(&state).await;
            return;
        };

        let addr = format!("{}:{}", server_addr, port);
        info!("Conectando ao servidor Movex em {}...", addr);
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Connecting;
        }
        crate::emit_status_to_main(&state).await;

        let connect_result = tokio::select! {
            _ = cancel.cancelled() => {
                info!("Cliente Movex cancelado durante conexão");
                connection_failed_client(&state).await;
                return;
            }
            r = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                TcpStream::connect(&addr)
            ) => r
        };

        match connect_result {
            Ok(Ok(tcp)) => {
                tcp_unreachable_streak = 0;
                let known_fp = state.settings.lock().await.server_cert_fingerprint.clone();
                let (connector, tofu_verifier) = create_tls_connector(known_fp);
                let domain = ServerName::try_from("movex.local").expect("domínio inválido");

                match connector.connect(domain, tcp).await {
                    Ok(mut tls) => {
                        if let Some(observed_fp) = tofu_verifier.take_observed() {
                            let mut settings = state.settings.lock().await;
                            if settings.server_cert_fingerprint.is_none() {
                                info!("TOFU: gravando fingerprint do servidor");
                                settings.server_cert_fingerprint = Some(observed_fp);
                                let _ = settings.save();
                            }
                        }

                        let (screen_name, psk_hex) = {
                            let s = state.settings.lock().await;
                            (s.screen_name.clone(), s.psk_hex.clone())
                        };

                        if let Ok(peer_hostname) =
                            do_handshake(&mut tls, &screen_name, &psk_hex, &state, &cancel).await
                        {
                            info!("Conectado ao servidor: {}", peer_hostname);
                                policy.reset();
                                {
                                    let mut status = state.connection_status.lock().await;
                                    *status = ConnectionStatus::Connected {
                                    peer_hostname: peer_hostname.clone(),
                                    peer_addr: addr.clone(),
                                        latency_ms: 0,
                                    };
                                let mut started = state.session_started_at.lock().await;
                                *started = Some(std::time::Instant::now());
                            }
                            crate::emit_status_to_main(&state).await;
                            state
                                .user_visible_connection_success(&format!(
                                    "Ligado ao servidor «{peer_hostname}» ({addr})."
                                ))
                                .await;

                            let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
                            { *state.message_tx.lock().await = Some(msg_tx); }
                            run_session(&mut tls, state.clone(), &mut msg_rx, cancel.clone()).await;
                            { *state.message_tx.lock().await = None; }
                        } else {
                            warn!("Handshake falhou — verifique filtro de nome de ecrã e versão do Movex");
                            state
                                .user_visible_connection_error(
                                    "Movex",
                                    "Handshake falhou: verifique filtro de nome de ecrã no servidor e mesma versão do app.",
                                )
                                .await;
                            connection_failed_client(&state).await;
                            return;
                        }
                    }
                    Err(e) => {
                        warn!("Falha no TLS handshake: {}", e);
                        state
                            .user_visible_connection_error(
                                "Movex — TLS",
                                "Não foi possível estabelecer TLS. Confirme IP/porta, firewall no PC servidor e mesma LAN. Se reinstalou o Movex no servidor ou mudou de PC: Configurações → Esquecer certificado TLS (ou Resetar).",
                            )
                            .await;
                        connection_failed_client(&state).await;
                        return;
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Falha ao conectar em {}: {}", addr, e);
                tcp_unreachable_streak = tcp_unreachable_streak.saturating_add(1);
                state
                    .log_to_connection_panel(
                        "warn",
                        &format!("TCP indisponível {addr}: {e} (tentativa {tcp_unreachable_streak})"),
                    )
                    .await;
                if tcp_unreachable_streak == 3 {
                    state
                        .user_visible_connection_error(
                            "Movex — Não alcança o servidor",
                            &format!(
                                "Confira IP, rede e firewall (porta). {} No Servidor: aceite o UAC do Firewall na 1.ª ligação.",
                                HINT_TOPOLOGY
                            ),
                        )
                        .await;
                }
                crate::emit_status_to_main(&state).await;
            }
            Err(_) => {
                warn!("Timeout ao conectar em {}", addr);
                tcp_unreachable_streak = tcp_unreachable_streak.saturating_add(1);
                state
                    .log_to_connection_panel(
                        "warn",
                        &format!("Timeout TCP ao conectar a {addr} (tentativa {tcp_unreachable_streak})"),
                    )
                    .await;
                if tcp_unreachable_streak == 3 {
                    state
                        .user_visible_connection_error(
                            "Movex — Timeout na rede",
                            &format!(
                                "Timeout ao alcançar o servidor. Rede ou firewall. {} No Servidor: aceite o UAC do Firewall na 1.ª ligação.",
                                HINT_TOPOLOGY
                            ),
                        )
                        .await;
                }
                crate::emit_status_to_main(&state).await;
            }
        }

        if cancel.is_cancelled() {
            connection_failed_client(&state).await;
            return;
        }

        let (attempt, wait) = policy.next_delay_with_attempt();
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Reconnecting { attempt };
            let mut started = state.session_started_at.lock().await;
            *started = None;
        }
        crate::emit_status_to_main(&state).await;
        info!("Tentativa {}: reconectando em {}s...", attempt + 1, wait.as_secs());

        tokio::select! {
            _ = cancel.cancelled() => {
                connection_failed_client(&state).await;
                return;
            }
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

            Some(msg) = msg_rx.recv() => {
                if let Err(e) = send_message(stream, &msg).await {
                    warn!("Erro ao enviar para servidor: {}", e);
                    break;
                }
            }

            _ = ping_interval.tick() => {
                ping_sent_at = Some(std::time::Instant::now());
                if send_message(stream, &Message::Ping).await.is_err() { break; }
            }

            _ = clipboard_check.tick() => {
                let sync_enabled = {
                    if let Ok(s) = state.settings.try_lock() {
                        s.clipboard_sync_enabled
                    } else {
                        false
                    }
                };
                if !sync_enabled { continue; }
                if let Some(msg) = crate::clipboard::sync::create_clipboard_message() {
                    let key = match &msg {
                        Message::ClipboardData { mime, data } => {
                            let hash = crate::core::utils::crc32(data);
                            format!("{}:{}", mime.split(';').next().unwrap_or(mime), hash)
                        }
                        _ => continue,
                    };
                    if last_clipboard.as_ref() != Some(&key) {
                        last_clipboard = Some(key);
                        if send_message(stream, &msg).await.is_err() { break; }
                    }
                }
            }

            result = recv_message(stream) => {
                match result {
            Ok(Message::EnterScreen) => {
                        state.active_screen_remote.store(true, std::sync::atomic::Ordering::Release);
                let mut active = state.active_screen.lock().await;
                *active = ActiveScreen::Remote;
                        info!("Cursor entrou nesta máquina");
            }
            Ok(Message::LeaveScreen) => {
                        state.active_screen_remote.store(false, std::sync::atomic::Ordering::Release);
                let mut active = state.active_screen.lock().await;
                *active = ActiveScreen::Local;
            }
                    Ok(Message::Input(event)) => {
                        inject_event(event);
                    }
                    Ok(ref msg @ Message::ClipboardData { .. }) => {
                        crate::clipboard::sync::apply_clipboard_message(msg);
                        // Atualizar cache com hash do conteúdo recebido para evitar reenvio
                        // imediato (dados binários como PNG não são comparáveis por texto)
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
                        if let Some(sent) = ping_sent_at.take() {
                            let rtt = sent.elapsed().as_millis() as u32;
                            let mut status = state.connection_status.lock().await;
                            if let ConnectionStatus::Connected { ref mut latency_ms, .. } = *status {
                                *latency_ms = rtt;
                            }
                            drop(status);
                            crate::emit_status_to_main(&state).await;
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
    drop(status);
    drop(started);
    state.stats.reset();
    crate::emit_status_to_main(&state).await;
}
