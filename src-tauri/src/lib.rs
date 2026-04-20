mod clipboard;
mod config;
mod core;
mod input;
mod network;
mod screen;
mod transfer;

use std::sync::Arc;
use tauri::State;

use crate::config::{Role, Settings};
use crate::core::state::{AppState, ConnectionStatus, SharedState};

// ── Payloads IPC ──────────────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
pub struct StatusPayload {
    pub connected: bool,
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

#[derive(serde::Serialize, Clone)]
pub struct PeerInfo {
    pub hostname: String,
    pub addr: String,
    pub port: u16,
}

async fn build_status_payload(state: &SharedState) -> StatusPayload {
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
        ConnectionStatus::PendingApproval { peer_hostname } => {
            (false, Some(peer_hostname.clone()), None, None)
        }
        _ => (false, None, None, None),
    };

    let uptime_secs = state
        .session_started_at
        .lock()
        .await
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    StatusPayload {
        connected,
        status_text: status.to_string(),
        peer_hostname,
        peer_addr,
        latency_ms,
        active_screen: format!("{:?}", active),
        uptime_secs,
    }
}

/// Emite estado para todas as janelas (evita falha silenciosa se o label da janela não for `main`).
pub(crate) async fn emit_status_to_main(state: &SharedState) {
    use tauri::Emitter;
    let app = { state.app_handle.lock().await.clone() };
    let Some(app) = app else {
        return;
    };
    let payload = build_status_payload(state).await;
    let _ = app.emit("movex://status-changed", &payload);
}

// ── Comandos IPC ──────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_status(state: State<'_, SharedState>) -> Result<StatusPayload, String> {
    Ok(build_status_payload(&state).await)
}

#[tauri::command]
async fn get_settings(state: State<'_, SharedState>) -> Result<SettingsPayload, String> {
    let s = state.settings.lock().await;
    // Expor apenas os primeiros 8 hex chars (4 bytes) para confirmação visual na UI
    let psk_preview = format!("{}...", &s.psk_hex[..s.psk_hex.len().min(8)]);
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

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn save_settings(
    state: State<'_, SharedState>,
    hostname: String,
    screen_name: String,
    expected_client_screen_name: Option<String>,
    launch_connection_on_startup: bool,
    role: String,
    server_addr: Option<String>,
    port: u16,
    psk_hex: String,
    peer_position: String,
    autostart: bool,
    theme: String,
) -> Result<(), String> {
    let autostart_changed;
    let autostart_enable;
    {
        let mut s = state.settings.lock().await;
        autostart_changed = s.autostart != autostart;
        autostart_enable = autostart;
        s.hostname = hostname;
        let sn = screen_name.trim();
        s.screen_name = if sn.is_empty() {
            s.hostname.clone()
        } else {
            sn.to_string()
        };
        s.expected_client_screen_name = expected_client_screen_name
            .as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty());
        s.launch_connection_on_startup = launch_connection_on_startup;
        s.role = if role == "server" { Role::Server } else { Role::Client };
        s.server_addr = server_addr;
        s.port = port;
        // Não sobrescrever a chave se o frontend enviou o preview truncado ("xxxx...")
        if !psk_hex.ends_with("...") && psk_hex.len() >= 16 {
            s.psk_hex = psk_hex;
        }
        s.peer_position = crate::config::ScreenPosition::from(peer_position.as_ref());
        s.autostart = autostart;
        s.theme = theme;
        s.setup_complete = true;
        s.save()?;
    }
    // Aplicar autostart imediatamente se mudou
    if autostart_changed {
        if autostart_enable {
            config::autostart::enable().unwrap_or_else(|e| {
                tracing::warn!("Erro ao ativar autostart: {}", e)
            });
        } else {
            config::autostart::disable().unwrap_or_else(|e| {
                tracing::warn!("Erro ao desativar autostart: {}", e)
            });
        }
    }
    Ok(())
}

#[tauri::command]
async fn complete_setup(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.setup_complete = true;
    s.save()
}

#[tauri::command]
async fn start_connection(state: State<'_, SharedState>) -> Result<(), String> {
    // Cancelar conexão anterior se existir
    state.cancel_connection().await;

    let role = { state.settings.lock().await.role.clone() };
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();

    match role {
        Role::Server => {
            tokio::spawn(async move {
                if let Err(e) = core::server::start(state_clone, cancel).await {
                    tracing::error!("Servidor encerrou com erro: {}", e);
                }
            });
        }
        Role::Client => {
            tokio::spawn(async move {
                core::client::connect(state_clone, cancel).await;
            });
        }
    }
    Ok(())
}

#[tauri::command]
async fn disconnect(state: State<'_, SharedState>) -> Result<(), String> {
    // Enviar Disconnect ao peer se conectado
    if let Some(tx) = state.message_tx.lock().await.as_ref() {
        let _ = tx.try_send(crate::network::protocol::Message::Disconnect {
            reason: "usuário desconectou".into(),
        });
    }
    // Cancelar task de conexão
    state.cancel_connection().await;
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }
    tracing::info!("Desconectado pelo usuário");
    Ok(())
}

#[tauri::command]
async fn reset_settings(state: State<'_, SharedState>) -> Result<(), String> {
    state.cancel_connection().await;
    let new_settings = Settings::default();
    new_settings.save()?;
    {
        let mut s = state.settings.lock().await;
        *s = new_settings;
    }
    let mut status = state.connection_status.lock().await;
    *status = ConnectionStatus::Disconnected;
    tracing::info!("Configurações resetadas");
    Ok(())
}

#[tauri::command]
async fn set_role(state: State<'_, SharedState>, role: String) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.role = if role == "server" { Role::Server } else { Role::Client };
    s.save()
}

#[tauri::command]
async fn set_server_addr(state: State<'_, SharedState>, addr: Option<String>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.server_addr = addr;
    s.save()
}

/// Descobre servidores Movex na rede local via mDNS (timeout: 3s)
/// usa spawn_blocking porque discover_peers internamente faz I/O síncrono
#[tauri::command]
async fn discover_peers() -> Result<Vec<PeerInfo>, String> {
    let peers = network::discovery::discover_peers(3).await;
    Ok(peers.into_iter().map(|p| PeerInfo {
        hostname: p.hostname,
        addr: p.addr,
        port: p.port,
    }).collect())
}

/// IPv4 desta máquina na LAN (para exibir em “Conectar rede” / diagnóstico).
#[tauri::command]
fn get_local_ipv4_addrs() -> Vec<String> {
    network::local_addrs::local_ipv4_strings()
}

/// Envia um arquivo ao peer conectado
#[tauri::command]
async fn send_file_to_peer(
    state: State<'_, SharedState>,
    path: String,
) -> Result<(), String> {
    use std::path::Path;

    let tx = state.message_tx.lock().await.clone()
        .ok_or_else(|| "Não há conexão ativa".to_string())?;

    let transfer_id = state.next_transfer_id().await;

    // Canonicalizar path para evitar path traversal (consistente com drop_file_to_peer)
    let path = match tokio::fs::canonicalize(Path::new(&path)).await {
        Ok(p) => p,
        Err(e) => return Err(format!("Path inválido: {}", e)),
    };
    if path.is_dir() {
        return Err("Não é possível enviar diretórios".to_string());
    }

    let state_clone = state.inner().clone();

    tokio::spawn(async move {
        let file_name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let file_size = match tokio::fs::metadata(&path).await {
            Ok(m) => m.len(),
            Err(e) => {
                tracing::error!("Erro ao ler arquivo '{}': {}", path.display(), e);
                return;
            }
        };

        // Registrar progresso
        {
            let mut transfers = state_clone.transfers.lock().await;
            transfers.insert(transfer_id, crate::transfer::TransferProgress {
                id: transfer_id,
                name: file_name.clone(),
                total_bytes: file_size,
                sent_bytes: 0,
                direction: crate::transfer::TransferDirection::Sending,
            });
        }

        tracing::info!("Enviando '{}' ({} bytes) ao peer...", file_name, file_size);

        // Enviar via canal: ler arquivo em chunks e enfileirar mensagens
        match send_file_via_channel(
            &path,
            transfer_id,
            file_size,
            file_name.clone(),
            &tx,
            state_clone.transfers.clone(),
        ).await {
            Ok(_) => {
                tracing::info!("Arquivo '{}' enviado com sucesso", file_name);
                state_clone.transfers.lock().await.remove(&transfer_id);
            }
            Err(e) => {
                tracing::error!("Erro ao enviar '{}': {}", file_name, e);
                state_clone.transfers.lock().await.remove(&transfer_id);
            }
        }
    });

    Ok(())
}

/// Lê arquivo em chunks e enfileira mensagens; atualiza `sent_bytes` para barra de progresso
async fn send_file_via_channel(
    path: &std::path::Path,
    id: u32,
    size: u64,
    name: String,
    tx: &tokio::sync::mpsc::Sender<crate::network::protocol::Message>,
    transfers: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<u32, crate::transfer::TransferProgress>>>,
) -> Result<(), String> {
    use tokio::io::AsyncReadExt;
    use sha2::{Digest, Sha256};

    tx.send(crate::network::protocol::Message::FileStart { id, name, size })
        .await.map_err(|e| e.to_string())?;

    let mut file = tokio::fs::File::open(path).await.map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut seq = 0u32;
    let mut buf = vec![0u8; crate::network::protocol::FILE_CHUNK_SIZE];
    let mut sent: u64 = 0;

    loop {
        let n = file.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 { break; }
        let chunk = buf[..n].to_vec();
        hasher.update(&chunk);
        sent += n as u64;

        // Atualizar progresso para o frontend
        if let Ok(mut map) = transfers.try_lock() {
            if let Some(p) = map.get_mut(&id) {
                p.sent_bytes = sent;
            }
        }

        tx.send(crate::network::protocol::Message::FileChunk { id, seq, data: chunk })
            .await.map_err(|e| e.to_string())?;
        seq += 1;
    }

    let checksum: [u8; 32] = hasher.finalize().into();
    tx.send(crate::network::protocol::Message::FileEnd { id, checksum })
        .await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Retorna o hostname do cliente aguardando aprovação (None = nenhum)
#[tauri::command]
async fn get_pending_approval(state: State<'_, SharedState>) -> Result<Option<String>, String> {
    Ok(state.pending_approval.lock().await.clone())
}

/// Aprova a conexão pendente
#[tauri::command]
async fn approve_connection(state: State<'_, SharedState>) -> Result<(), String> {
    let tx = state.approval_tx.lock().await.take()
        .ok_or_else(|| "Nenhuma conexão aguardando aprovação".to_string())?;
    tx.send(true).map_err(|_| "Erro ao enviar aprovação".to_string())?;
    tracing::info!("Conexão aprovada pelo usuário");
    Ok(())
}

/// Rejeita a conexão pendente
#[tauri::command]
async fn reject_connection(state: State<'_, SharedState>) -> Result<(), String> {
    let tx = state.approval_tx.lock().await.take()
        .ok_or_else(|| "Nenhuma conexão aguardando aprovação".to_string())?;
    tx.send(false).map_err(|_| "Erro ao enviar rejeição".to_string())?;
    tracing::info!("Conexão rejeitada pelo usuário");
    Ok(())
}

/// Retorna lista de transferências em andamento
#[tauri::command]
async fn get_transfers(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::transfer::TransferProgress>, String> {
    let transfers = state.transfers.lock().await;
    Ok(transfers.values().cloned().collect())
}

/// Conecta diretamente a um peer descoberto via mDNS.
/// O endereço é armazenado apenas na sessão — não sobrescreve settings persistidas.
#[tauri::command]
async fn connect_to_peer(
    state: State<'_, SharedState>,
    addr: String,
    port: u16,
) -> Result<(), String> {
    // Guardar endereço de sessão sem persistir
    { *state.session_server_addr.lock().await = Some((addr.clone(), port)); }

    state.cancel_connection().await;
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        core::client::connect_to_addr(state_clone, addr, port, cancel).await;
    });
    Ok(())
}

// ── Novos comandos IPC ────────────────────────────────────────────────────────

/// Toggle do modo lock (pausar/retomar transição de cursor) — operação atômica
#[tauri::command]
async fn toggle_lock(state: State<'_, SharedState>) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    // fetch_xor é atômico — evita race condition entre atalho global e comando IPC
    let was_locked = state.lock_mode.fetch_xor(true, Ordering::AcqRel);
    let now_locked = !was_locked;
    tracing::info!("Modo lock: {}", if now_locked { "ATIVO" } else { "INATIVO" });
    Ok(now_locked)
}

/// Retorna estatísticas da sessão atual
#[tauri::command]
async fn get_stats(state: State<'_, SharedState>) -> Result<crate::core::stats::StatsSnapshot, String> {
    Ok(state.stats.snapshot())
}

/// Retorna resolução real do monitor principal
#[tauri::command]
fn get_screen_resolution() -> (u32, u32) {
    crate::core::stats::get_primary_screen_size()
}

/// Troca papel E reinicia conexão automaticamente
#[tauri::command]
async fn switch_role(state: State<'_, SharedState>, role: String) -> Result<(), String> {
    // Desconectar sessão ativa
    if let Some(tx) = state.message_tx.lock().await.as_ref() {
        let _ = tx.try_send(crate::network::protocol::Message::Disconnect {
            reason: "troca de papel".into(),
        });
    }
    state.cancel_connection().await;
    {
        let mut status = state.connection_status.lock().await;
        *status = crate::core::state::ConnectionStatus::Disconnected;
    }

    // Atualizar papel
    {
        let mut s = state.settings.lock().await;
        s.role = if role == "server" { Role::Server } else { Role::Client };
        s.save()?;
    }

    // Relançar com o novo papel
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();
    let role_enum = if role == "server" { Role::Server } else { Role::Client };
    match role_enum {
        Role::Server => {
            tokio::spawn(async move {
                if let Err(e) = core::server::start(state_clone, cancel).await {
                    tracing::error!("Servidor: {}", e);
                }
            });
        }
        Role::Client => {
            tokio::spawn(async move {
                core::client::connect(state_clone, cancel).await;
            });
        }
    }
    tracing::info!("Papel trocado para: {}", role);
    Ok(())
}

/// Atualizar preferências — re-registra atalho global se mudou.
#[tauri::command]
async fn update_preferences(
    app: tauri::AppHandle,
    state: State<'_, SharedState>,
    notifications_enabled: bool,
    lock_key: String,
    clipboard_sync_enabled: bool,
    theme: String,
) -> Result<(), String> {
    // Validar formato do atalho antes de persistir — evitar estado inconsistente
    if lock_key.trim().is_empty() {
        return Err("Atalho de teclado não pode ser vazio".to_string());
    }

    let old_key = { state.settings.lock().await.lock_key.clone() };

    // Testar se o novo atalho é reconhecido pelo plugin antes de salvar
    if old_key != lock_key {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let state_for_new = state.inner().clone();
        let register_result = app.global_shortcut().on_shortcut(
            lock_key.as_str(),
            move |_app, _shortcut, event| {
                use tauri_plugin_global_shortcut::ShortcutState;
                if event.state == ShortcutState::Pressed {
                    use std::sync::atomic::Ordering;
                    let was = state_for_new.lock_mode.fetch_xor(true, Ordering::AcqRel);
                    tracing::info!("Atalho: modo lock → {}", !was);
                }
            },
        );

        match register_result {
            Ok(_) => {
                // Novo atalho registrado com sucesso — remover o antigo
                if let Err(e) = app.global_shortcut().unregister(old_key.as_str()) {
                    tracing::warn!("Falha ao remover atalho antigo '{}': {}", old_key, e);
                }
                tracing::info!("Atalho global atualizado: '{}'", lock_key);
            }
            Err(e) => {
                // Atalho inválido — retornar erro sem persistir nem alterar o anterior
                return Err(format!("Atalho '{}' inválido ou já em uso: {}", lock_key, e));
            }
        }
    }

    {
        let mut s = state.settings.lock().await;
        s.notifications_enabled = notifications_enabled;
        s.lock_key = lock_key;
        s.clipboard_sync_enabled = clipboard_sync_enabled;
        s.theme = theme;
        s.save()?;
    }

    Ok(())
}

/// Retorna histórico de peers recentes
#[tauri::command]
async fn get_recent_peers(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::config::settings::RecentPeer>, String> {
    Ok(state.settings.lock().await.recent_peers.clone())
}

/// Limpa o histórico de peers
#[tauri::command]
async fn clear_recent_peers(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.recent_peers.clear();
    s.save()
}

/// Retorna lista de monitores detectados localmente
#[tauri::command]
fn get_monitors() -> Vec<crate::screen::layout::Monitor> {
    crate::screen::layout::detect_monitors().monitors
}

/// Envia arquivo ao peer via drag-and-drop ou caminho direto
#[tauri::command]
async fn drop_file_to_peer(
    state: State<'_, SharedState>,
    paths: Vec<String>,
) -> Result<u32, String> {
    let tx = state.message_tx.lock().await.clone()
        .ok_or_else(|| "Não há conexão ativa".to_string())?;

    let mut count = 0u32;
    for path_str in paths {
        let path = std::path::Path::new(&path_str).to_path_buf();

        // Canonicalizar para resolver symlinks e rejeitar path traversal
        let canonical = match tokio::fs::canonicalize(&path).await {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("drop_file_to_peer: path inválido ou inexistente: {:?}", path);
                continue;
            }
        };
        // Rejeitar diretórios e caminhos com ".." (pós-canonicalização)
        if canonical.is_dir() {
            tracing::warn!("drop_file_to_peer: rejeitando diretório: {:?}", canonical);
            continue;
        }
        if !canonical.is_absolute() {
            tracing::warn!("drop_file_to_peer: path não absoluto após canonicalização: {:?}", canonical);
            continue;
        }
        let path = canonical; // usar o path canonicalizado daqui em diante

        let transfer_id = state.next_transfer_id().await;
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let file_size = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);

        {
            let mut transfers = state.transfers.lock().await;
            transfers.insert(transfer_id, crate::transfer::TransferProgress {
                id: transfer_id,
                name: file_name.clone(),
                total_bytes: file_size,
                sent_bytes: 0,
                direction: crate::transfer::TransferDirection::Sending,
            });
        }

        let tx_clone = tx.clone();
        let state_clone = state.inner().clone();
        tokio::spawn(async move {
            match send_file_via_channel(
                &path,
                transfer_id,
                file_size,
                file_name.clone(),
                &tx_clone,
                state_clone.transfers.clone(),
            ).await {
                Ok(_) => { state_clone.stats.inc_file_sent(); }
                Err(e) => tracing::error!("drop_file_to_peer: {}", e),
            }
            state_clone.transfers.lock().await.remove(&transfer_id);
        });
        count += 1;
    }
    Ok(count)
}

/// Verifica se há atualização disponível
#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    match app.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(Some(update)) => Ok(Some(update.version.to_string())),
                Ok(None) => Ok(None),
                Err(e) => {
                    tracing::warn!("Erro ao verificar atualizações: {}", e);
                    Ok(None)
                }
            }
        }
        Err(e) => {
            tracing::warn!("Updater não disponível: {}", e);
            Ok(None)
        }
    }
}

/// Instala a atualização disponível — retorna Err se não há update
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => {
            let version = update.version.to_string();
            update.download_and_install(|downloaded, total| {
                if let Some(t) = total {
                    tracing::info!("Update: {}/{} bytes ({:.0}%)", downloaded, t,
                        downloaded as f64 / t as f64 * 100.0);
                }
            }, || {
                tracing::info!("Update instalado — reiniciando...");
            }).await.map_err(|e| e.to_string())?;
            Ok(format!("v{} instalada com sucesso", version))
        }
        None => Err("Nenhuma atualização disponível".to_string()),
    }
}

/// Valida que color é um valor CSS seguro (hex ou nome simples)
fn is_safe_css_color(color: &str) -> bool {
    // Aceitar #RGB, #RRGGBB, #RRGGBBAA e nomes CSS simples (max 30 chars, alfanum)
    color.len() <= 30 && color.chars().all(|c| c.is_ascii_alphanumeric() || c == '#')
}

/// Ativa/desativa borda luminosa no monitor via evento Tauri (sem eval/unsafe-inline)
#[tauri::command]
async fn set_screen_border(
    app: tauri::AppHandle,
    active: bool,
    color: String,
) -> Result<(), String> {
    if !is_safe_css_color(&color) {
        return Err(format!("Cor CSS inválida: '{}'", color));
    }
    // Emitir evento para o frontend — elimina necessidade de eval() e unsafe-inline na CSP
    use tauri::{Emitter, Manager};
    // Emitir apenas para a janela principal (não afeta futuras janelas)
    if let Some(win) = app.get_webview_window("main") {
        win.emit("movex://screen-border", serde_json::json!({ "active": active, "color": color }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        // Falha esperada se já instalado em outro ponto de entrada
        tracing::debug!("CryptoProvider: {:?}", e);
    }

    crate::core::logging::init();

    let settings = Settings::load();
    let shared_state: SharedState = Arc::new(AppState::new(settings));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(shared_state.clone())
        .setup(move |app| {
            {
                let handle = app.handle().clone();
                tauri::async_runtime::block_on(async {
                    *shared_state.app_handle.lock().await = Some(handle);
                });
            }

            // ── System Tray ──────────────────────────────────────────────────
            use tauri::{
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
                menu::{Menu, MenuItem, PredefinedMenuItem},
            };

            let quit  = MenuItem::with_id(app, "quit",       "Sair",             true, None::<&str>)?;
            let show  = MenuItem::with_id(app, "show",       "Abrir Movex",      true, None::<&str>)?;
            let sep   = PredefinedMenuItem::separator(app)?;
            let lock  = MenuItem::with_id(app, "lock",       "Modo Lock",        true, None::<&str>)?;
            let disco = MenuItem::with_id(app, "disconnect", "Desconectar",      true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &sep, &lock, &disco, &sep, &quit])?;

            // Tray é opcional — não bloquear auto-start se o ícone estiver ausente
            if let Some(icon) = app.default_window_icon().cloned() {
                let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("Movex — KVM por software")
                .on_menu_event({
                    let state_tray = shared_state.clone();
                    let _ = &state_tray; // evitar warning de captura
                    move |app, event| {
                        match event.id.as_ref() {
                            "quit" => { app.exit(0); }
                            "show" => {
                                if let Some(win) = tauri::Manager::get_webview_window(app, "main") {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                            "lock" => {
                                use std::sync::atomic::Ordering;
                                let was = state_tray.lock_mode.fetch_xor(true, Ordering::AcqRel);
                                tracing::info!("Tray: modo lock {}", if !was { "ATIVO" } else { "INATIVO" });
                            }
                            "disconnect" => {
                                let s = state_tray.clone();
                                tauri::async_runtime::spawn(async move {
                                    s.cancel_connection().await;
                                    let mut status = s.connection_status.lock().await;
                                    *status = crate::core::state::ConnectionStatus::Disconnected;
                                });
                            }
                            _ => {}
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        let app = tray.app_handle();
                        if let Some(win) = tauri::Manager::get_webview_window(app, "main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;
            } else {
                tracing::warn!("Ícone padrão não encontrado — system tray desativada");
            }

            // ── Atalho global para modo lock ─────────────────────────────────
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                let state_for_shortcut = shared_state.clone();
                let lock_key = tauri::async_runtime::block_on(async {
                    state_for_shortcut.settings.lock().await.lock_key.clone()
                });
                if let Err(e) = app.global_shortcut().on_shortcut(
                    lock_key.as_str(),
                    move |_app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            use std::sync::atomic::Ordering;
                            let was = state_for_shortcut.lock_mode.fetch_xor(true, Ordering::AcqRel);
                            tracing::info!("Atalho: modo lock toggled → {}", !was);
                        }
                    },
                ) {
                    tracing::warn!("Falha ao registrar atalho global: {}", e);
                }
            }

            // ── Auto-start da conexão ao abrir o app ─────────────────────────
            {
                let setup_done = tauri::async_runtime::block_on(async {
                    shared_state.settings.lock().await.setup_complete
                });
                if setup_done {
                    let state_auto = shared_state.clone();
                    tauri::async_runtime::spawn(async move {
                        let (role, auto_launch) = {
                            let s = state_auto.settings.lock().await;
                            (s.role.clone(), s.launch_connection_on_startup)
                        };
                        if !auto_launch {
                            tracing::info!("Arranque automático da sessão desligado (estilo Barrier: use Conectar no painel).");
                            return;
                        }
                        let cancel = state_auto.new_cancel_token().await;
                        match role {
                            crate::config::Role::Server => {
                                tracing::info!("Auto-start: iniciando como Servidor...");
                                if let Err(e) = core::server::start(state_auto, cancel).await {
                                    tracing::error!("Auto-start servidor: {}", e);
                                }
                            }
                            crate::config::Role::Client => {
                                let has_addr = state_auto.settings.lock().await.server_addr.is_some();
                                if has_addr {
                                    tracing::info!("Auto-start: iniciando como Cliente...");
                                    core::client::connect(state_auto, cancel).await;
                                } else {
                                    tracing::warn!("Auto-start: modo Cliente sem server_addr — aguardando configuração do usuário");
                                }
                            }
                        }
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_settings,
            save_settings,
            complete_setup,
            start_connection,
            disconnect,
            reset_settings,
            set_role,
            set_server_addr,
            discover_peers,
            get_local_ipv4_addrs,
            connect_to_peer,
            send_file_to_peer,
            get_transfers,
            get_pending_approval,
            approve_connection,
            reject_connection,
            toggle_lock,
            get_stats,
            get_screen_resolution,
            switch_role,
            update_preferences,
            get_recent_peers,
            clear_recent_peers,
            get_monitors,
            drop_file_to_peer,
            check_for_update,
            install_update,
            set_screen_border,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar Movex");
}
