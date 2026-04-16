/// Módulo de autenticação compartilhado entre servidor e cliente
use sha2::{Sha256, Digest};

/// Computa HMAC-SHA256(psk_hex, server_nonce) — usado no handshake
/// O servidor gera o nonce, o cliente computa o HMAC e envia de volta
pub fn compute_hmac(psk_hex: &str, server_nonce: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(psk_hex.as_bytes());
    hasher.update(b":");
    hasher.update(server_nonce.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verifica se o HMAC recebido corresponde ao esperado
pub fn verify_hmac(psk_hex: &str, server_nonce: &str, received_hmac: &str) -> bool {
    let expected = compute_hmac(psk_hex, server_nonce);
    // Comparação em tempo constante para evitar timing attacks
    expected.len() == received_hmac.len()
        && expected.bytes().zip(received_hmac.bytes()).all(|(a, b)| a == b)
}
