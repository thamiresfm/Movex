pub mod auth;
pub mod client;
pub mod notifications;
pub mod server;
pub mod state;
pub mod stats;
pub mod utils;

pub use state::{ActiveScreen, AppState, ConnectionStatus, SharedState};
pub use stats::{SessionStats, StatsSnapshot, format_bytes, get_primary_screen_size};

