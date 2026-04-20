use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Computa HMAC-SHA256 usando PSK decodificada de hex para bytes brutos.
pub fn compute_hmac(psk_hex: &str, server_nonce: &str) -> String {
    let key_bytes = hex::decode(psk_hex)
        .unwrap_or_else(|_| psk_hex.as_bytes().to_vec());

    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .expect("HMAC aceita chave de qualquer tamanho");
    mac.update(server_nonce.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verifica HMAC em tempo constante (usada em testes; o servidor já não exige PSK coincidente).
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
        // Vetor fixo: HMAC-SHA256(key=hex.decode("616263")=b"abc", msg="313233")
        let result = compute_hmac("616263", "313233");
        assert_eq!(
            result,
            "ab1cf4202ec5e2318ddb7a118dae531640700891a7be1b0b8aa7654768a27db9",
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

    #[test]
    fn verify_hmac_wrong_length_rejected() {
        assert!(!verify_hmac("psk", "nonce", "tooshort"));
    }

    #[test]
    fn verify_hmac_invalid_hex_rejected() {
        assert!(!verify_hmac("psk", "nonce", "not-valid-hex!@#$"));
    }
}
