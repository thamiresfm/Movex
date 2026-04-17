use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tauri::AppHandle;
use crate::config::Settings;
use crate::network::protocol::Message;
use crate::transfer::TransferProgress;
use crate::core::stats::SessionStats;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    /// Handshake TLS em andamento — aguardando aprovação do usuário
    PendingApproval { peer_hostname: String },
    Connected { peer_hostname: String, latency_ms: u32 },
    Reconnecting { attempt: u32 },
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Desconectado"),
            Self::Connecting => write!(f, "Conectando..."),
            Self::PendingApproval { peer_hostname } => {
                write!(f, "Aguardando aprovação de {}", peer_hostname)
            }
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
    /// Flag atômico para acesso lock-free no hot-path do callback de captura
    /// true = cursor está na tela remota, false = local
    pub active_screen_remote: Arc<std::sync::atomic::AtomicBool>,
    /// Token para cancelar a task de conexão em andamento
    pub cancel_token: Arc<Mutex<Option<CancellationToken>>>,
    /// Canal para enviar mensagens ao peer conectado
    pub message_tx: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    /// Timestamp de início da sessão (para uptime real)
    pub session_started_at: Arc<Mutex<Option<std::time::Instant>>>,
    /// Transferências de arquivo em andamento (id → progresso)
    pub transfers: Arc<Mutex<HashMap<u32, TransferProgress>>>,
    /// Próximo ID de transferência
    pub next_transfer_id: Arc<Mutex<u32>>,
    /// Hostname do cliente aguardando aprovação (Some = aguardando, None = nenhum)
    pub pending_approval: Arc<Mutex<Option<String>>>,
    /// Canal para enviar decisão de aprovação (true = aceitar, false = rejeitar)
    pub approval_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>,
    /// Estatísticas da sessão atual
    pub stats: Arc<SessionStats>,
    /// Flag de modo lock (pausar transição de cursor)
    pub lock_mode: Arc<std::sync::atomic::AtomicBool>,
    /// AppHandle para enviar notificações do sistema
    pub app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            connection_status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            active_screen: Arc::new(Mutex::new(ActiveScreen::Local)),
            active_screen_remote: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cancel_token: Arc::new(Mutex::new(None)),
            message_tx: Arc::new(Mutex::new(None)),
            session_started_at: Arc::new(Mutex::new(None)),
            transfers: Arc::new(Mutex::new(HashMap::new())),
            next_transfer_id: Arc::new(Mutex::new(1)),
            pending_approval: Arc::new(Mutex::new(None)),
            approval_tx: Arc::new(Mutex::new(None)),
            stats: Arc::new(SessionStats::default()),
            lock_mode: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Envia notificação do sistema se habilitado
    pub async fn send_notification(&self, title: &str, body: &str) {
        let notifications_on = self.settings.lock().await.notifications_enabled;
        if !notifications_on { return; }
        if let Some(app) = self.app_handle.lock().await.as_ref() {
            crate::core::notifications::notify(app, title, body);
        }
    }

    /// Gera próximo ID de transferência (thread-safe)
    pub async fn next_transfer_id(&self) -> u32 {
        let mut id = self.next_transfer_id.lock().await;
        let current = *id;
        *id = id.wrapping_add(1).max(1);
        current
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
