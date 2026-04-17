use tauri::AppHandle;
use tracing::warn;

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
    {
        warn!("Falha ao enviar notificação: {}", e);
    }
}
