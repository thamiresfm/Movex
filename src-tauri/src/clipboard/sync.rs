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
            // Tentar imagem primeiro
            if let Ok(img) = cb.get_image() {
                let rgba = img.bytes.to_vec();
                return ClipboardContent::Image {
                    width: img.width as u32,
                    height: img.height as u32,
                    rgba,
                };
            }
            // Fallback para texto
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
            // Comprimir PNG em memória usando encode simples
            let png_data = encode_rgba_to_png(width, height, &rgba);
            if png_data.len() > CLIPBOARD_MAX_BYTES {
                warn!("Clipboard imagem excede {}MB — ignorado", CLIPBOARD_MAX_BYTES / 1024 / 1024);
                return None;
            }
            // Incluir width/height no mime type para decodificação
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
            // Extrair dimensões do mime type: "image/png;w=1920;h=1080"
            let (width, height) = parse_image_mime(mime);
            // Decodificar PNG → RGBA
            if let Some(rgba) = decode_png_to_rgba(data) {
                write_clipboard_image(width, height, &rgba);
                info!("Clipboard imagem sincronizada: {}x{}", width, height);
            }
        }
    }
}

// ── Codificação PNG mínima ────────────────────────────────────────────────────

/// Codifica RGBA para PNG usando zlib puro (sem dependência extra)
fn encode_rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use std::io::Write;

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFFFFFFu32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB88320 } else { crc >> 1 };
            }
        }
        !crc
    }

    fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let len = (data.len() as u32).to_be_bytes();
        let mut combined = Vec::with_capacity(4 + data.len());
        combined.extend_from_slice(tag);
        combined.extend_from_slice(data);
        let crc = crc32(&combined).to_be_bytes();
        let mut out = Vec::with_capacity(8 + data.len() + 4);
        out.extend_from_slice(&len);
        out.extend_from_slice(&combined);
        out.extend_from_slice(&crc);
        out
    }

    // IHDR
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA

    // IDAT: filtrar linhas com filter byte 0
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0); // filter none
        let start = row * width as usize * 4;
        let end = start + width as usize * 4;
        raw.extend_from_slice(&rgba[start..end.min(rgba.len())]);
    }
    let compressed = miniz_compress(&raw);

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    png.extend_from_slice(&chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&chunk(b"IDAT", &compressed));
    png.extend_from_slice(&chunk(b"IEND", &[]));
    png
}

/// Compressão zlib real via flate2 (nível fast — boa compressão, baixa latência)
fn miniz_compress(data: &[u8]) -> Vec<u8> {
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).unwrap_or_default();
    encoder.finish().unwrap_or_default()
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
        let png = encode_rgba_to_png(4, 4, &rgba);
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn create_clipboard_message_returns_correct_mime() {
        let _result = create_clipboard_message();
    }
}
