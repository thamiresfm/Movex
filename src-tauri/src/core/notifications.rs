use tauri::{AppHandle, Manager};
use tracing::warn;

/// Envia notificação do sistema se o usuário habilitou
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

pub fn notify_connected(app: &AppHandle, peer: &str) {
    notify(app, "Movex — Conectado", &format!("Controlando: {}", peer));
}

pub fn notify_disconnected(app: &AppHandle, peer: &str) {
    notify(app, "Movex — Desconectado", &format!("Sessão encerrada com {}", peer));
}

pub fn notify_connection_request(app: &AppHandle, peer: &str) {
    notify(app, "Movex — Solicitação de Conexão", &format!("{} quer controlar este computador", peer));
}

pub fn notify_file_received(app: &AppHandle, filename: &str) {
    notify(app, "Movex — Arquivo Recebido", &format!("📁 {}", filename));
}

pub fn notify_rejected(app: &AppHandle) {
    notify(app, "Movex — Conexão Recusada", "O servidor recusou a conexão");
}
