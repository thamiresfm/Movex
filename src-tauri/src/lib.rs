mod clipboard;
mod config;
mod core;
mod input;
mod ipc;
mod network;
mod permissions;
mod screen;
mod transfer;

use std::sync::Arc;

use crate::config::Settings;
use crate::core::state::{AppState, SharedState};

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

            // ── Pedir Acessibilidade (macOS) + texto em Info.plist ─────────────
            permissions::request_macos_accessibility_prompt();

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
            ipc::status::get_status,
            ipc::status::get_settings,
            ipc::settings::save_settings,
            ipc::settings::complete_setup,
            ipc::connection::start_connection,
            ipc::connection::disconnect,
            ipc::connection::reset_settings,
            ipc::connection::clear_server_cert_trust,
            ipc::settings::set_role,
            ipc::settings::set_server_addr,
            ipc::discovery::discover_peers,
            ipc::discovery::get_local_ipv4_addrs,
            ipc::connection::connect_to_peer,
            ipc::transfer::send_file_to_peer,
            ipc::transfer::get_transfers,
            ipc::connection::get_pending_approval,
            ipc::connection::approve_connection,
            ipc::connection::reject_connection,
            ipc::system::toggle_lock,
            ipc::system::get_stats,
            ipc::system::get_screen_resolution,
            ipc::connection::switch_role,
            ipc::settings::update_preferences,
            ipc::settings::get_recent_peers,
            ipc::settings::clear_recent_peers,
            ipc::system::get_monitors,
            ipc::transfer::drop_file_to_peer,
            ipc::update::check_for_update,
            ipc::update::install_update,
            ipc::system::set_screen_border,
            ipc::system::diagnose_connection,
            permissions::open_system_settings,
            permissions::get_platform_kind,
            permissions::windows_apply_firewall_rules_for_movex,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar Movex");
}
