use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::Settings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected { peer_hostname: String, latency_ms: u32 },
    Reconnecting { attempt: u32 },
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Desconectado"),
            Self::Connecting => write!(f, "Conectando..."),
            Self::Connected { peer_hostname, latency_ms } => {
                write!(f, "Conectado a {} ({}ms)", peer_hostname, latency_ms)
            }
            Self::Reconnecting { attempt } => {
                write!(f, "Reconectando... (tentativa {})", attempt)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveScreen {
    Local,
    Remote,
}

#[derive(Debug)]
pub struct AppState {
    pub settings: Arc<Mutex<Settings>>,
    pub connection_status: Arc<Mutex<ConnectionStatus>>,
    pub active_screen: Arc<Mutex<ActiveScreen>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            connection_status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            active_screen: Arc::new(Mutex::new(ActiveScreen::Local)),
        }
    }
}

pub type SharedState = Arc<AppState>;
