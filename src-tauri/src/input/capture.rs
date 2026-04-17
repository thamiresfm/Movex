use tokio::sync::mpsc;
use tracing::{error, warn};

use crate::input::events::InputEvent;
use crate::input::platform::create_capture;

/// Capacidade do canal de input — limita memória usada se o consumer for lento.
/// 1024 eventos = ~2MB no pior caso; excedente é descartado com aviso.
const INPUT_CHANNEL_CAPACITY: usize = 1024;

/// Inicia captura global de input e retorna receiver do canal de eventos.
/// O canal é bounded (1024 slots) para evitar OOM se o consumer estiver lento.
pub fn start_capture() -> mpsc::Receiver<InputEvent> {
    let (tx, rx) = mpsc::channel::<InputEvent>(INPUT_CHANNEL_CAPACITY);
    let capture = create_capture();

    let callback = move |event: InputEvent| {
        if tx.try_send(event).is_err() {
            warn!("Canal de input cheio — evento descartado");
        }
    };

    if let Err(e) = capture.start(Box::new(callback)) {
        error!("Falha ao iniciar captura de input: {}", e);
    }

    rx
}
