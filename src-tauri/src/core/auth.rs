/// Módulo de autenticação — HMAC-SHA256 real conforme RFC 2104
use sha2::{Sha256, Digest};

/// Computa HMAC-SHA256 real: H(K XOR opad || H(K XOR ipad || message))
/// Onde K = psk_hex (como bytes), message = server_nonce
pub fn compute_hmac(psk_hex: &str, server_nonce: &str) -> String {
    // Decodificar PSK de hex para bytes reais (256 bits de entropia efetiva)
    let key_bytes = hex::decode(psk_hex).unwrap_or_else(|_| psk_hex.as_bytes().to_vec());
    let key = key_bytes.as_slice();
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
    fn hmac_deterministic_known_answer() {
        // Vetor fixo calculado com a assinatura real da função:
        // compute_hmac(psk_hex="616263", nonce="313233")
        // = HMAC-SHA256(key=b"616263", msg=b"313233")
        // Calculado offline e fixado para detectar qualquer regressão
        let result = compute_hmac("616263", "313233");
        assert_eq!(
            result,
            "ab1cf4202ec5e2318ddb7a118dae531640700891a7be1b0b8aa7654768a27db9",
            "HMAC produziu valor inesperado — possível regressão no algoritmo"
        );
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
