use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Server,
    Client,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScreenPosition {
    #[default]
    Right,
    Left,
    Above,
    Below,
}

impl std::fmt::Display for ScreenPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Right => write!(f, "right"),
            Self::Left  => write!(f, "left"),
            Self::Above => write!(f, "above"),
            Self::Below => write!(f, "below"),
        }
    }
}

impl From<&str> for ScreenPosition {
    fn from(s: &str) -> Self {
        match s {
            "left"  => Self::Left,
            "above" => Self::Above,
            "below" => Self::Below,
            _       => Self::Right,
        }
    }
}

/// Peer recente no histórico de conexões
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPeer {
    pub hostname: String,
    pub addr: String,
    pub port: u16,
    /// Timestamp Unix da última conexão
    pub last_connected: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    pub hostname: String,
    /// Nome do ecrã neste PC (como Barrier/Deskflow: identifica o cliente no handshake).
    /// Por defeito iguala ao hostname da máquina.
    #[serde(default)]
    pub screen_name: String,
    /// No servidor: se preenchido, só aceita clientes cujo `screen_name` coincida (exato, sem espaços nas pontas).
    #[serde(default)]
    pub expected_client_screen_name: Option<String>,
    /// Se verdadeiro, ao abrir o app com setup completo inicia servidor ou cliente automaticamente.
    /// Comportamento tipo Barrier: normalmente desligado (utilizador clica em Conectar).
    #[serde(default)]
    pub launch_connection_on_startup: bool,
    pub role: Role,
    pub server_addr: Option<String>,
    pub port: u16,
    pub psk_hex: String,
    pub peer_position: ScreenPosition,
    pub autostart: bool,
    pub theme: String,
    pub setup_complete: bool,
    pub notifications_enabled: bool,
    /// Atalho global para ativar/desativar modo lock, ex: "ctrl+alt+l"
    pub lock_key: String,
    pub clipboard_sync_enabled: bool,
    pub recent_peers: Vec<RecentPeer>,
    /// Não persiste como `true` — sempre resetado no load
    pub lock_mode: bool,
    /// Fingerprint SHA-256 (hex) do certificado TLS do servidor — TOFU
    /// None = primeira conexão (aceita qualquer cert e armazena)
    /// Some(fp) = rejeita se o cert apresentado divergir
    #[serde(default)]
    pub server_cert_fingerprint: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let hn = get_hostname();
        Self {
            schema_version: 5,
            hostname: hn.clone(),
            screen_name: hn,
            expected_client_screen_name: None,
            launch_connection_on_startup: false,
            role: Role::default(),
            server_addr: None,
            port: 24800,
            psk_hex: generate_psk(),
            peer_position: ScreenPosition::default(),
            autostart: false,
            theme: "dark".to_string(),
            setup_complete: false,
            notifications_enabled: true,
            lock_key: "ctrl+alt+l".to_string(),
            clipboard_sync_enabled: true,
            recent_peers: vec![],
            lock_mode: false,
            server_cert_fingerprint: None,
        }
    }
}

fn get_hostname() -> String {
    let name = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if name.trim().is_empty() {
        "Movex-Device".to_string()
    } else {
        name
    }
}

fn generate_psk() -> String {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    hex::encode(key)
}

impl Settings {
    pub fn config_path() -> PathBuf {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(".movex").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Settings>(&content) {
                    Ok(mut s) => {
                        if s.screen_name.trim().is_empty() {
                            s.screen_name = s.hostname.clone();
                        }
                        if s.schema_version < 2 {
                            s.schema_version = 2;
                            s.notifications_enabled = true;
                            s.lock_key = "ctrl+alt+l".to_string();
                            s.recent_peers = vec![];
                            s.lock_mode = false;
                        }
                        if s.schema_version < 3 {
                            s.schema_version = 3;
                            s.clipboard_sync_enabled = true;
                        }
                        if s.schema_version < 4 {
                            s.schema_version = 4;
                            s.server_cert_fingerprint = None;
                            let _ = s.save();
                        }
                        if s.schema_version < 5 {
                            s.schema_version = 5;
                            s.screen_name = s.hostname.clone();
                            s.expected_client_screen_name = None;
                            // Manter o comportamento anterior: quem já usava o app tinha sessão ao abrir
                            s.launch_connection_on_startup = true;
                            let _ = s.save();
                        }
                        s.lock_mode = false;
                        info!("Configurações carregadas de {:?}", path);
                        return s;
                    }
                    Err(e) => warn!("Erro ao parsear config: {} — usando padrão", e),
                },
                Err(e) => warn!("Erro ao ler config: {} — usando padrão", e),
            }
        }
        let default = Self::default();
        let _ = default.save();
        default
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut to_save = self.clone();
        to_save.lock_mode = false;
        let json = serde_json::to_string_pretty(&to_save).map_err(|e| e.to_string())?;
        // Escrever em arquivo temporário e renomear atomicamente para evitar corrupção
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Adiciona ou atualiza peer no histórico (mantém max 10, ordenado por recência)
    pub fn add_recent_peer(&mut self, hostname: &str, addr: &str, port: u16) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.recent_peers.retain(|p| p.addr != addr);
        self.recent_peers.insert(0, RecentPeer {
            hostname: hostname.to_string(),
            addr: addr.to_string(),
            port,
            last_connected: now,
        });
        self.recent_peers.truncate(10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_have_valid_psk() {
        let s = Settings::default();
        assert_eq!(s.psk_hex.len(), 64);
        assert!(s.psk_hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn default_port_is_24800() {
        let s = Settings::default();
        assert_eq!(s.port, 24800);
    }

    #[test]
    fn settings_roundtrip_json() {
        let original = Settings::default();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.port, original.port);
        assert_eq!(restored.psk_hex, original.psk_hex);
        assert_eq!(restored.schema_version, 5);
        assert_eq!(restored.screen_name, original.screen_name);
    }

    #[test]
    fn add_recent_peer_keeps_max_10() {
        let mut s = Settings::default();
        for i in 0..15 {
            s.add_recent_peer(&format!("host{}", i), &format!("192.168.1.{}", i), 24800);
        }
        assert_eq!(s.recent_peers.len(), 10);
        assert_eq!(s.recent_peers[0].hostname, "host14"); // mais recente
    }
}
