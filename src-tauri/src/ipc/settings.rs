use tauri::State;

use crate::config::Role;
use crate::core::state::SharedState;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn save_settings(
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
            crate::config::autostart::enable().unwrap_or_else(|e| {
                tracing::warn!("Erro ao ativar autostart: {}", e)
            });
        } else {
            crate::config::autostart::disable().unwrap_or_else(|e| {
                tracing::warn!("Erro ao desativar autostart: {}", e)
            });
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn complete_setup(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.setup_complete = true;
    s.save()
}

#[tauri::command]
pub async fn set_role(state: State<'_, SharedState>, role: String) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.role = if role == "server" { Role::Server } else { Role::Client };
    s.save()
}

#[tauri::command]
pub async fn set_server_addr(state: State<'_, SharedState>, addr: Option<String>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.server_addr = addr;
    s.save()
}

/// Atualizar preferências — re-registra atalho global se mudou.
#[tauri::command]
pub async fn update_preferences(
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
pub async fn get_recent_peers(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::config::settings::RecentPeer>, String> {
    Ok(state.settings.lock().await.recent_peers.clone())
}

/// Limpa o histórico de peers
#[tauri::command]
pub async fn clear_recent_peers(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.recent_peers.clear();
    s.save()
}
