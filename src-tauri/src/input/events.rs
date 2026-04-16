use serde::{Deserialize, Serialize};

/// Botões do mouse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

/// Modificadores de teclado (bit flags)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Modifiers(pub u32);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const META: Self = Self(1 << 3); // Cmd (macOS) / Win (Windows)

    pub fn contains(&self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

/// Evento de input normalizado, agnóstico de plataforma
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum InputEvent {
    /// Movimento do mouse. x e y normalizados: 0.0 = início, 1.0 = fim da tela
    MouseMove { x: f32, y: f32 },
    /// Clique ou release de botão do mouse
    MouseButton { button: MouseButton, pressed: bool },
    /// Scroll. dx = horizontal, dy = vertical (positivo = cima/direita)
    MouseScroll { dx: f32, dy: f32 },
    /// Tecla. keycode segue USB HID (independente de layout)
    KeyEvent {
        keycode: u32,
        pressed: bool,
        modifiers: Modifiers,
    },
}
