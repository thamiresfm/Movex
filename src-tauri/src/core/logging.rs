//! Tracing: consola (desenvolvimento e diagnóstico) + ficheiro rotativo em `~/.movex/logs/`
//! em macOS, Windows e Linux.

use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// Inicializa `tracing` com duas saídas: stderr formatado e `movex.log` (rotação diária).
/// O worker do ficheiro fica vivo com `mem::forget` até ao fim do processo.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout = fmt::layer()
        .with_target(true)
        .with_line_number(true);

    let log_dir = dirs::home_dir()
        .map(|h| h.join(".movex").join("logs"))
        .unwrap_or_else(|| PathBuf::from(".movex").join("logs"));

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Movex: não foi possível criar a pasta de logs {:?}: {}", log_dir, e);
    }

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "movex.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let file = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_line_number(true);

    Registry::default()
        .with(filter)
        .with(stdout)
        .with(file)
        .init();

    std::mem::forget(guard);

    tracing::info!(
        target: "movex",
        "Logs também em {}",
        log_dir.join("movex.log").display()
    );
}
