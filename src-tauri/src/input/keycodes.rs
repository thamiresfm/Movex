//! Conversão de keycodes nativos para USB HID (e vice-versa).
//!
//! Cada SO tem o seu próprio espaço de códigos:
//! - Windows: Virtual Key codes (`VK_*`).
//! - macOS: códigos do Carbon HIToolbox (`kVK_*`).
//!
//! O protocolo do Movex viaja em USB HID Usage IDs (Keyboard/Keypad page 0x07,
//! da `HID Usage Tables 1.12` da USB-IF), independentes de layout. Cada lado
//! converte o seu nativo para HID no envio e de HID para o nativo na injeção.
//!
//! Cobre as teclas habituais para um layout US (~80): A-Z, 0-9, Enter, Esc,
//! Tab, Space, Backspace, símbolos US, modificadores (Shift/Ctrl/Alt/Meta L+R),
//! setas, F1-F12, Home/End/PageUp/PageDown/Insert/Delete, Caps Lock. Teclas
//! menos comuns (numpad estendido, mídia, IME) ainda passam por translação
//! aproximada quando possível, e fall-back para `None` quando não há mapa.

/// Converte um Virtual Key code do Windows para HID Usage ID.
/// Devolve `None` se a tecla não tiver mapa conhecido — o chamador deve
/// descartar o evento (em vez de injectar lixo no outro lado).
#[allow(dead_code)]
pub fn vk_to_hid(vk: u32) -> Option<u32> {
    let hid = match vk {
        // Letras A-Z (Windows VK = ASCII 'A'..'Z' = 0x41..0x5A)
        0x41..=0x5A => 0x04 + (vk - 0x41), // A=0x04, Z=0x1D
        // Números 1-9 (HID 0x1E..0x26)
        0x31..=0x39 => 0x1E + (vk - 0x31),
        // Número 0 → HID 0x27
        0x30 => 0x27,
        // F1-F12 (Windows 0x70..0x7B → HID 0x3A..0x45)
        0x70..=0x7B => 0x3A + (vk - 0x70),
        0x0D => 0x28, // VK_RETURN → Enter
        0x1B => 0x29, // VK_ESCAPE
        0x08 => 0x2A, // VK_BACK → Backspace
        0x09 => 0x2B, // VK_TAB
        0x20 => 0x2C, // VK_SPACE
        0xBD => 0x2D, // VK_OEM_MINUS → -
        0xBB => 0x2E, // VK_OEM_PLUS  → =
        0xDB => 0x2F, // VK_OEM_4     → [
        0xDD => 0x30, // VK_OEM_6     → ]
        0xDC => 0x31, // VK_OEM_5     → \
        0xBA => 0x33, // VK_OEM_1     → ;
        0xDE => 0x34, // VK_OEM_7     → '
        0xC0 => 0x35, // VK_OEM_3     → `
        0xBC => 0x36, // VK_OEM_COMMA → ,
        0xBE => 0x37, // VK_OEM_PERIOD → .
        0xBF => 0x38, // VK_OEM_2     → /
        0x14 => 0x39, // VK_CAPITAL   → Caps Lock
        // Setas
        0x27 => 0x4F, // VK_RIGHT
        0x25 => 0x50, // VK_LEFT
        0x28 => 0x51, // VK_DOWN
        0x26 => 0x52, // VK_UP
        // Bloco de navegação
        0x2D => 0x49, // VK_INSERT
        0x24 => 0x4A, // VK_HOME
        0x21 => 0x4B, // VK_PRIOR (PageUp)
        0x2E => 0x4C, // VK_DELETE
        0x23 => 0x4D, // VK_END
        0x22 => 0x4E, // VK_NEXT (PageDown)
        // Modificadores: VK_LSHIFT/RSHIFT/LCONTROL/RCONTROL/LMENU/RMENU/LWIN/RWIN
        0xA0 => 0xE1, // VK_LSHIFT
        0xA1 => 0xE5, // VK_RSHIFT
        0xA2 => 0xE0, // VK_LCONTROL
        0xA3 => 0xE4, // VK_RCONTROL
        0xA4 => 0xE2, // VK_LMENU (left Alt)
        0xA5 => 0xE6, // VK_RMENU (right Alt)
        0x5B => 0xE3, // VK_LWIN (left meta)
        0x5C => 0xE7, // VK_RWIN
        // Genéricos, quando o Windows não diferencia esquerdo/direito
        0x10 => 0xE1, // VK_SHIFT  → trata como left
        0x11 => 0xE0, // VK_CONTROL
        0x12 => 0xE2, // VK_MENU (Alt)
        _ => return None,
    };
    Some(hid)
}

/// Converte HID Usage ID para Virtual Key code do Windows.
#[allow(dead_code)]
pub fn hid_to_vk(hid: u32) -> Option<u16> {
    let vk: u16 = match hid {
        // Letras A-Z
        0x04..=0x1D => (0x41 + (hid - 0x04)) as u16,
        // Números 1-9
        0x1E..=0x26 => (0x31 + (hid - 0x1E)) as u16,
        // Número 0
        0x27 => 0x30,
        // F1-F12
        0x3A..=0x45 => (0x70 + (hid - 0x3A)) as u16,
        0x28 => 0x0D, // Enter
        0x29 => 0x1B, // Escape
        0x2A => 0x08, // Backspace
        0x2B => 0x09, // Tab
        0x2C => 0x20, // Space
        0x2D => 0xBD, // -
        0x2E => 0xBB, // =
        0x2F => 0xDB, // [
        0x30 => 0xDD, // ]
        0x31 => 0xDC, // \
        0x33 => 0xBA, // ;
        0x34 => 0xDE, // '
        0x35 => 0xC0, // `
        0x36 => 0xBC, // ,
        0x37 => 0xBE, // .
        0x38 => 0xBF, // /
        0x39 => 0x14, // Caps Lock
        0x4F => 0x27, // Right
        0x50 => 0x25, // Left
        0x51 => 0x28, // Down
        0x52 => 0x26, // Up
        0x49 => 0x2D, // Insert
        0x4A => 0x24, // Home
        0x4B => 0x21, // PageUp
        0x4C => 0x2E, // Delete
        0x4D => 0x23, // End
        0x4E => 0x22, // PageDown
        0xE0 => 0xA2, // LCtrl
        0xE1 => 0xA0, // LShift
        0xE2 => 0xA4, // LAlt
        0xE3 => 0x5B, // LWin
        0xE4 => 0xA3, // RCtrl
        0xE5 => 0xA1, // RShift
        0xE6 => 0xA5, // RAlt
        0xE7 => 0x5C, // RWin
        _ => return None,
    };
    Some(vk)
}

/// Converte um keycode do macOS (Carbon HIToolbox) para HID Usage ID.
#[allow(dead_code)]
pub fn mac_to_hid(mac: u32) -> Option<u32> {
    let hid = match mac {
        0x00 => 0x04, // A
        0x0B => 0x05, // B
        0x08 => 0x06, // C
        0x02 => 0x07, // D
        0x0E => 0x08, // E
        0x03 => 0x09, // F
        0x05 => 0x0A, // G
        0x04 => 0x0B, // H
        0x22 => 0x0C, // I
        0x26 => 0x0D, // J
        0x28 => 0x0E, // K
        0x25 => 0x0F, // L
        0x2E => 0x10, // M
        0x2D => 0x11, // N
        0x1F => 0x12, // O
        0x23 => 0x13, // P
        0x0C => 0x14, // Q
        0x0F => 0x15, // R
        0x01 => 0x16, // S
        0x11 => 0x17, // T
        0x20 => 0x18, // U
        0x09 => 0x19, // V
        0x0D => 0x1A, // W
        0x07 => 0x1B, // X
        0x10 => 0x1C, // Y
        0x06 => 0x1D, // Z
        0x12 => 0x1E, // 1
        0x13 => 0x1F, // 2
        0x14 => 0x20, // 3
        0x15 => 0x21, // 4
        0x17 => 0x22, // 5
        0x16 => 0x23, // 6
        0x1A => 0x24, // 7
        0x1C => 0x25, // 8
        0x19 => 0x26, // 9
        0x1D => 0x27, // 0
        0x24 => 0x28, // Return
        0x35 => 0x29, // Escape
        0x33 => 0x2A, // Delete (Backspace)
        0x30 => 0x2B, // Tab
        0x31 => 0x2C, // Space
        0x1B => 0x2D, // -
        0x18 => 0x2E, // =
        0x21 => 0x2F, // [
        0x1E => 0x30, // ]
        0x2A => 0x31, // \
        0x29 => 0x33, // ;
        0x27 => 0x34, // '
        0x32 => 0x35, // `
        0x2B => 0x36, // ,
        0x2F => 0x37, // .
        0x2C => 0x38, // /
        0x39 => 0x39, // Caps Lock
        0x7A => 0x3A, // F1
        0x78 => 0x3B, // F2
        0x63 => 0x3C, // F3
        0x76 => 0x3D, // F4
        0x60 => 0x3E, // F5
        0x61 => 0x3F, // F6
        0x62 => 0x40, // F7
        0x64 => 0x41, // F8
        0x65 => 0x42, // F9
        0x6D => 0x43, // F10
        0x67 => 0x44, // F11
        0x6F => 0x45, // F12
        0x73 => 0x4A, // Home
        0x74 => 0x4B, // PageUp
        0x75 => 0x4C, // Delete (Forward)
        0x77 => 0x4D, // End
        0x79 => 0x4E, // PageDown
        0x7C => 0x4F, // Right
        0x7B => 0x50, // Left
        0x7D => 0x51, // Down
        0x7E => 0x52, // Up
        0x3B => 0xE0, // LCtrl
        0x38 => 0xE1, // LShift
        0x3A => 0xE2, // LAlt (Option)
        0x37 => 0xE3, // LMeta (Command)
        0x3E => 0xE4, // RCtrl
        0x3C => 0xE5, // RShift
        0x3D => 0xE6, // RAlt
        0x36 => 0xE7, // RMeta
        _ => return None,
    };
    Some(hid)
}

/// Converte HID Usage ID para keycode macOS (Carbon).
#[allow(dead_code)]
pub fn hid_to_mac(hid: u32) -> Option<u16> {
    let mac: u16 = match hid {
        0x04 => 0x00, // A
        0x05 => 0x0B, // B
        0x06 => 0x08, // C
        0x07 => 0x02, // D
        0x08 => 0x0E, // E
        0x09 => 0x03, // F
        0x0A => 0x05, // G
        0x0B => 0x04, // H
        0x0C => 0x22, // I
        0x0D => 0x26, // J
        0x0E => 0x28, // K
        0x0F => 0x25, // L
        0x10 => 0x2E, // M
        0x11 => 0x2D, // N
        0x12 => 0x1F, // O
        0x13 => 0x23, // P
        0x14 => 0x0C, // Q
        0x15 => 0x0F, // R
        0x16 => 0x01, // S
        0x17 => 0x11, // T
        0x18 => 0x20, // U
        0x19 => 0x09, // V
        0x1A => 0x0D, // W
        0x1B => 0x07, // X
        0x1C => 0x10, // Y
        0x1D => 0x06, // Z
        0x1E => 0x12, // 1
        0x1F => 0x13, // 2
        0x20 => 0x14, // 3
        0x21 => 0x15, // 4
        0x22 => 0x17, // 5
        0x23 => 0x16, // 6
        0x24 => 0x1A, // 7
        0x25 => 0x1C, // 8
        0x26 => 0x19, // 9
        0x27 => 0x1D, // 0
        0x28 => 0x24, // Return
        0x29 => 0x35, // Escape
        0x2A => 0x33, // Backspace
        0x2B => 0x30, // Tab
        0x2C => 0x31, // Space
        0x2D => 0x1B, // -
        0x2E => 0x18, // =
        0x2F => 0x21, // [
        0x30 => 0x1E, // ]
        0x31 => 0x2A, // \
        0x33 => 0x29, // ;
        0x34 => 0x27, // '
        0x35 => 0x32, // `
        0x36 => 0x2B, // ,
        0x37 => 0x2F, // .
        0x38 => 0x2C, // /
        0x39 => 0x39, // Caps Lock
        0x3A => 0x7A, // F1
        0x3B => 0x78, // F2
        0x3C => 0x63, // F3
        0x3D => 0x76, // F4
        0x3E => 0x60, // F5
        0x3F => 0x61, // F6
        0x40 => 0x62, // F7
        0x41 => 0x64, // F8
        0x42 => 0x65, // F9
        0x43 => 0x6D, // F10
        0x44 => 0x67, // F11
        0x45 => 0x6F, // F12
        0x4A => 0x73, // Home
        0x4B => 0x74, // PageUp
        0x4C => 0x75, // Forward Delete
        0x4D => 0x77, // End
        0x4E => 0x79, // PageDown
        0x4F => 0x7C, // Right
        0x50 => 0x7B, // Left
        0x51 => 0x7D, // Down
        0x52 => 0x7E, // Up
        0xE0 => 0x3B, // LCtrl
        0xE1 => 0x38, // LShift
        0xE2 => 0x3A, // LAlt
        0xE3 => 0x37, // LMeta
        0xE4 => 0x3E, // RCtrl
        0xE5 => 0x3C, // RShift
        0xE6 => 0x3D, // RAlt
        0xE7 => 0x36, // RMeta
        _ => return None,
    };
    Some(mac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vk_a_roundtrips_via_hid() {
        let hid = vk_to_hid(0x41).unwrap();
        assert_eq!(hid, 0x04);
        assert_eq!(hid_to_vk(hid).unwrap(), 0x41);
    }

    #[test]
    fn mac_a_roundtrips_via_hid() {
        let hid = mac_to_hid(0x00).unwrap();
        assert_eq!(hid, 0x04);
        assert_eq!(hid_to_mac(hid).unwrap(), 0x00);
    }

    #[test]
    fn cross_platform_a_consistent() {
        // Letra A: VK_A → HID → mac_A; mac_A → HID → VK_A
        let hid_from_vk = vk_to_hid(0x41).unwrap();
        let hid_from_mac = mac_to_hid(0x00).unwrap();
        assert_eq!(hid_from_vk, hid_from_mac);
        assert_eq!(hid_to_mac(hid_from_vk).unwrap(), 0x00);
        assert_eq!(hid_to_vk(hid_from_mac).unwrap(), 0x41);
    }

    #[test]
    fn enter_consistent() {
        assert_eq!(vk_to_hid(0x0D).unwrap(), 0x28);
        assert_eq!(mac_to_hid(0x24).unwrap(), 0x28);
        assert_eq!(hid_to_vk(0x28).unwrap(), 0x0D);
        assert_eq!(hid_to_mac(0x28).unwrap(), 0x24);
    }

    #[test]
    fn lshift_consistent() {
        assert_eq!(vk_to_hid(0xA0).unwrap(), 0xE1);
        assert_eq!(mac_to_hid(0x38).unwrap(), 0xE1);
        assert_eq!(hid_to_vk(0xE1).unwrap(), 0xA0);
        assert_eq!(hid_to_mac(0xE1).unwrap(), 0x38);
    }

    #[test]
    fn unknown_key_returns_none() {
        assert_eq!(vk_to_hid(0xFFFF), None);
        assert_eq!(mac_to_hid(0xFFFF), None);
        assert_eq!(hid_to_vk(0xFFFF), None);
        assert_eq!(hid_to_mac(0xFFFF), None);
    }

    #[test]
    fn arrow_keys_full_roundtrip() {
        for (vk, mac, hid) in [
            (0x27u32, 0x7Cu32, 0x4Fu32), // Right
            (0x25, 0x7B, 0x50),          // Left
            (0x28, 0x7D, 0x51),          // Down
            (0x26, 0x7E, 0x52),          // Up
        ] {
            assert_eq!(vk_to_hid(vk).unwrap(), hid);
            assert_eq!(mac_to_hid(mac).unwrap(), hid);
            assert_eq!(hid_to_vk(hid).unwrap() as u32, vk);
            assert_eq!(hid_to_mac(hid).unwrap() as u32, mac);
        }
    }
}
