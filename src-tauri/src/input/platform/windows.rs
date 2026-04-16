use tracing::info;
use crate::input::events::InputEvent;
use super::{InputCapture, InputInjector};

pub struct WindowsCapture;

impl WindowsCapture {
    pub fn new() -> Self {
        Self
    }
}

impl InputCapture for WindowsCapture {
    fn start(&self, _callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String> {
        info!("WindowsCapture: stub — implementar na Task 5 Windows");
        Ok(())
    }

    fn stop(&self) {}

    fn lock_cursor(&self) {}

    fn unlock_cursor(&self) {}
}

pub struct WindowsInjector;

impl WindowsInjector {
    pub fn new() -> Self {
        Self
    }
}

impl InputInjector for WindowsInjector {
    fn inject(&self, event: InputEvent) -> Result<(), String> {
        info!("WindowsInjector: stub {:?}", event);
        Ok(())
    }
}
