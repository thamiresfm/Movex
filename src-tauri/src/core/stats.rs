use std::sync::atomic::{AtomicU64, Ordering};

/// Contadores de bytes transferidos na sessão atual
#[derive(Debug, Default)]
pub struct SessionStats {
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub events_sent: AtomicU64,
    pub events_received: AtomicU64,
    pub files_sent: AtomicU64,
    pub files_received: AtomicU64,
}

impl SessionStats {
    pub fn add_sent(&self, bytes: u64)     { self.bytes_sent.fetch_add(bytes, Ordering::Relaxed); }
    pub fn add_received(&self, bytes: u64) { self.bytes_received.fetch_add(bytes, Ordering::Relaxed); }
    pub fn inc_event_sent(&self)           { self.events_sent.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_event_received(&self)       { self.events_received.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_file_sent(&self)            { self.files_sent.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_file_received(&self)        { self.files_received.fetch_add(1, Ordering::Relaxed); }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            bytes_sent:      self.bytes_sent.load(Ordering::Relaxed),
            bytes_received:  self.bytes_received.load(Ordering::Relaxed),
            events_sent:     self.events_sent.load(Ordering::Relaxed),
            events_received: self.events_received.load(Ordering::Relaxed),
            files_sent:      self.files_sent.load(Ordering::Relaxed),
            files_received:  self.files_received.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
        self.events_sent.store(0, Ordering::Relaxed);
        self.events_received.store(0, Ordering::Relaxed);
        self.files_sent.store(0, Ordering::Relaxed);
        self.files_received.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsSnapshot {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub events_sent: u64,
    pub events_received: u64,
    pub files_sent: u64,
    pub files_received: u64,
}

/// Detecta resolução real do monitor principal
#[cfg(target_os = "macos")]
pub fn get_primary_screen_size() -> (u32, u32) {
    use core_graphics::display::CGDisplay;
    let d = CGDisplay::main();
    (d.pixels_wide() as u32, d.pixels_high() as u32)
}

#[cfg(target_os = "windows")]
pub fn get_primary_screen_size() -> (u32, u32) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        (GetSystemMetrics(SM_CXSCREEN) as u32, GetSystemMetrics(SM_CYSCREEN) as u32)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_primary_screen_size() -> (u32, u32) {
    (1920, 1080)
}

