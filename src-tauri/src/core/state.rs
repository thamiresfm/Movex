use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use crate::config::Settings;
use crate::network::protocol::Message;

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
    /// Token para cancelar a task de conexão em andamento
    pub cancel_token: Arc<Mutex<Option<CancellationToken>>>,
    /// Canal para enviar mensagens ao peer conectado (servidor → cliente)
    pub message_tx: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    /// Timestamp de início da sessão (para uptime real)
    pub session_started_at: Arc<Mutex<Option<std::time::Instant>>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            connection_status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            active_screen: Arc::new(Mutex::new(ActiveScreen::Local)),
            cancel_token: Arc::new(Mutex::new(None)),
            message_tx: Arc::new(Mutex::new(None)),
            session_started_at: Arc::new(Mutex::new(None)),
        }
    }

    /// Cancela qualquer task de conexão ativa
    pub async fn cancel_connection(&self) {
        let mut token = self.cancel_token.lock().await;
        if let Some(t) = token.take() {
            t.cancel();
        }
        let mut tx = self.message_tx.lock().await;
        *tx = None;
    }

    /// Cria e armazena novo token de cancelamento
    pub async fn new_cancel_token(&self) -> CancellationToken {
        let token = CancellationToken::new();
        let mut t = self.cancel_token.lock().await;
        *t = Some(token.clone());
        token
    }
}

pub type SharedState = Arc<AppState>;
