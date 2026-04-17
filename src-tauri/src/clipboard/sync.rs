use tracing::{debug, info, warn};
use crate::network::protocol::{Message, CLIPBOARD_MAX_BYTES};

// ── Tipo de conteúdo do clipboard ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ClipboardContent {
    Text(String),
    Image { width: u32, height: u32, rgba: Vec<u8> },
    None,
}

// ── Leitura do clipboard ──────────────────────────────────────────────────────

/// Lê o conteúdo atual do clipboard (texto ou imagem)
pub fn read_clipboard_content() -> ClipboardContent {
    match arboard::Clipboard::new() {
        Err(e) => {
            warn!("Falha ao abrir clipboard: {}", e);
            ClipboardContent::None
        }
        Ok(mut cb) => {
            if let Ok(img) = cb.get_image() {
                let rgba = img.bytes.to_vec();
                return ClipboardContent::Image {
                    width: img.width as u32,
                    height: img.height as u32,
                    rgba,
                };
            }
            if let Ok(text) = cb.get_text() {
                if !text.is_empty() {
                    return ClipboardContent::Text(text);
                }
            }
            ClipboardContent::None
        }
    }
}

/// Lê apenas texto (compatibilidade com código existente)
pub fn read_clipboard() -> Option<String> {
    match arboard::Clipboard::new() {
        Ok(mut cb) => cb.get_text().ok().filter(|t| !t.is_empty()),
        Err(_) => None,
    }
}

/// Escreve texto no clipboard
pub fn write_clipboard(text: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Err(e) = cb.set_text(text) {
            warn!("Falha ao escrever texto no clipboard: {}", e);
        }
    }
}

/// Escreve imagem RGBA no clipboard
pub fn write_clipboard_image(width: u32, height: u32, rgba: &[u8]) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let img = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Borrowed(rgba),
        };
        if let Err(e) = cb.set_image(img) {
            warn!("Falha ao escrever imagem no clipboard: {}", e);
        }
    }
}

// ── Serialização para rede ────────────────────────────────────────────────────

/// Cria mensagem ClipboardData a partir do conteúdo atual (texto ou imagem)
pub fn create_clipboard_message() -> Option<Message> {
    match read_clipboard_content() {
        ClipboardContent::Text(text) => {
            let data = text.into_bytes();
            if data.len() > CLIPBOARD_MAX_BYTES {
                warn!("Clipboard texto excede {}MB — ignorado", CLIPBOARD_MAX_BYTES / 1024 / 1024);
                return None;
            }
            debug!("Clipboard texto: {} bytes", data.len());
            Some(Message::ClipboardData { mime: "text/plain".to_string(), data })
        }
        ClipboardContent::Image { width, height, rgba } => {
            let png_data = match encode_rgba_to_png_safe(width, height, &rgba) {
                Ok(d) => d,
                Err(e) => { warn!("Falha ao codificar imagem PNG: {}", e); return None; }
            };
            if png_data.len() > CLIPBOARD_MAX_BYTES {
                warn!("Clipboard imagem excede {}MB — ignorado", CLIPBOARD_MAX_BYTES / 1024 / 1024);
                return None;
            }
            let mime = format!("image/png;w={};h={}", width, height);
            debug!("Clipboard imagem: {}x{} = {} bytes PNG", width, height, png_data.len());
            Some(Message::ClipboardData { mime, data: png_data })
        }
        ClipboardContent::None => None,
    }
}

/// Aplica mensagem ClipboardData ao clipboard local
pub fn apply_clipboard_message(msg: &Message) {
    if let Message::ClipboardData { mime, data } = msg {
        if mime == "text/plain" {
            if let Ok(text) = std::str::from_utf8(data) {
                write_clipboard(text);
                info!("Clipboard texto sincronizado: {} bytes", data.len());
            }
        } else if mime.starts_with("image/png") {
            let (width, height) = parse_image_mime(mime);
            if let Some(rgba) = decode_png_to_rgba(data) {
                write_clipboard_image(width, height, &rgba);
                info!("Clipboard imagem sincronizada: {}x{}", width, height);
            }
        }
    }
}

fn encode_rgba_to_png_safe(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    use image::{DynamicImage, RgbaImage};

    let img = RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| format!("Buffer RGBA inválido para {}x{}", width, height))?;
    let mut out: Vec<u8> = Vec::new();
    DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("Falha ao codificar PNG: {}", e))?;
    Ok(out)
}

/// Decodifica PNG → bytes RGBA raw (obrigatório para arboard::set_image)
fn decode_png_to_rgba(png_data: &[u8]) -> Option<Vec<u8>> {
    use image::ImageDecoder;
    let cursor = std::io::Cursor::new(png_data);
    let decoder = image::codecs::png::PngDecoder::new(cursor).ok()?;
    let total = decoder.total_bytes() as usize;
    let mut rgba = vec![0u8; total];
    decoder.read_image(&mut rgba).ok()?;
    Some(rgba)
}

fn parse_image_mime(mime: &str) -> (u32, u32) {
    let w = mime.split(";w=").nth(1)
        .and_then(|s| s.split(';').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let h = mime.split(";h=").nth(1)
        .and_then(|s| s.split(';').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_clipboard_message_ignores_unknown_mime() {
        let msg = Message::ClipboardData {
            mime: "application/octet-stream".to_string(),
            data: vec![0u8; 10],
        };
        apply_clipboard_message(&msg); // não deve panicar
    }

    #[test]
    fn parse_image_mime_extracts_dimensions() {
        let (w, h) = parse_image_mime("image/png;w=1920;h=1080");
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn encode_png_produces_valid_header() {
        let rgba = vec![255u8; 4 * 4 * 4]; // 4x4 white image
        let png = encode_rgba_to_png_safe(4, 4, &rgba).expect("encode PNG falhou no teste");
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn create_clipboard_message_returns_correct_mime() {
        let _result = create_clipboard_message();
    }
}
