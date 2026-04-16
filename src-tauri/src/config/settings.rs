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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    pub hostname: String,
    pub role: Role,
    pub server_addr: Option<String>,
    pub port: u16,
    pub psk_hex: String,
    pub peer_position: ScreenPosition,
    pub autostart: bool,
    pub theme: String,
    pub setup_complete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            hostname: get_hostname(),
            role: Role::default(),
            server_addr: None,
            port: 24800,
            psk_hex: generate_psk(),
            peer_position: ScreenPosition::default(),
            autostart: false,
            theme: "dark".to_string(),
            setup_complete: false,
        }
    }
}

fn get_hostname() -> String {
    hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
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
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(s) => {
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
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        info!("Configurações salvas em {:?}", path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_have_valid_psk() {
        let s = Settings::default();
        assert_eq!(s.psk_hex.len(), 64, "PSK deve ter 64 hex chars (32 bytes)");
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
        assert_eq!(restored.schema_version, 1);
    }
}
