use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScreenResolution {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

impl Default for ScreenResolution {
    fn default() -> Self {
        Self { width: 1920, height: 1080, scale_factor: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PeerPosition {
    Left,
    #[default]
    Right,
    Above,
    Below,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenLayout {
    pub local: ScreenResolution,
    pub peer: Option<ScreenResolution>,
    pub peer_position: PeerPosition,
}

impl Default for ScreenLayout {
    fn default() -> Self {
        Self {
            local: ScreenResolution::default(),
            peer: None,
            peer_position: PeerPosition::default(),
        }
    }
}

impl ScreenLayout {
    pub fn to_local_pixels(&self, x_norm: f32, y_norm: f32) -> (f32, f32) {
        (x_norm * self.local.width as f32, y_norm * self.local.height as f32)
    }

    pub fn to_normalized(&self, x_px: f32, y_px: f32) -> (f32, f32) {
        (x_px / self.local.width as f32, y_px / self.local.height as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_roundtrip() {
        let layout = ScreenLayout::default();
        let (nx, ny) = layout.to_normalized(960.0, 540.0);
        assert!((nx - 0.5).abs() < 0.001);
        assert!((ny - 0.5).abs() < 0.001);
        let (px, py) = layout.to_local_pixels(nx, ny);
        assert!((px - 960.0).abs() < 0.1);
        assert!((py - 540.0).abs() < 0.1);
    }
}
