use crate::input::events::InputEvent;
use crate::input::platform::create_injector;
use tracing::error;

/// Injeta um evento de input no SO local como se fosse físico.
pub fn inject_event(event: InputEvent) {
    let injector = create_injector();
    if let Err(e) = injector.inject(event) {
        error!("Falha ao injetar evento de input: {}", e);
    }
}
