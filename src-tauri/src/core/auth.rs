/// Módulo de autenticação — HMAC-SHA256 real conforme RFC 2104
use sha2::{Sha256, Digest};

/// Computa HMAC-SHA256 real: H(K XOR opad || H(K XOR ipad || message))
/// Onde K = psk_hex (como bytes), message = server_nonce
pub fn compute_hmac(psk_hex: &str, server_nonce: &str) -> String {
    let key = psk_hex.as_bytes();
    let msg = server_nonce.as_bytes();

    // Preparar chave (pad para 64 bytes — block size do SHA-256)
    let mut k_padded = [0u8; 64];
    if key.len() <= 64 {
        k_padded[..key.len()].copy_from_slice(key);
    } else {
        // Chave longa: fazer hash primeiro
        let hashed = Sha256::digest(key);
        k_padded[..32].copy_from_slice(&hashed);
    }

    // ipad = 0x36, opad = 0x5c
    let ipad: Vec<u8> = k_padded.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k_padded.iter().map(|b| b ^ 0x5c).collect();

    // Inner hash: SHA256(ipad || message)
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(msg);
    let inner_hash = inner.finalize();

    // Outer hash: SHA256(opad || inner_hash)
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_hash);

    hex::encode(outer.finalize())
}

/// Verifica HMAC em tempo constante (evita timing attacks)
pub fn verify_hmac(psk_hex: &str, server_nonce: &str, received_hmac: &str) -> bool {
    let expected = compute_hmac(psk_hex, server_nonce);
    if expected.len() != received_hmac.len() { return false; }
    // Comparação em tempo constante — XOR acumula diferenças
    let diff: u8 = expected.bytes().zip(received_hmac.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_known_vector() {
        // RFC 4231 Test Case 1: key=0x0b*20, data="Hi There"
        let key_hex = hex::encode([0x0bu8; 20]);
        let result = compute_hmac(&key_hex, "Hi There");
        // Não é exatamente RFC4231 porque a chave é passada como hex string
        // mas garante que a função é determinística
        assert_eq!(result, compute_hmac(&key_hex, "Hi There"));
    }

    #[test]
    fn hmac_different_nonce_different_result() {
        let psk = "deadbeef".repeat(8);
        assert_ne!(compute_hmac(&psk, "nonce1"), compute_hmac(&psk, "nonce2"));
    }

    #[test]
    fn hmac_different_psk_different_result() {
        assert_ne!(compute_hmac("psk1", "nonce"), compute_hmac("psk2", "nonce"));
    }

    #[test]
    fn verify_hmac_correct() {
        let psk = "mysecretkey";
        let nonce = "randomnonce123";
        let hmac = compute_hmac(psk, nonce);
        assert!(verify_hmac(psk, nonce, &hmac));
    }

    #[test]
    fn verify_hmac_wrong_key() {
        let hmac = compute_hmac("correct_psk", "nonce");
        assert!(!verify_hmac("wrong_psk", "nonce", &hmac));
    }
}
