use serde::{Deserialize, Serialize};
use crate::input::InputEvent;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAGIC: [u8; 4] = [0x46, 0x4C, 0x4F, 0x57]; // "FLOW"

#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Message {
    // Handshake
    Hello { version: u32, hostname: String, nonce: String },
    HelloAck { version: u32, hostname: String, nonce: String },
    HelloReject { reason: String },

    // Input
    Input(InputEvent),

    // Transição de tela
    EnterScreen,
    LeaveScreen,

    // Clipboard
    ClipboardData { mime: String, data: Vec<u8> },

    // Transferência de arquivos
    FileStart { id: u32, name: String, size: u64 },
    FileChunk { id: u32, seq: u32, data: Vec<u8> },
    FileEnd { id: u32, checksum: [u8; 32] },
    FileRetry { id: u32 },

    // Controle
    Ping,
    Pong,
    Disconnect { reason: String },
}

pub const FILE_CHUNK_SIZE: usize = 64 * 1024;
pub const CLIPBOARD_MAX_BYTES: usize = 10 * 1024 * 1024;

impl Message {
    /// Serializa para bytes: magic (4) + length (4 big-endian) + payload (bincode)
    pub fn encode(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        let payload = bincode::encode_to_vec(self, bincode::config::standard())?;
        let length = payload.len() as u32;
        let mut buf = Vec::with_capacity(8 + payload.len());
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&length.to_be_bytes());
        buf.extend_from_slice(&payload);
        Ok(buf)
    }

    /// Deserializa a partir do payload (sem magic/length)
    pub fn decode(payload: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        let (msg, _) = bincode::decode_from_slice(payload, bincode::config::standard())?;
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip_ping() {
        let msg = Message::Ping;
        let bytes = msg.encode().unwrap();
        assert_eq!(&bytes[0..4], &MAGIC);
        let len = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(len, bytes.len() - 8);
        let decoded = Message::decode(&bytes[8..]).unwrap();
        assert!(matches!(decoded, Message::Ping));
    }

    #[test]
    fn encode_decode_hello() {
        let msg = Message::Hello {
            version: PROTOCOL_VERSION,
            hostname: "Estação Principal".to_string(),
            nonce: "abc123".to_string(),
        };
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes[8..]).unwrap();
        match decoded {
            Message::Hello { version, hostname, .. } => {
                assert_eq!(version, PROTOCOL_VERSION);
                assert_eq!(hostname, "Estação Principal");
            }
            _ => panic!("tipo errado"),
        }
    }
}
