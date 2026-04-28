//! Contrato do rato entre ecrãs (sem rede). Valida geometria servidor ↔ cliente espelhado.

use crate::screen::boundary::{check_boundary, BoundaryResult};
use crate::screen::layout::{PeerPosition, ScreenLayout, ScreenResolution};

fn layout_fullhd(peer_position: PeerPosition) -> ScreenLayout {
    ScreenLayout {
        local: ScreenResolution {
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        },
        peer: None,
        peer_position,
    }
}

#[test]
fn servidor_segundo_pc_a_direita_cruzamento_borda_direita() {
    let layout = layout_fullhd(PeerPosition::Right);
    match check_boundary(1919.0, 540.0, &layout) {
        BoundaryResult::CrossedToPeer { entry_x, entry_y } => {
            assert!((entry_x - 0.0).abs() < f32::EPSILON);
            assert!((entry_y - 0.5).abs() < 0.01);
        }
        BoundaryResult::Local => panic!("deveria cruzar para o peer à direita"),
    }
}

#[test]
fn cliente_borda_retorno_espelha_servidor_peer_direita() {
    let client_return_peer = PeerPosition::Right.invert();
    assert_eq!(client_return_peer, PeerPosition::Left);

    let layout = layout_fullhd(client_return_peer);
    let px = 1.5_f32;
    let py = 540.0_f32;
    assert!(
        matches!(check_boundary(px, py, &layout), BoundaryResult::CrossedToPeer { .. }),
        "à esquerda do cliente deve activar LeaveScreen ao servidor quando o servidor tinha peer à direita"
    );
}
