use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Computa HMAC-SHA256 usando PSK decodificada de hex para bytes brutos.
/// Retorna Err se `psk_hex` não for hex válido — falha explícita em vez de fallback silencioso.
pub fn compute_hmac(psk_hex: &str, server_nonce: &str) -> Result<String, String> {
    let key_bytes = hex::decode(psk_hex)
        .map_err(|e| format!("PSK inválida (hex): {e}"))?;
    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .expect("HMAC aceita chave de qualquer tamanho");
    mac.update(server_nonce.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Verifica HMAC em tempo constante.
#[allow(dead_code)]
pub fn verify_hmac(psk_hex: &str, server_nonce: &str, received_hmac: &str) -> bool {
    let expected_bytes = match hex::decode(received_hmac) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let key_bytes = hex::decode(psk_hex)
        .unwrap_or_else(|_| psk_hex.as_bytes().to_vec());

    let mut mac = match HmacSha256::new_from_slice(&key_bytes) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(server_nonce.as_bytes());
    mac.verify_slice(&expected_bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_deterministic_known_answer() {
        let result = compute_hmac("616263", "313233").unwrap();
        assert_eq!(result, "ab1cf4202ec5e2318ddb7a118dae531640700891a7be1b0b8aa7654768a27db9");
    }

    #[test]
    fn hmac_invalid_psk_returns_err() {
        assert!(compute_hmac("not-hex!", "nonce").is_err());
    }

    #[test]
    fn hmac_different_nonce_different_result() {
        let psk = "deadbeef".repeat(8);
        assert_ne!(compute_hmac(&psk, "nonce1").unwrap(), compute_hmac(&psk, "nonce2").unwrap());
    }

    #[test]
    fn hmac_different_psk_different_result() {
        assert_ne!(
            compute_hmac("aaaa", "nonce").unwrap(),
            compute_hmac("bbbb", "nonce").unwrap()
        );
    }

    #[test]
    fn verify_hmac_correct() {
        let psk_hex = hex::encode("mysecretkey");
        let nonce = "randomnonce123";
        let hmac = compute_hmac(&psk_hex, nonce).unwrap();
        assert!(verify_hmac(&psk_hex, nonce, &hmac));
    }

    #[test]
    fn verify_hmac_wrong_key() {
        let correct = hex::encode("correct_psk");
        let wrong   = hex::encode("wrong_psk");
        let hmac = compute_hmac(&correct, "nonce").unwrap();
        assert!(!verify_hmac(&wrong, "nonce", &hmac));
    }

    #[test]
    fn verify_hmac_wrong_length_rejected() {
        assert!(!verify_hmac("psk", "nonce", "tooshort"));
    }

    #[test]
    fn verify_hmac_invalid_hex_rejected() {
        assert!(!verify_hmac("psk", "nonce", "not-valid-hex!@#$"));
    }
}
