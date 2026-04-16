#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

use crate::input::InputEvent;

/// Interface de captura de input (implementada por cada plataforma)
pub trait InputCapture: Send + Sync {
    fn start(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync + 'static>) -> Result<(), String>;
    fn stop(&self);
    fn lock_cursor(&self);
    fn unlock_cursor(&self);
}

/// Interface de injeção de input
pub trait InputInjector: Send + Sync {
    fn inject(&self, event: InputEvent) -> Result<(), String>;
}

/// Cria a implementação de captura para a plataforma atual
pub fn create_capture() -> Box<dyn InputCapture> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOsCapture::new());

    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsCapture::new());

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    panic!("Plataforma não suportada");
}

/// Cria a implementação de injeção para a plataforma atual
pub fn create_injector() -> Box<dyn InputInjector> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOsInjector::new());

    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsInjector::new());

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    panic!("Plataforma não suportada");
}
