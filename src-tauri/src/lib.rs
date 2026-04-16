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
    pub latency_ms: Option<u32>,
    pub active_screen: String,
    pub uptime_secs: u64,
}

#[derive(serde::Serialize, Clone)]
pub struct SettingsPayload {
    pub hostname: String,
    pub role: String,
    pub server_addr: Option<String>,
    pub port: u16,
    pub psk_hex: String,
    pub peer_position: String,
    pub autostart: bool,
    pub theme: String,
    pub setup_complete: bool,
    pub notifications_enabled: bool,
    pub lock_key: String,
    pub recent_peers: Vec<crate::config::settings::RecentPeer>,
    pub lock_mode: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct PeerInfo {
    pub hostname: String,
    pub addr: String,
    pub port: u16,
}

// ── Comandos IPC ──────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_status(state: State<'_, SharedState>) -> Result<StatusPayload, String> {
    let status = state.connection_status.lock().await.clone();
    let active = state.active_screen.lock().await.clone();

    let (connected, peer_hostname, latency_ms) = match &status {
        ConnectionStatus::Connected { peer_hostname, latency_ms } => {
            (true, Some(peer_hostname.clone()), Some(*latency_ms))
        }
        ConnectionStatus::PendingApproval { peer_hostname } => {
            (false, Some(peer_hostname.clone()), None)
        }
        _ => (false, None, None),
    };

    let uptime_secs = state.session_started_at.lock().await
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    Ok(StatusPayload {
        connected,
        status_text: status.to_string(),
        peer_hostname,
        latency_ms,
        active_screen: format!("{:?}", active),
        uptime_secs,
    })
}

#[tauri::command]
async fn get_settings(state: State<'_, SharedState>) -> Result<SettingsPayload, String> {
    let s = state.settings.lock().await;
    Ok(SettingsPayload {
        hostname: s.hostname.clone(),
        role: format!("{:?}", s.role).to_lowercase(),
        server_addr: s.server_addr.clone(),
        port: s.port,
        psk_hex: s.psk_hex.clone(),
        peer_position: s.peer_position.to_string(),
        autostart: s.autostart,
        theme: s.theme.clone(),
        setup_complete: s.setup_complete,
        notifications_enabled: s.notifications_enabled,
        lock_key: s.lock_key.clone(),
        recent_peers: s.recent_peers.clone(),
        lock_mode: state.lock_mode.load(std::sync::atomic::Ordering::Relaxed),
    })
}

#[tauri::command]
async fn save_settings(
    state: State<'_, SharedState>,
    hostname: String,
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
        s.role = if role == "server" { Role::Server } else { Role::Client };
        s.server_addr = server_addr;
        s.port = port;
        s.psk_hex = psk_hex;
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
    let path = Path::new(&path).to_path_buf();
    let state_clone = state.inner().clone();

    tokio::spawn(async move {
        // Abrir arquivo e enviar via canal de mensagens
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
        match send_file_via_channel(&path, transfer_id, file_size, file_name.clone(), &tx).await {
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

/// Lê arquivo em chunks e enfileira mensagens no canal do peer
async fn send_file_via_channel(
    path: &std::path::Path,
    id: u32,
    size: u64,
    name: String,
    tx: &tokio::sync::mpsc::Sender<crate::network::protocol::Message>,
) -> Result<(), String> {
    use tokio::io::AsyncReadExt;
    use sha2::{Digest, Sha256};

    tx.send(crate::network::protocol::Message::FileStart { id, name, size })
        .await.map_err(|e| e.to_string())?;

    let mut file = tokio::fs::File::open(path).await.map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut seq = 0u32;
    let mut buf = vec![0u8; crate::network::protocol::FILE_CHUNK_SIZE];

    loop {
        let n = file.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 { break; }
        let chunk = buf[..n].to_vec();
        hasher.update(&chunk);
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

/// Conecta diretamente a um peer descoberto via mDNS
#[tauri::command]
async fn connect_to_peer(
    state: State<'_, SharedState>,
    addr: String,
    port: u16,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock().await;
        s.server_addr = Some(addr);
        s.port = port;
        s.role = Role::Client;
        s.save()?;
    }
    // Reusar start_connection (já cancela anterior)
    drop(state.cancel_connection().await);
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        core::client::connect(state_clone, cancel).await;
    });
    Ok(())
}

// ── Novos comandos IPC ────────────────────────────────────────────────────────

/// Toggle do modo lock (pausar/retomar transição de cursor)
#[tauri::command]
async fn toggle_lock(state: State<'_, SharedState>) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    let locked = !state.lock_mode.load(Ordering::Relaxed);
    state.lock_mode.store(locked, Ordering::Relaxed);
    tracing::info!("Modo lock: {}", if locked { "ATIVO" } else { "INATIVO" });
    Ok(locked)
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

/// Atualizar apenas as preferências sem reiniciar conexão
#[tauri::command]
async fn update_preferences(
    state: State<'_, SharedState>,
    notifications_enabled: bool,
    lock_key: String,
    theme: String,
) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.notifications_enabled = notifications_enabled;
    s.lock_key = lock_key.clone();
    s.theme = theme;
    s.save()?;
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

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    // Instalar o CryptoProvider do rustls antes de qualquer conexão TLS
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let settings = Settings::load();
    let shared_state: SharedState = Arc::new(AppState::new(settings));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared_state.clone())
        .setup(move |app| {
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

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
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
                                let current = state_tray.lock_mode.load(Ordering::Relaxed);
                                state_tray.lock_mode.store(!current, Ordering::Relaxed);
                                tracing::info!("Tray: modo lock {}", if !current { "ATIVO" } else { "INATIVO" });
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

            // ── Atalho global para modo lock ─────────────────────────────────
            // Atalho global para modo lock
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
                            let current = state_for_shortcut.lock_mode.load(Ordering::Relaxed);
                            state_for_shortcut.lock_mode.store(!current, Ordering::Relaxed);
                            tracing::info!("Atalho: modo lock toggled → {}", !current);
                        }
                    },
                ) {
                    tracing::warn!("Falha ao registrar atalho global: {}", e);
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
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar Movex");
}
