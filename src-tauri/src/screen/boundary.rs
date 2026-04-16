use crate::screen::layout::{PeerPosition, ScreenLayout};

const EDGE_THRESHOLD: f32 = 2.0;

#[derive(Debug, PartialEq)]
pub enum BoundaryResult {
    Local,
    CrossedToPeer { entry_x: f32, entry_y: f32 },
}

pub fn check_boundary(x_px: f32, y_px: f32, layout: &ScreenLayout) -> BoundaryResult {
    let w = layout.local.width as f32;
    let h = layout.local.height as f32;

    let crossed = match layout.peer_position {
        PeerPosition::Right  => x_px >= w - EDGE_THRESHOLD,
        PeerPosition::Left   => x_px <= EDGE_THRESHOLD,
        PeerPosition::Below  => y_px >= h - EDGE_THRESHOLD,
        PeerPosition::Above  => y_px <= EDGE_THRESHOLD,
    };

    if crossed {
        let (entry_x, entry_y) = match layout.peer_position {
            PeerPosition::Right  => (0.0, y_px / h),
            PeerPosition::Left   => (1.0, y_px / h),
            PeerPosition::Below  => (x_px / w, 0.0),
            PeerPosition::Above  => (x_px / w, 1.0),
        };
        BoundaryResult::CrossedToPeer { entry_x, entry_y }
    } else {
        BoundaryResult::Local
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::layout::{ScreenLayout, ScreenResolution};

    fn layout_1920x1080_right() -> ScreenLayout {
        ScreenLayout {
            local: ScreenResolution { width: 1920, height: 1080, scale_factor: 1.0 },
            peer: None,
            peer_position: PeerPosition::Right,
        }
    }

    #[test]
    fn centro_nao_cruza() {
        assert_eq!(check_boundary(960.0, 540.0, &layout_1920x1080_right()), BoundaryResult::Local);
    }

    #[test]
    fn borda_direita_cruza() {
        assert!(matches!(
            check_boundary(1919.0, 540.0, &layout_1920x1080_right()),
            BoundaryResult::CrossedToPeer { .. }
        ));
    }

    #[test]
    fn borda_esquerda_nao_cruza_se_peer_e_direita() {
        assert_eq!(check_boundary(0.0, 540.0, &layout_1920x1080_right()), BoundaryResult::Local);
    }

    #[test]
    fn posicao_entrada_normalizada() {
        match check_boundary(1919.0, 540.0, &layout_1920x1080_right()) {
            BoundaryResult::CrossedToPeer { entry_x, entry_y } => {
                assert!((entry_x - 0.0).abs() < 0.001);
                assert!((entry_y - 0.5).abs() < 0.01);
            }
            _ => panic!("deveria ter cruzado"),
        }
    }
}
