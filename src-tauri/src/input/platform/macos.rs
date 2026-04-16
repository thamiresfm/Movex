use std::sync::{Arc, Mutex};
use tracing::info;

use crate::input::events::InputEvent;
use super::{InputCapture, InputInjector};

pub struct MacOsCapture {
    locked: Arc<Mutex<bool>>,
}

impl MacOsCapture {
    pub fn new() -> Self {
        Self {
            locked: Arc::new(Mutex::new(false)),
        }
    }
}

impl InputCapture for MacOsCapture {
    fn start(&self, _callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        // CGEventTap requer permissão de Acessibilidade do SO em runtime.
        // A implementação completa usa CGEventTapCreate (unsafe) num thread dedicado.
        // Esta versão estrutural loga a intenção sem acionar o tap.
        info!("MacOsCapture: iniciando captura de input (requer permissão de Acessibilidade)");
        Ok(())
    }

    fn stop(&self) {
        info!("MacOsCapture: captura encerrada");
    }

    fn lock_cursor(&self) {
        let mut locked = self.locked.lock().unwrap();
        *locked = true;
        info!("MacOsCapture: cursor bloqueado na borda");
    }

    fn unlock_cursor(&self) {
        let mut locked = self.locked.lock().unwrap();
        *locked = false;
        info!("MacOsCapture: cursor liberado");
    }
}

pub struct MacOsInjector;

impl MacOsInjector {
    pub fn new() -> Self {
        Self
    }
}

impl InputInjector for MacOsInjector {
    fn inject(&self, event: InputEvent) -> Result<(), String> {
        // Injeção via CGEventCreateMouseEvent / CGEventCreateKeyboardEvent (unsafe).
        // Esta versão estrutural loga o evento sem injetar.
        match &event {
            InputEvent::MouseMove { x, y } => {
                info!("MacOsInjector: MouseMove ({:.3}, {:.3})", x, y);
            }
            InputEvent::MouseButton { button, pressed } => {
                info!(
                    "MacOsInjector: MouseButton {:?} {}",
                    button,
                    if *pressed { "down" } else { "up" }
                );
            }
            InputEvent::MouseScroll { dx, dy } => {
                info!("MacOsInjector: MouseScroll ({:.3}, {:.3})", dx, dy);
            }
            InputEvent::KeyEvent { keycode, pressed, modifiers } => {
                info!(
                    "MacOsInjector: KeyEvent {} {} mods={}",
                    keycode,
                    if *pressed { "down" } else { "up" },
                    modifiers.0
                );
            }
        }
        Ok(())
    }
}
