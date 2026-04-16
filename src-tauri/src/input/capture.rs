use tokio::sync::mpsc;
use tracing::error;

use crate::input::events::InputEvent;
use crate::input::platform::create_capture;

/// Inicia captura global de input e retorna receiver do canal de eventos.
/// O canal é ilimitado — eventos são descartados se o receiver não consumir rápido o suficiente.
pub fn start_capture() -> mpsc::UnboundedReceiver<InputEvent> {
    let (tx, rx) = mpsc::unbounded_channel::<InputEvent>();
    let capture = create_capture();

    let callback = move |event: InputEvent| {
        if tx.send(event).is_err() {
            // Canal fechado — captura deve parar
        }
    };

    if let Err(e) = capture.start(Box::new(callback)) {
        error!("Falha ao iniciar captura de input: {}", e);
    }

    rx
}
