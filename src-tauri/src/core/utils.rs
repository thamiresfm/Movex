/// Utilitários compartilhados entre módulos

/// CRC32 (IEEE 802.3 / polinômio 0xEDB88320) — usado para detecção de mudanças de clipboard
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_value() {
        // CRC32 de "123456789" = 0xCBF43926 (valor padrão IEEE)
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn crc32_empty_is_zero() {
        // Por definição: crc inicial = 0xFFFFFFFF, nenhum byte processado → !0xFFFFFFFF = 0
        assert_eq!(crc32(b""), 0x00000000);
    }

    #[test]
    fn crc32_single_null_byte_differs_from_empty() {
        // Um byte zero deve produzir resultado diferente de vazio
        assert_ne!(crc32(b""), crc32(b"\x00"));
    }

    #[test]
    fn crc32_different_data_different_hash() {
        assert_ne!(crc32(b"hello"), crc32(b"world"));
    }

    #[test]
    fn crc32_same_size_different_content() {
        // Garante que tamanho igual não implica hash igual (falso negativo do mime:len)
        assert_ne!(crc32(b"AAAA"), crc32(b"BBBB"));
    }
}
