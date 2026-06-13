use crate::network::protocol::{Message, CLIPBOARD_MAX_BYTES};
use tracing::{debug, info, warn};

// ── Leitura do clipboard ──────────────────────────────────────────────────────

pub fn read_clipboard() -> Option<String> {
    match arboard::Clipboard::new() {
        Ok(mut cb) => cb.get_text().ok().filter(|t| !t.is_empty()),
        Err(e) => {
            warn!("Falha ao abrir clipboard: {}", e);
            None
        }
    }
}

pub fn write_clipboard(text: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Err(e) = cb.set_text(text) {
            warn!("Falha ao escrever texto no clipboard: {}", e);
        }
    }
}

// ── Serialização para rede ────────────────────────────────────────────────────

pub fn create_clipboard_message() -> Option<Message> {
    let text = read_clipboard()?;
    let data = text.into_bytes();
    if data.len() > CLIPBOARD_MAX_BYTES {
        warn!(
            "Clipboard texto excede {}MB — ignorado",
            CLIPBOARD_MAX_BYTES / 1024 / 1024
        );
        return None;
    }
    debug!("Clipboard texto: {} bytes", data.len());
    Some(Message::ClipboardData {
        mime: "text/plain".to_string(),
        data,
    })
}

pub fn apply_clipboard_message(msg: &Message) {
    if let Message::ClipboardData { mime, data } = msg {
        if mime == "text/plain" {
            // Aplicar o mesmo limite de tamanho do envio também na recepção.
            // Sem isto, um peer poderia enviar até o máximo do protocolo e
            // escrever tudo no clipboard local.
            if data.len() > CLIPBOARD_MAX_BYTES {
                warn!(
                    "Clipboard recebido excede {}MB — ignorado",
                    CLIPBOARD_MAX_BYTES / 1024 / 1024
                );
                return;
            }
            if let Ok(text) = std::str::from_utf8(data) {
                write_clipboard(text);
                info!("Clipboard texto sincronizado: {} bytes", data.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_clipboard_message_ignores_unknown_mime() {
        let msg = Message::ClipboardData {
            mime: "image/png;w=1;h=1".to_string(),
            data: vec![0u8; 10],
        };
        apply_clipboard_message(&msg); // deve ignorar silenciosamente
    }

    #[test]
    fn apply_clipboard_message_ignores_octet_stream() {
        let msg = Message::ClipboardData {
            mime: "application/octet-stream".to_string(),
            data: vec![0u8; 10],
        };
        apply_clipboard_message(&msg);
    }
}
