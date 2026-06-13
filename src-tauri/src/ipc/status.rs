use tauri::State;

use crate::core::state::{ConnectionStatus, SharedState};

#[derive(serde::Serialize, Clone)]
pub struct StatusPayload {
    pub connected: bool,
    /// `true` quando há sessão ativa: à escuta, a ligar, ligado ou a reconectar (não parado).
    pub in_session: bool,
    pub status_text: String,
    pub peer_hostname: Option<String>,
    /// Endereço do peer quando conectado (ex.: `192.168.1.10:24800`).
    pub peer_addr: Option<String>,
    pub latency_ms: Option<u32>,
    pub active_screen: String,
    pub uptime_secs: u64,
}

#[derive(serde::Serialize, Clone)]
pub struct SettingsPayload {
    pub hostname: String,
    pub screen_name: String,
    pub expected_client_screen_name: Option<String>,
    pub launch_connection_on_startup: bool,
    pub role: String,
    pub server_addr: Option<String>,
    pub port: u16,
    /// Apenas os primeiros 8 caracteres são enviados ao frontend para exibição/confirmação.
    /// O valor completo nunca deixa o processo Rust — reduz exposição via WebView/XSS.
    pub psk_hex: String,
    pub peer_position: String,
    pub autostart: bool,
    pub theme: String,
    pub setup_complete: bool,
    pub notifications_enabled: bool,
    pub lock_key: String,
    pub clipboard_sync_enabled: bool,
    pub recent_peers: Vec<crate::config::settings::RecentPeer>,
    pub lock_mode: bool,
}

pub(crate) async fn build_status_payload(state: &SharedState) -> StatusPayload {
    let status = state.connection_status.lock().await.clone();
    let active = state.active_screen.lock().await.clone();

    let (connected, peer_hostname, peer_addr, latency_ms) = match &status {
        ConnectionStatus::Connected {
            peer_hostname,
            peer_addr,
            latency_ms,
        } => (
            true,
            Some(peer_hostname.clone()),
            Some(peer_addr.clone()),
            Some(*latency_ms),
        ),
        _ => (false, None, None, None),
    };

    let uptime_secs = state
        .session_started_at
        .lock()
        .await
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    let in_session = !matches!(&status, ConnectionStatus::Disconnected);
    // Texto vazio em `Disconnected` evita que a UI trate a palavra "Desconectado" como estado
    // e use `connected` + fallback; em combinação com `in_session` os botões refletem Listening/Connecting.
    let status_text = match &status {
        ConnectionStatus::Disconnected => String::new(),
        _ => status.to_string(),
    };

    StatusPayload {
        connected,
        in_session,
        status_text,
        peer_hostname,
        peer_addr,
        latency_ms,
        active_screen: format!("{:?}", active),
        uptime_secs,
    }
}

#[tauri::command]
pub async fn get_status(state: State<'_, SharedState>) -> Result<StatusPayload, String> {
    Ok(build_status_payload(&state).await)
}

#[tauri::command]
pub async fn get_settings(state: State<'_, SharedState>) -> Result<SettingsPayload, String> {
    let s = state.settings.lock().await;
    // Expor apenas os primeiros 8 hex chars (4 bytes) para confirmação visual na UI.
    // Fatiar por chars (não por bytes) evita panic se a PSK contiver chars multibyte.
    let psk_preview: String = s.psk_hex.chars().take(8).collect();
    let psk_preview = format!("{}...", psk_preview);
    Ok(SettingsPayload {
        hostname: s.hostname.clone(),
        screen_name: s.screen_name.clone(),
        expected_client_screen_name: s.expected_client_screen_name.clone(),
        launch_connection_on_startup: s.launch_connection_on_startup,
        role: format!("{:?}", s.role).to_lowercase(),
        server_addr: s.server_addr.clone(),
        port: s.port,
        psk_hex: psk_preview,
        peer_position: s.peer_position.to_string(),
        autostart: s.autostart,
        theme: s.theme.clone(),
        setup_complete: s.setup_complete,
        notifications_enabled: s.notifications_enabled,
        lock_key: s.lock_key.clone(),
        clipboard_sync_enabled: s.clipboard_sync_enabled,
        recent_peers: s.recent_peers.clone(),
        lock_mode: state.lock_mode.load(std::sync::atomic::Ordering::Relaxed),
    })
}
