use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::OnceLock;

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

/// Ponto do espaço virtual (como `MSLLHOOKSTRUCT.pt` no Windows) normalizado
/// 0.0..=1.0 relativamente a um ecrã (ex. monitor primário). `Y` cresce para baixo.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn normalize_point_against_display_rect(
    left: i32, top: i32, width: u32, height: u32, x: i32, y: i32,
) -> (f32, f32) {
    let wf = width.max(1) as f32;
    let hf = height.max(1) as f32;
    let nx = ((x - left) as f32 / wf).clamp(0.0, 1.0);
    let ny = ((y - top) as f32 / hf).clamp(0.0, 1.0);
    (nx, ny)
}

/// Bounding box cache da área de trabalho **virtual completa** (todos os monitores).
/// Necessário para detecção de borda KMS: com multi-monitor, o utilizador pode mover o
/// cursor entre monitores no SO, mas a normalização relativa apenas ao rect do primário
/// impedia detectar corretamente a borda física onde está o outro PC — o Barrier usa o
/// mesmo princípio (desktop virtual → bounding box).
///
/// Apenas compilado em macOS/Windows: no Linux CI o input KMS usa apenas stubs e isto ficaria morto (`dead_code`).
///
/// Calculado só na primeira utilização (tal como antes com apenas o primário).
#[cfg(any(target_os = "macos", target_os = "windows"))]
static DESKTOP_BBOX_CACHE: OnceLock<(i32, i32, u32, u32)> = OnceLock::new();

/// Origem `(min_x, min_y)` e tamanho `(width, height)` do rect que engloba todos os monitores.
///
/// Limitação conhecida: o cache não é invalidado em hotplug de monitores. Reiniciar a
/// aplicação ou reconectar para atualizar o layout após adicionar/remover monitores.
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[inline]
pub fn desktop_bounding_box_cached() -> (i32, i32, u32, u32) {
    *DESKTOP_BBOX_CACHE.get_or_init(|| detect_monitors().bounding_box())
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

    let mut monitors: Vec<Monitor> = displays.iter().map(|&id| {
        let display = CGDisplay::new(id);
        let bounds = display.bounds();
        // Usar PONTOS lógicos (bounds.size) e não pixels físicos (pixels_wide).
        // O sistema de coordenadas Quartz (eventos de rato, warp, normalize) opera
        // em pontos — em Retina 2×, pixels_wide = 2 × bounds.size.width. Misturar
        // estas unidades causava un factor de escala 2× errado no check_boundary
        // (entry_y calculado contra h=2160 mas normalizado contra h=1080 pontos).
        let scale = if bounds.size.width > 0.0 {
            display.pixels_wide() as f32 / bounds.size.width as f32
        } else { 1.0 };
        Monitor {
            id,
            x: bounds.origin.x as i32,
            y: bounds.origin.y as i32,
            width:  bounds.size.width  as u32,   // pontos lógicos (coerente com Quartz)
            height: bounds.size.height as u32,   // pontos lógicos
            scale_factor: scale,
            is_primary: id == main_id,
        }
    }).collect();

    if monitors.is_empty() {
        // Usar bounds (pontos lógicos) mesmo no fallback — consistente com Quartz.
        let d = CGDisplay::main();
        let b = d.bounds();
        let scale = if b.size.width > 0.0 { d.pixels_wide() as f32 / b.size.width as f32 } else { 1.0 };
        monitors.push(Monitor {
            id: d.id,
            x: b.origin.x as i32,
            y: b.origin.y as i32,
            width:  b.size.width  as u32,
            height: b.size.height as u32,
            scale_factor: scale,
            is_primary: true,
        });
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
        use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
        let monitors = &mut *(data.0 as *mut Vec<Monitor>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
            let r = info.monitorInfo.rcMonitor;
            let is_primary = info.monitorInfo.dwFlags & 1 != 0;
            let mut dpi_x: u32 = 96;
            let mut dpi_y: u32 = 96;
            let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            monitors.push(Monitor {
                id: monitors.len() as u32,
                x: r.left,
                y: r.top,
                width: (r.right - r.left) as u32,
                height: (r.bottom - r.top) as u32,
                scale_factor: dpi_x as f32 / 96.0,
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

impl PeerPosition {
    /// Espelha o eixo: se no servidor o outro monitor está à direita, no cliente a borda de retorno é a esquerda.
    pub fn invert(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
            Self::Above => Self::Below,
            Self::Below => Self::Above,
        }
    }
}

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

    /// Regressão: primário com offset no espaço virtual; `pt/1920` dava ~1,5 no centro
    /// (código antigo) e a borda nunca alinhava com o servidor.
    #[test]
    fn normalize_against_offset_primary_gives_center() {
        let (nx, ny) = normalize_point_against_display_rect(1920, 0, 1920, 1080, 2880, 540);
        assert!((nx - 0.5).abs() < 0.01, "nx={}", nx);
        assert!((ny - 0.5).abs() < 0.01, "ny={}", ny);
    }

    #[test]
    fn normalize_against_offset_primary_right_edge() {
        let (nx, _ny) = normalize_point_against_display_rect(1920, 0, 1920, 1080, 3839, 100);
        assert!(nx > 0.99, "nx={} devia aproximar 1.0 na borda direita", nx);
    }

    #[test]
    fn peer_position_invert_es_involutivo() {
        assert_eq!(PeerPosition::Right.invert(), PeerPosition::Left);
        assert_eq!(PeerPosition::Left.invert(), PeerPosition::Right);
        assert_eq!(PeerPosition::Above.invert(), PeerPosition::Below);
        assert_eq!(PeerPosition::Below.invert(), PeerPosition::Above);
        assert_eq!(PeerPosition::Right.invert().invert(), PeerPosition::Right);
    }
}
