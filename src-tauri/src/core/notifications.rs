//! Notificações nativas (toast no Windows, Notification Center no macOS).
//! Títulos e corpos curtos evitam cortes no sistema; ícone PNG embutido nos recursos do bundle.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri::Manager;
use tauri::path::BaseDirectory;
use tracing::{info, warn};

/// Limite seguro para o Centro de Notificações (especialmente Windows Toast).
const MAX_TITLE: usize = 64;
const MAX_BODY: usize = 420;

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let take = max_chars.saturating_sub(1);
    s.chars().take(take).collect::<String>() + "…"
}

/// Ícone 32×32 nos recursos do pacote (ver `bundle.resources` em `tauri.conf.json`).
fn icon_png_path(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = app.path().resolve("icons/32x32.png", BaseDirectory::Resource) {
        if p.exists() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons/32x32.png");
    if dev.exists() {
        return Some(dev);
    }
    None
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;

    let title = truncate_chars(title, MAX_TITLE);
    let body = truncate_chars(body, MAX_BODY);

    let mut builder = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .group("com.movex.session");

    if let Some(ref path) = icon_png_path(app) {
        if let Some(s) = path.to_str() {
            builder = builder.icon(s);
        }
    }

    match builder.show() {
        Ok(()) => {
            info!(target: "movex_notify", "{} — {}", title, body);
        }
        Err(e) => {
            warn!(
                target: "movex_notify",
                "Falha ao mostrar notificação ({}): {} · {}",
                title, e, body
            );
        }
    }
}
