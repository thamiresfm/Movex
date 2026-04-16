pub mod receiver;
pub mod sender;

pub use receiver::FileReceiver;
pub use sender::send_file;

use serde::{Deserialize, Serialize};

/// Progresso de uma transferência de arquivo em andamento
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub id: u32,
    pub name: String,
    pub total_bytes: u64,
    pub sent_bytes: u64,
    pub direction: TransferDirection,
}

impl TransferProgress {
    pub fn percent(&self) -> u8 {
        if self.total_bytes == 0 { return 0; }
        ((self.sent_bytes as f64 / self.total_bytes as f64) * 100.0) as u8
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferDirection {
    Sending,
    Receiving,
}
