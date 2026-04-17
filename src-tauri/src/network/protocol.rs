use serde::{Deserialize, Serialize};
use crate::input::InputEvent;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAGIC: [u8; 4] = [0x46, 0x4C, 0x4F, 0x57]; // "FLOW"

#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Message {
    // Handshake
    /// Servidor envia primeiro: versão + nonce aleatório para o cliente computar HMAC
    ServerChallenge { version: u32, hostname: String, server_nonce: String },
    /// Cliente responde: hostname + HMAC(psk, server_nonce)
    Hello { version: u32, hostname: String, hmac: String },
    HelloAck { version: u32, hostname: String },
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

    // Aprovação de conexão
    /// Servidor aguarda aprovação do usuário antes de aceitar
    ConnectionPending { hostname: String },
    /// Usuário aprovou a conexão
    ConnectionApproved,
    /// Usuário rejeitou a conexão
    ConnectionRejected { reason: String },
}

pub const FILE_CHUNK_SIZE: usize = 64 * 1024;
pub const CLIPBOARD_MAX_BYTES: usize = 10 * 1024 * 1024;

impl Message {
    /// Serializa para bytes: magic (4) + length (4 big-endian) + payload (bincode)
    ///
    /// Retorna erro explícito se o payload exceder u32::MAX (4 GB) em vez de truncar.
    pub fn encode(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        let payload = bincode::encode_to_vec(self, bincode::config::standard())?;
        let length = u32::try_from(payload.len()).map_err(|_| {
            bincode::error::EncodeError::Other("payload excede limite de 4 GB (u32::MAX)")
        })?;
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
    fn encode_decode_input_key_event() {
        use crate::input::events::{InputEvent, Modifiers};
        let event = InputEvent::KeyEvent {
            keycode: 0x04, // 'a' em USB HID
            pressed: true,
            modifiers: Modifiers::CTRL,
        };
        let msg = Message::Input(event);
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes[8..]).unwrap();
        match decoded {
            Message::Input(InputEvent::KeyEvent { keycode, pressed, modifiers }) => {
                assert_eq!(keycode, 0x04);
                assert!(pressed);
                assert!(modifiers.contains(Modifiers::CTRL));
                assert!(!modifiers.contains(Modifiers::SHIFT));
            }
            _ => panic!("tipo errado"),
        }
    }

    #[test]
    fn encode_decode_file_end_checksum() {
        let checksum = [0xABu8; 32];
        let msg = Message::FileEnd { id: 42, checksum };
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes[8..]).unwrap();
        match decoded {
            Message::FileEnd { id, checksum: c } => {
                assert_eq!(id, 42);
                assert_eq!(c, [0xABu8; 32]);
            }
            _ => panic!("tipo errado"),
        }
    }

    #[test]
    fn encode_decode_hello() {
        let msg = Message::Hello {
            version: PROTOCOL_VERSION,
            hostname: "Estação Principal".to_string(),
            hmac: "deadbeef".to_string(),
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
