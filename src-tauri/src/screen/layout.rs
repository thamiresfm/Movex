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

/// Monitor individual com posição e resolução
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub id: u32,
    pub x: i32,           // posição X no espaço virtual
    pub y: i32,           // posição Y no espaço virtual
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub is_primary: bool,
}

/// Configuração de múltiplos monitores
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultiMonitorLayout {
    pub monitors: Vec<Monitor>,
}

#[allow(dead_code)]
impl MultiMonitorLayout {
    /// Retorna o monitor que contém o ponto (x, y) em pixels absolutos
    pub fn monitor_at(&self, x: i32, y: i32) -> Option<&Monitor> {
        self.monitors.iter().find(|m| {
            x >= m.x && x < m.x + m.width as i32
            && y >= m.y && y < m.y + m.height as i32
        })
    }

    /// Bounding box de todos os monitores
    pub fn bounding_box(&self) -> (i32, i32, u32, u32) {
        if self.monitors.is_empty() {
            return (0, 0, 1920, 1080);
        }
        let min_x = self.monitors.iter().map(|m| m.x).min().unwrap_or(0);
        let min_y = self.monitors.iter().map(|m| m.y).min().unwrap_or(0);
        let max_x = self.monitors.iter().map(|m| m.x + m.width as i32).max().unwrap_or(1920);
        let max_y = self.monitors.iter().map(|m| m.y + m.height as i32).max().unwrap_or(1080);
        (min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32)
    }

    /// Normaliza coordenadas absolutas para 0.0-1.0 relativo ao bounding box
    pub fn to_normalized(&self, x: i32, y: i32) -> (f32, f32) {
        let (bx, by, bw, bh) = self.bounding_box();
        (
            (x - bx) as f32 / bw as f32,
            (y - by) as f32 / bh as f32,
        )
    }

    /// Converte coordenadas normalizadas de volta para absolutas
    pub fn to_absolute(&self, nx: f32, ny: f32) -> (i32, i32) {
        let (bx, by, bw, bh) = self.bounding_box();
        (
            bx + (nx * bw as f32) as i32,
            by + (ny * bh as f32) as i32,
        )
    }
}

/// Detecta monitores disponíveis na plataforma atual
pub fn detect_monitors() -> MultiMonitorLayout {
    #[cfg(target_os = "macos")]
    return detect_monitors_macos();
    #[cfg(target_os = "windows")]
    return detect_monitors_windows();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    MultiMonitorLayout { monitors: vec![Monitor { id: 0, x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1.0, is_primary: true }] }
}

#[cfg(target_os = "macos")]
fn detect_monitors_macos() -> MultiMonitorLayout {
    use core_graphics::display::CGDisplay;

    // Obter lista de todos os displays ativos via CGGetActiveDisplayList
    let displays = CGDisplay::active_displays().unwrap_or_default();
    let main_id = CGDisplay::main().id;

    let mut monitors: Vec<Monitor> = displays.iter().enumerate().map(|(_i, &id)| {
        let display = CGDisplay::new(id);
        let bounds = display.bounds();
        let scale = if bounds.size.width > 0.0 {
            display.pixels_wide() as f32 / bounds.size.width as f32
        } else { 1.0 };
        Monitor {
            id,
            x: bounds.origin.x as i32,
            y: bounds.origin.y as i32,
            width: display.pixels_wide() as u32,
            height: display.pixels_high() as u32,
            scale_factor: scale,
            is_primary: id == main_id,
        }
    }).collect();

    if monitors.is_empty() {
        let d = CGDisplay::main();
        monitors.push(Monitor { id: d.id, x: 0, y: 0, width: d.pixels_wide() as u32, height: d.pixels_high() as u32, scale_factor: 1.0, is_primary: true });
    }

    MultiMonitorLayout { monitors }
}

#[cfg(target_os = "windows")]
fn detect_monitors_windows() -> MultiMonitorLayout {
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW, MONITORINFO,
        HDC, HMONITOR,
    };
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};

    let mut monitors: Vec<Monitor> = Vec::new();

    unsafe extern "system" fn monitor_callback(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(data.0 as *mut Vec<Monitor>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
            let r = info.monitorInfo.rcMonitor;
            let is_primary = info.monitorInfo.dwFlags & 1 != 0;
            monitors.push(Monitor {
                id: monitors.len() as u32,
                x: r.left,
                y: r.top,
                width: (r.right - r.left) as u32,
                height: (r.bottom - r.top) as u32,
                scale_factor: 1.0,
                is_primary,
            });
        }
        BOOL(1)
    }

    unsafe {
        EnumDisplayMonitors(None, None, Some(monitor_callback), LPARAM(&mut monitors as *mut Vec<Monitor> as isize));
    }

    if monitors.is_empty() {
        monitors.push(Monitor { id: 0, x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1.0, is_primary: true });
    }

    MultiMonitorLayout { monitors }
}

// ── Layout legado (mantido para compatibilidade) ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PeerPosition { Left, #[default] Right, Above, Below }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenLayout {
    pub local: ScreenResolution,
    pub peer: Option<ScreenResolution>,
    pub peer_position: PeerPosition,
}

#[allow(dead_code)]
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

    #[test]
    fn multi_monitor_bounding_box() {
        let layout = MultiMonitorLayout {
            monitors: vec![
                Monitor { id: 0, x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1.0, is_primary: true },
                Monitor { id: 1, x: 1920, y: 0, width: 2560, height: 1440, scale_factor: 1.0, is_primary: false },
            ],
        };
        let (x, y, w, h) = layout.bounding_box();
        assert_eq!(x, 0); assert_eq!(y, 0);
        assert_eq!(w, 4480); assert_eq!(h, 1440);
    }

    #[test]
    fn multi_monitor_normalized_coords() {
        let layout = MultiMonitorLayout {
            monitors: vec![
                Monitor { id: 0, x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1.0, is_primary: true },
            ],
        };
        let (nx, ny) = layout.to_normalized(960, 540);
        assert!((nx - 0.5).abs() < 0.001);
        assert!((ny - 0.5).abs() < 0.001);
    }
}
