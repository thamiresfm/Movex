pub mod connection;
pub mod discovery;
pub mod settings;
pub mod status;
pub mod system;
pub mod transfer;
pub mod update;

use crate::core::state::SharedState;

/// Emite estado para todas as janelas (evita falha silenciosa se o label da janela não for `main`).
pub(crate) async fn emit_status_to_main(state: &SharedState) {
    use tauri::Emitter;
    let app = { state.app_handle.lock().await.clone() };
    let Some(app) = app else {
        return;
    };
    let payload = status::build_status_payload(state).await;
    let _ = app.emit("movex://status-changed", &payload);
}
