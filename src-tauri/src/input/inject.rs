use std::sync::OnceLock;
use crate::input::events::InputEvent;
use crate::input::platform::{create_injector, InputInjector};
use tracing::error;

/// Injector singleton — criado uma única vez, reutilizado em todas as chamadas
/// Evita alocação de Box<dyn InputInjector> a cada evento (centenas por segundo)
static INJECTOR: OnceLock<Box<dyn InputInjector>> = OnceLock::new();

fn get_injector() -> &'static dyn InputInjector {
    INJECTOR.get_or_init(create_injector).as_ref()
}

/// Injeta um evento de input no SO local como se fosse físico
pub fn inject_event(event: InputEvent) {
    if let Err(e) = get_injector().inject(event) {
        error!("Falha ao injetar evento: {}", e);
    }
}

/// Actualiza a sensibilidade do cursor para o lado receptor.
/// Chamado na inicialização a partir das Settings.
pub fn set_mouse_sensitivity(s: f64) {
    #[cfg(target_os = "macos")]
    crate::input::platform::macos::set_sensitivity(s as f32);
}
