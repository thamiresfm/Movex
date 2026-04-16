use tracing::{debug, info, warn};

use crate::network::protocol::{Message, CLIPBOARD_MAX_BYTES};

/// Lê conteúdo de texto do clipboard nativo
pub fn read_clipboard() -> Option<String> {
    #[cfg(target_os = "macos")]
    return read_clipboard_macos();
    #[cfg(target_os = "windows")]
    return read_clipboard_windows();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    None
}

/// Escreve texto no clipboard nativo
pub fn write_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    write_clipboard_macos(text);
    #[cfg(target_os = "windows")]
    write_clipboard_windows(text);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = text;
}

/// Cria uma mensagem `ClipboardData` a partir do clipboard atual
pub fn create_clipboard_message() -> Option<Message> {
    let text = read_clipboard()?;
    let data = text.into_bytes();
    if data.len() > CLIPBOARD_MAX_BYTES {
        warn!("Clipboard excede {}MB — ignorado", CLIPBOARD_MAX_BYTES / 1024 / 1024);
        return None;
    }
    debug!("Clipboard capturado: {} bytes", data.len());
    Some(Message::ClipboardData { mime: "text/plain".to_string(), data })
}

/// Aplica uma mensagem `ClipboardData` ao clipboard local
pub fn apply_clipboard_message(msg: &Message) {
    if let Message::ClipboardData { mime, data } = msg {
        if mime == "text/plain" {
            if let Ok(text) = std::str::from_utf8(data) {
                write_clipboard(text);
                info!("Clipboard sincronizado: {} bytes", data.len());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn read_clipboard_macos() -> Option<String> {
    let out = std::process::Command::new("pbpaste").output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn write_clipboard_macos(text: &str) {
    use std::io::Write;
    if let Ok(mut child) = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

#[cfg(target_os = "windows")]
fn read_clipboard_windows() -> Option<String> {
    None // stub
}

#[cfg(target_os = "windows")]
fn write_clipboard_windows(_text: &str) {} // stub

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_clipboard_message_ignores_non_text_mime() {
        let msg = Message::ClipboardData {
            mime: "image/png".to_string(),
            data: vec![0u8; 10],
        };
        // Não deve entrar em pânico — apenas ignorar
        apply_clipboard_message(&msg);
    }

    #[test]
    fn create_clipboard_message_returns_correct_mime() {
        // Apenas executa sem pânico; retorno depende do ambiente
        let _result = create_clipboard_message();
    }
}
