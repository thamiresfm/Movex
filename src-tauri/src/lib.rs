// Módulos implementados mas ainda não totalmente conectados ao runtime —
// os warnings são esperados e serão removidos conforme as features forem integradas.
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

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

// ── Payloads IPC ─────────────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
pub struct StatusPayload {
    pub connected: bool,
    pub status_text: String,
    pub peer_hostname: Option<String>,
    pub latency_ms: Option<u32>,
    pub active_screen: String,
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
}

// ── Comandos IPC ─────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_status(state: State<'_, SharedState>) -> Result<StatusPayload, String> {
    let status = state.connection_status.lock().await.clone();
    let active = state.active_screen.lock().await.clone();

    let (connected, peer_hostname, latency_ms) = match &status {
        ConnectionStatus::Connected { peer_hostname, latency_ms } => {
            (true, Some(peer_hostname.clone()), Some(*latency_ms))
        }
        _ => (false, None, None),
    };

    Ok(StatusPayload {
        connected,
        status_text: status.to_string(),
        peer_hostname,
        latency_ms,
        active_screen: format!("{:?}", active),
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
        peer_position: format!("{:?}", s.peer_position).to_lowercase(),
        autostart: s.autostart,
        theme: s.theme.clone(),
        setup_complete: s.setup_complete,
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
    _peer_position: String,
    autostart: bool,
    theme: String,
) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.hostname = hostname;
    s.role = if role == "server" { Role::Server } else { Role::Client };
    s.server_addr = server_addr;
    s.port = port;
    s.psk_hex = psk_hex;
    s.autostart = autostart;
    s.theme = theme;
    s.setup_complete = true;
    s.save()
}

#[tauri::command]
async fn complete_setup(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.setup_complete = true;
    s.save()
}

#[tauri::command]
async fn start_connection(state: State<'_, SharedState>) -> Result<(), String> {
    let role = { state.settings.lock().await.role.clone() };
    let state_clone = state.inner().clone();
    match role {
        Role::Server => {
            tokio::spawn(async move {
                if let Err(e) = core::server::start(state_clone).await {
                    tracing::error!("Servidor encerrou com erro: {}", e);
                }
            });
        }
        Role::Client => {
            tokio::spawn(async move {
                core::client::connect(state_clone).await;
            });
        }
    }
    Ok(())
}

#[tauri::command]
async fn disconnect(state: State<'_, SharedState>) -> Result<(), String> {
    let mut status = state.connection_status.lock().await;
    *status = ConnectionStatus::Disconnected;
    tracing::info!("Desconectado pelo usuário");
    Ok(())
}

/// Reseta todas as configurações para o padrão (volta ao setup wizard)
#[tauri::command]
async fn reset_settings(state: State<'_, SharedState>) -> Result<(), String> {
    // Desconectar primeiro
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }
    // Gerar novas configurações padrão (nova PSK, setup_complete = false)
    let new_settings = Settings::default();
    new_settings.save()?;
    {
        let mut s = state.settings.lock().await;
        *s = new_settings;
    }
    tracing::info!("Configurações resetadas — voltando ao setup wizard");
    Ok(())
}

/// Troca o papel (servidor ↔ cliente) e salva
#[tauri::command]
async fn set_role(state: State<'_, SharedState>, role: String) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.role = if role == "server" { Role::Server } else { Role::Client };
    s.save()
}

/// Define o endereço do servidor (modo cliente)
#[tauri::command]
async fn set_server_addr(state: State<'_, SharedState>, addr: Option<String>) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.server_addr = addr;
    s.save()
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
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
        .manage(shared_state)
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
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar Movex");
}
