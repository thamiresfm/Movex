use tauri::State;

use crate::core::state::SharedState;

/// Valida que color é um valor CSS seguro (hex ou nome simples)
fn is_safe_css_color(color: &str) -> bool {
    // Aceitar #RGB, #RRGGBB, #RRGGBBAA e nomes CSS simples (max 30 chars, alfanum)
    color.len() <= 30 && color.chars().all(|c| c.is_ascii_alphanumeric() || c == '#')
}

/// Retorna estatísticas da sessão atual
#[tauri::command]
pub async fn get_stats(state: State<'_, SharedState>) -> Result<crate::core::stats::StatsSnapshot, String> {
    Ok(state.stats.snapshot())
}

/// Retorna resolução real do monitor principal
#[tauri::command]
pub fn get_screen_resolution() -> (u32, u32) {
    crate::core::stats::get_primary_screen_size()
}

/// Retorna lista de monitores detectados localmente
#[tauri::command]
pub fn get_monitors() -> Vec<crate::screen::layout::Monitor> {
    crate::screen::layout::detect_monitors().monitors
}

/// Toggle do modo lock (pausar/retomar transição de cursor) — operação atômica
#[tauri::command]
pub async fn toggle_lock(state: State<'_, SharedState>) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    // fetch_xor é atômico — evita race condition entre atalho global e comando IPC
    let was_locked = state.lock_mode.fetch_xor(true, Ordering::AcqRel);
    let now_locked = !was_locked;
    tracing::info!("Modo lock: {}", if now_locked { "ATIVO" } else { "INATIVO" });
    Ok(now_locked)
}

/// Ativa/desativa borda luminosa no monitor via evento Tauri (sem eval/unsafe-inline)
#[tauri::command]
pub async fn set_screen_border(
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
