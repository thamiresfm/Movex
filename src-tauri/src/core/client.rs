use rustls::pki_types::ServerName;
use std::sync::atomic::Ordering;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::core::state::{ActiveScreen, ConnectionStatus, SharedState};
use crate::input::events::InputEvent;
use crate::input::inject::inject_event;
use crate::network::protocol::{Message, PROTOCOL_VERSION};
use crate::network::reconnect::ReconnectPolicy;
use crate::network::transport::{create_tls_connector, recv_message, recv_message_counted, send_message};
use crate::screen::boundary::{check_boundary, BoundaryResult};
use crate::screen::layout::{PeerPosition, ScreenLayout, ScreenResolution};

/// Quem está em papel **Cliente** não abre a porta TCP: não adianta apontar o IP do Cliente a partir do Servidor.
const HINT_TOPOLOGY: &str = "Quem está só como Cliente não aceita conexões nesta porta — ligue a partir do Cliente para o IP do computador em Servidor (ou ative Servidor e Conectar no outro PC).";

async fn connection_failed_client(state: &SharedState) {
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }
    crate::ipc::emit_status_to_main(state).await;
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

    // TCP keepalive: previne que NATs/routers derrubem a conexão por inatividade
    let tcp = {
        use socket2::{Socket, TcpKeepalive};
        match tcp.into_std() {
            Ok(std_s) => {
                let sock = Socket::from(std_s);
                let ka = TcpKeepalive::new()
                    .with_time(std::time::Duration::from_secs(5))
                    .with_interval(std::time::Duration::from_secs(2));
                let _ = sock.set_tcp_keepalive(&ka);
                match tokio::net::TcpStream::from_std(sock.into()) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Falha ao recriar TcpStream com keepalive: {}", e);
                        connection_failed_client(&state).await;
                        return;
                    }
                }
            }
            Err(e) => {
                warn!("Falha ao converter TcpStream para socket2: {}", e);
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

    let server_was_trusted = {
        let mut settings = state.settings.lock().await;
        let trusted = settings.server_cert_fingerprint.is_some();
        if let Some(observed_fp) = tofu_verifier.take_observed() {
            if settings.server_cert_fingerprint.is_none() {
                settings.server_cert_fingerprint = Some(observed_fp);
                let _ = settings.save();
            }
        }
        trusted
    };

    let (screen_name, psk_hex) = {
        let s = state.settings.lock().await;
        (s.screen_name.clone(), s.psk_hex.clone())
    };

    match do_handshake(&mut tls, &screen_name, &psk_hex, server_was_trusted, &state, &cancel).await {
        HandshakeResult::Connected(peer_hostname) => {
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
            crate::ipc::emit_status_to_main(&state).await;
            state
                .user_visible_connection_success(&format!("Ligado a «{peer_hostname}» ({target})."))
                .await;
            let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
            { *state.message_tx.lock().await = Some(msg_tx); }
            run_session(&mut tls, state.clone(), &mut msg_rx, cancel).await;
            { *state.message_tx.lock().await = None; }
        }
        HandshakeResult::PskSynced => {
            // PSK atualizado — connect_to_addr não retenta, mas o próximo Conectar vai funcionar
            connection_failed_client(&state).await;
        }
        HandshakeResult::Failed => {
            state.user_visible_connection_error(
                "Movex — Conexão",
                "Handshake falhou: confira versão do Movex e rede (TLS).",
            ).await;
            connection_failed_client(&state).await;
        }
    }

    { *state.session_server_addr.lock().await = None; }
}

/// Resultado do handshake com três saídas possíveis.
enum HandshakeResult {
    /// Conectado com sucesso — hostname do peer.
    Connected(String),
    /// PSK foi auto-sincronizado; reconectar imediatamente com o novo PSK.
    PskSynced,
    /// Falha fatal — parar tentativas.
    Failed,
}

/// Executa o handshake HMAC com o servidor.
/// `server_was_trusted`: true se o certificado TLS do servidor já estava armazenado (TOFU).
async fn do_handshake<S>(
    stream: &mut S,
    screen_name: &str,
    psk_hex: &str,
    server_was_trusted: bool,
    state: &SharedState,
    cancel: &CancellationToken,
) -> HandshakeResult
where
    S: tokio::io::AsyncReadExt + tokio::io::AsyncWriteExt + Unpin,
{
    let challenge = match recv_message(stream).await {
        Ok(Message::ServerChallenge { version, server_nonce, .. }) => {
            if version != PROTOCOL_VERSION {
                warn!("Versão incompatível do servidor");
                return HandshakeResult::Failed;
            }
            server_nonce
        }
        Ok(_) => { warn!("Esperava ServerChallenge"); return HandshakeResult::Failed; }
        Err(e) => { warn!("Erro ao receber ServerChallenge: {}", e); return HandshakeResult::Failed; }
    };

    let hmac = match crate::core::auth::compute_hmac(psk_hex, &challenge) {
        Ok(h) => h,
        Err(e) => {
            warn!("PSK inválida — não é possível calcular HMAC: {}", e);
            state.user_visible_connection_error(
                "Movex — Chave de segurança inválida",
                "A chave de segurança (PSK) configurada não é hexadecimal válida. Regenere a chave nas Configurações.",
            ).await;
            return HandshakeResult::Failed;
        }
    };
    if let Err(e) = send_message(stream, &Message::Hello {
        version: PROTOCOL_VERSION,
        hostname: screen_name.to_string(),
        hmac,
    }).await {
        warn!("Erro ao enviar Hello: {}", e);
        return HandshakeResult::Failed;
    }

    let first_msg = recv_message(stream).await;

    let resolved = match first_msg {
        Ok(Message::ConnectionPending { hostname: server_name }) => {
            info!("Aguardando aprovação do servidor '{}'...", server_name);
            {
                let mut status = state.connection_status.lock().await;
                *status = ConnectionStatus::Connecting;
            }
            if cancel.is_cancelled() { return HandshakeResult::Failed; }
            recv_message(stream).await
        }
        other => other,
    };

    match resolved {
        Ok(Message::HelloPskSync { new_psk_hex }) => {
            if server_was_trusted {
                // Canal TLS já autenticado por TOFU — seguro receber o PSK pelo canal cifrado.
                info!("PSK sync recebido do servidor confiável — atualizando e reconectando");
                {
                    let mut s = state.settings.lock().await;
                    s.psk_hex = new_psk_hex;
                    let _ = s.save();
                }
                state.log_to_connection_panel(
                    "info",
                    "PSK sincronizado automaticamente com o servidor — reconectando...",
                ).await;
                HandshakeResult::PskSynced
            } else {
                warn!("HelloPskSync recebido mas certificado do servidor não é confiável — ignorado por segurança");
                state.user_visible_connection_error(
                    "Movex — PSK incorreta",
                    "Chave de segurança (PSK) incorreta. Configure o mesmo PSK nos dois PCs.",
                ).await;
                HandshakeResult::Failed
            }
        }
        Ok(Message::HelloReject { reason }) => {
            warn!("Servidor rejeitou o handshake: {}", reason);
            state
                .user_visible_connection_error("Movex — Nome do ecrã", &reason)
                .await;
            HandshakeResult::Failed
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
            HandshakeResult::Failed
        }
        Ok(Message::HelloAck { hostname: peer, .. }) => HandshakeResult::Connected(peer),
        Ok(Message::ConnectionApproved) => {
            info!("Conexão aprovada pelo servidor!");
            match recv_message(stream).await {
                Ok(Message::HelloAck { hostname: peer, .. }) => HandshakeResult::Connected(peer),
                Ok(Message::HelloReject { reason }) => {
                    warn!("Rejeitado após aprovação: {}", reason);
                    HandshakeResult::Failed
                }
                Ok(_) => { warn!("Esperava HelloAck após aprovação"); HandshakeResult::Failed }
                Err(e) => { warn!("Erro ao receber HelloAck: {}", e); HandshakeResult::Failed }
            }
        }
        Ok(other) => {
            warn!("Mensagem inesperada durante handshake: {:?}", other);
            HandshakeResult::Failed
        }
        Err(e) => {
            warn!("Erro durante handshake: {}", e);
            HandshakeResult::Failed
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
        crate::ipc::emit_status_to_main(&state).await;

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
                        let server_was_trusted = {
                            let mut settings = state.settings.lock().await;
                            let trusted = settings.server_cert_fingerprint.is_some();
                            if let Some(observed_fp) = tofu_verifier.take_observed() {
                                if settings.server_cert_fingerprint.is_none() {
                                    info!("TOFU: gravando fingerprint do servidor");
                                    settings.server_cert_fingerprint = Some(observed_fp);
                                    let _ = settings.save();
                                }
                            }
                            trusted
                        };

                        let (screen_name, psk_hex) = {
                            let s = state.settings.lock().await;
                            (s.screen_name.clone(), s.psk_hex.clone())
                        };

                        match do_handshake(&mut tls, &screen_name, &psk_hex, server_was_trusted, &state, &cancel).await {
                            HandshakeResult::Connected(peer_hostname) => {
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
                                crate::ipc::emit_status_to_main(&state).await;
                                state
                                    .user_visible_connection_success(&format!(
                                        "Ligado ao servidor «{peer_hostname}» ({addr})."
                                    ))
                                    .await;

                                let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(256);
                                { *state.message_tx.lock().await = Some(msg_tx); }
                                run_session(&mut tls, state.clone(), &mut msg_rx, cancel.clone()).await;
                                { *state.message_tx.lock().await = None; }
                            }
                            HandshakeResult::PskSynced => {
                                // PSK atualizado — reconectar imediatamente sem espera
                                info!("PSK sincronizado — reconectando imediatamente");
                                policy.reset();
                                continue;
                            }
                            HandshakeResult::Failed => {
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
                crate::ipc::emit_status_to_main(&state).await;
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
                crate::ipc::emit_status_to_main(&state).await;
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
        crate::ipc::emit_status_to_main(&state).await;
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

    let peer_from_settings = {
        let s = state.settings.lock().await;
        match s.peer_position {
            crate::config::ScreenPosition::Left  => PeerPosition::Left,
            crate::config::ScreenPosition::Above => PeerPosition::Above,
            crate::config::ScreenPosition::Below => PeerPosition::Below,
            _                                    => PeerPosition::Right,
        }
    };

    let monitors = crate::screen::layout::detect_monitors();
    let (_, _, bbox_w, bbox_h) = monitors.bounding_box();
    let primary = monitors
        .monitors
        .iter()
        .find(|m| m.is_primary)
        .or_else(|| monitors.monitors.first());
    let scale = primary.map(|m| m.scale_factor).unwrap_or(1.0_f32);

    let local_resolution = ScreenResolution {
        width: bbox_w,
        height: bbox_h,
        scale_factor: scale,
    };

    // `client_return_peer_pos` é derivado automaticamente da posição de entrada
    // enviada pelo servidor (entry_x / entry_y no primeiro MouseMove após EnterScreen).
    // Isso elimina a dependência das configurações locais do cliente, que poderiam
    // estar configuradas de forma assimétrica em relação ao servidor.
    //
    // Fallback: peer_from_settings.invert() para compatibilidade quando a posição
    // de entrada ainda não foi recebida.
    let fallback_peer_pos = peer_from_settings.invert();
    let mut client_return_peer_pos: Option<PeerPosition> = None;

    tracing::debug!(
        "Cliente: fallback borda de retorno {:?} (desktop virtual {}x{}) — será substituído pela posição de entrada",
        fallback_peer_pos,
        bbox_w,
        bbox_h,
    );

    let mut prev_in_return_strip = false;
    // macOS: sem Acessibilidade o `CGEvent::post` em `inject_event` é descartado
    // silenciosamente — o utilizador vê EnterScreen no log mas o cursor não se mexe.
    // Avisamos uma única vez por sessão, na primeira injeção pendente.
    let mut permission_warning_emitted = false;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = send_message(stream, &Message::Disconnect {
                    reason: "cliente encerrado".into(),
                }).await;
                break;
            }

            Some(msg) = msg_rx.recv() => {
                match send_message(stream, &msg).await {
                    Ok(n) => { state.stats.add_sent(n as u64); }
                    Err(e) => {
                        warn!("Erro ao enviar para servidor: {}", e);
                        break;
                    }
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

            result = recv_message_counted(stream) => {
                let (msg, recv_bytes) = match result {
                    Ok(pair) => pair,
                    Err(e) => {
                        warn!("Erro na sessão: {}", e);
                        break;
                    }
                };
                state.stats.add_received(recv_bytes as u64);
                match msg {
                    Message::EnterScreen => {
                        // Inicializar como `true` para suprimir o LeaveScreen imediato:
                        // o cursor chega na borda de entrada (ex: x=0 para Right), que já
                        // é a strip de retorno. Se deixarmos `false`, o primeiro MouseMove
                        // dispara edge_enter=true e devolve o cursor ao servidor antes de
                        // o utilizador ver qualquer coisa no PC remoto.
                        prev_in_return_strip = true;
                        // Resetar: a borda de retorno será re-derivada do primeiro MouseMove
                        client_return_peer_pos = None;
                        state.active_screen_remote.store(true, Ordering::Release);
                        {
                            let mut active = state.active_screen.lock().await;
                            *active = ActiveScreen::Remote;
                        }
                        info!("Cursor entrou nesta máquina (modo remoto activado)");
                        crate::ipc::emit_status_to_main(&state).await;
                    }
                    Message::LeaveScreen => {
                        state.active_screen_remote.store(false, Ordering::Release);
                        {
                            let mut active = state.active_screen.lock().await;
                            *active = ActiveScreen::Local;
                        }
                        info!("Cursor voltou a esta máquina (modo local)");
                        crate::ipc::emit_status_to_main(&state).await;
                    }
                    Message::Input(event) => {
                        if !state.active_screen_remote.load(Ordering::Acquire) {
                            // Cursor não está neste PC — descartar evento.
                            // Ocorre durante a janela de race entre enviar LeaveScreen
                            // e o servidor parar de encaminhar; sem este guard o evento
                            // seria injetado no Mac enquanto o cursor está no Windows.
                            prev_in_return_strip = false;
                            client_return_peer_pos = None;
                            continue;
                        } else if let InputEvent::MouseMove { x, y } = &event {
                            // Derivar borda de retorno automaticamente a partir da posição
                            // de entrada (primeiro MouseMove após EnterScreen).
                            // entry_x ≈ 0 → cursor entrou pela esquerda → retorno pela esquerda
                            // entry_x ≈ 1 → cursor entrou pela direita  → retorno pela direita
                            // entry_y ≈ 0 → cursor entrou por cima      → retorno por cima
                            // entry_y ≈ 1 → cursor entrou por baixo     → retorno por baixo
                            if client_return_peer_pos.is_none() {
                                let derived = derive_return_edge_from_entry(*x, *y);
                                tracing::info!(
                                    "Borda de retorno derivada da entrada ({:.3},{:.3}): {:?}",
                                    x, y, derived
                                );
                                client_return_peer_pos = Some(derived);
                            }

                            let return_pos = client_return_peer_pos.unwrap_or(fallback_peer_pos);
                            let return_layout = ScreenLayout {
                                local: local_resolution,
                                peer: None,
                                peer_position: return_pos,
                            };

                            let px = x * return_layout.local.width as f32;
                            let py = y * return_layout.local.height as f32;
                            let in_strip = matches!(
                                check_boundary(px, py, &return_layout),
                                BoundaryResult::CrossedToPeer { .. }
                            );
                            let edge_enter = in_strip && !prev_in_return_strip;
                            prev_in_return_strip = in_strip;
                            if edge_enter {
                                info!(
                                    "Borda de retorno ao servidor (return_pos={:?}) — enviando LeaveScreen",
                                    return_pos
                                );
                                let mtx = state.message_tx.lock().await;
                                if let Some(tx) = mtx.as_ref() {
                                    let _ = tx.try_send(Message::LeaveScreen);
                                }
                                drop(mtx);
                                state.active_screen_remote.store(false, Ordering::Release);
                                client_return_peer_pos = None;
                                let mut active = state.active_screen.lock().await;
                                *active = ActiveScreen::Local;
                                drop(active);
                                crate::ipc::emit_status_to_main(&state).await;
                            }
                        } else {
                            prev_in_return_strip = false;
                        }
                        if !permission_warning_emitted
                            && !crate::permissions::macos_accessibility_trusted()
                        {
                            permission_warning_emitted = true;
                            state.user_visible_connection_error(
                                "Movex — Permissão de Acessibilidade",
                                "O outro PC está a enviar o mouse, mas o macOS está a bloquear a injeção. Abra Ajustes do Sistema → Privacidade e Segurança → Acessibilidade, ative Movex e feche/reabra o app.",
                            ).await;
                        }
                        // Só injetar se o cursor ainda está nesta máquina — impede
                        // injeção do MouseMove de borda que disparou edge_enter=true.
                        if state.active_screen_remote.load(Ordering::Acquire) {
                            inject_event(event);
                        }
                    }
                    ref msg @ Message::ClipboardData { .. } => {
                        crate::clipboard::sync::apply_clipboard_message(msg);
                        // Atualizar cache com hash do conteúdo recebido para evitar reenvio
                        // imediato (dados binários como PNG não são comparáveis por texto)
                        if let Message::ClipboardData { ref mime, ref data } = *msg {
                            let hash = crate::core::utils::crc32(data);
                            last_clipboard = Some(format!("{}:{}", mime.split(';').next().unwrap_or(mime), hash));
                        }
                    }
                    Message::FileStart { id, name, size } => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_start(id, name, size).await.unwrap_or_else(|e| warn!("FileStart: {}", e));
                        }
                    }
                    Message::FileChunk { id, seq, data } => {
                        if let Some(ref mut recv) = file_receiver {
                            recv.on_file_chunk(id, seq, data).await.unwrap_or_else(|e| warn!("FileChunk: {}", e));
                        }
                    }
                    Message::FileEnd { id, checksum } => {
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
                    Message::FileRetry { id } => {
                        warn!("Peer solicitou reenvio do arquivo id={}", id);
                    }
                    Message::Ping => {
                        let _ = send_message(stream, &Message::Pong).await;
                    }
                    Message::Pong => {
                        if let Some(sent) = ping_sent_at.take() {
                            let rtt = sent.elapsed().as_millis() as u32;
                            let mut status = state.connection_status.lock().await;
                            if let ConnectionStatus::Connected { ref mut latency_ms, .. } = *status {
                                *latency_ms = rtt;
                            }
                            drop(status);
                            crate::ipc::emit_status_to_main(&state).await;
                        }
                    }
                    Message::Disconnect { reason } => {
                        info!("Servidor desconectou: {}", reason);
                        break;
                    }
                    _ => {}
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
    crate::ipc::emit_status_to_main(&state).await;
}

/// Deriva a borda de retorno ao servidor a partir da posição de entrada do cursor.
///
/// O servidor envia `entry_x = 0.0` quando o cursor entra pela borda ESQUERDA do
/// cliente, e `entry_x = 1.0` quando entra pela borda DIREITA. Analogamente para
/// `entry_y`. Esta função inverte essa lógica: a borda de retorno é a borda pela
/// qual o cursor entrou.
///
/// Isso elimina a dependência da configuração local `peer_position` do cliente,
/// que poderia estar configurada de forma assimétrica em relação ao servidor.
fn derive_return_edge_from_entry(entry_x: f32, entry_y: f32) -> PeerPosition {
    const EDGE_MARGIN: f32 = 0.05;
    if entry_x < EDGE_MARGIN {
        PeerPosition::Left
    } else if entry_x > 1.0 - EDGE_MARGIN {
        PeerPosition::Right
    } else if entry_y < EDGE_MARGIN {
        PeerPosition::Above
    } else {
        PeerPosition::Below
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_return_edge_entrada_esquerda() {
        assert_eq!(derive_return_edge_from_entry(0.0, 0.5), PeerPosition::Left);
    }

    #[test]
    fn derive_return_edge_entrada_direita() {
        assert_eq!(derive_return_edge_from_entry(1.0, 0.5), PeerPosition::Right);
    }

    #[test]
    fn derive_return_edge_entrada_cima() {
        assert_eq!(derive_return_edge_from_entry(0.5, 0.0), PeerPosition::Above);
    }

    #[test]
    fn derive_return_edge_entrada_baixo() {
        assert_eq!(derive_return_edge_from_entry(0.5, 1.0), PeerPosition::Below);
    }

    #[test]
    fn derive_return_edge_configura_assimetricamente_funciona() {
        // Servidor=Right (cursor entra pela esquerda do cliente, entry_x=0)
        // Mesmo que o cliente tenha peer_position=Left (configuração "intuitiva"),
        // a derivação devolve Left (correto) independentemente das settings.
        let derived = derive_return_edge_from_entry(0.0, 0.3);
        assert_eq!(derived, PeerPosition::Left,
            "Cursor que entrou pela esquerda deve retornar pela esquerda");
    }

    // Validar mapeamento completo com os valores exactos que o servidor envia:
    // boundary.rs: Right  → entry_x=0.0 | Left  → entry_x=1.0
    //              Below  → entry_y=0.0 | Above → entry_y=1.0

    #[test]
    fn servidor_peer_right_envia_entry_x_zero_retorna_left() {
        // Servidor peer_position=Right → cursor sai pela direita do servidor
        // → entra no cliente pela esquerda → entry_x=0.0
        assert_eq!(derive_return_edge_from_entry(0.0, 0.4), PeerPosition::Left);
    }

    #[test]
    fn servidor_peer_left_envia_entry_x_um_retorna_right() {
        assert_eq!(derive_return_edge_from_entry(1.0, 0.6), PeerPosition::Right);
    }

    #[test]
    fn servidor_peer_below_envia_entry_y_zero_retorna_above() {
        assert_eq!(derive_return_edge_from_entry(0.5, 0.0), PeerPosition::Above);
    }

    #[test]
    fn servidor_peer_above_envia_entry_y_um_retorna_below() {
        assert_eq!(derive_return_edge_from_entry(0.3, 1.0), PeerPosition::Below);
    }

    #[test]
    fn canto_superior_esquerdo_prioridade_x() {
        // entry_x=0.0 E entry_y=0.0: o servidor sempre envia UMA coordenada exacta
        // (0.0 ou 1.0), portanto entrada no canto é via borda X (Left) quando entry_x=0
        assert_eq!(derive_return_edge_from_entry(0.0, 0.0), PeerPosition::Left);
    }
}
