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
            Self::Left => write!(f, "left"),
            Self::Above => write!(f, "above"),
            Self::Below => write!(f, "below"),
        }
    }
}

impl From<&str> for ScreenPosition {
    fn from(s: &str) -> Self {
        match s {
            "left" => Self::Left,
            "above" => Self::Above,
            "below" => Self::Below,
            _ => Self::Right,
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
    /// Fingerprint SHA-256 (hex) do certificado TLS do servidor — TOFU
    /// None = primeira conexão (aceita qualquer cert e armazena)
    /// Some(fp) = rejeita se o cert apresentado divergir
    #[serde(default)]
    pub server_cert_fingerprint: Option<String>,
    /// Windows: já foi apresentado o pedido automático de regras no firewall (UAC) na primeira conexão.
    #[serde(default)]
    pub windows_firewall_prompt_done: bool,
    /// Multiplicador de velocidade do cursor no lado receptor.
    /// 1.0 = proporcional ao ecrã remoto (padrão neutral).
    /// Aumente para 1.2-1.5 se o cursor parecer "pesado" ou lento no Mac quando
    /// controlado pelo Windows (ecrãs com tamanhos diferentes).
    #[serde(default = "default_mouse_sensitivity")]
    pub mouse_sensitivity: f64,
}

fn default_mouse_sensitivity() -> f64 {
    1.2
}

impl Default for Settings {
    fn default() -> Self {
        let hn = get_hostname();
        Self {
            schema_version: 7,
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
            server_cert_fingerprint: None,
            windows_firewall_prompt_done: false,
            mouse_sensitivity: default_mouse_sensitivity(),
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
        let base = dirs::home_dir().unwrap_or_else(|| {
            // Sem diretório home: evita gravar a PSK no diretório atual (imprevisível).
            // Usa o diretório temporário do sistema como fallback seguro.
            let tmp = std::env::temp_dir();
            warn!(
                "Diretório home indisponível — a usar diretório temporário como fallback para config"
            );
            tmp
        });
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
                        // Valida o formato da PSK carregada (64 chars hex = 32 bytes).
                        // Apenas regista o aviso — NÃO regenera aqui para não quebrar
                        // uma sessão emparelhada existente.
                        if s.psk_hex.len() != 64
                            || !s.psk_hex.chars().all(|c| c.is_ascii_hexdigit())
                        {
                            warn!("PSK carregada com formato inválido (esperado 64 chars hex)");
                        }
                        if s.schema_version < 2 {
                            s.schema_version = 2;
                            s.notifications_enabled = true;
                            // Só define o atalho padrão se ainda não houver um valor,
                            // para não sobrescrever uma configuração existente do utilizador.
                            if s.lock_key.is_empty() {
                                s.lock_key = "ctrl+alt+l".to_string();
                            }
                            s.recent_peers = vec![];
                            if let Err(e) = s.save() {
                                warn!("Falha ao persistir migração v2: {}", e);
                            }
                        }
                        if s.schema_version < 3 {
                            s.schema_version = 3;
                            s.clipboard_sync_enabled = true;
                            if let Err(e) = s.save() {
                                warn!("Falha ao persistir migração v3: {}", e);
                            }
                        }
                        if s.schema_version < 4 {
                            s.schema_version = 4;
                            s.server_cert_fingerprint = None;
                            if let Err(e) = s.save() {
                                warn!("Falha ao persistir migração v4: {}", e);
                            }
                        }
                        if s.schema_version < 5 {
                            s.schema_version = 5;
                            s.screen_name = s.hostname.clone();
                            s.expected_client_screen_name = None;
                            // Manter o comportamento anterior: quem já usava o app tinha sessão ao abrir
                            s.launch_connection_on_startup = true;
                            if let Err(e) = s.save() {
                                warn!("Falha ao persistir migração v5: {}", e);
                            }
                        }
                        if s.schema_version < 6 {
                            s.schema_version = 6;
                            s.windows_firewall_prompt_done = false;
                            if let Err(e) = s.save() {
                                warn!("Falha ao persistir migração v6: {}", e);
                            }
                        }
                        if s.schema_version < 7 {
                            s.schema_version = 7;
                            s.mouse_sensitivity = default_mouse_sensitivity();
                            if let Err(e) = s.save() {
                                warn!("Falha ao persistir migração v7: {}", e);
                            }
                        }
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
        // Escrever em arquivo temporário e renomear atomicamente para evitar corrupção
        let tmp = path.with_extension("json.tmp");
        // Garante que os dados foram efetivamente gravados em disco (fsync) ANTES do
        // rename, para o rename não apontar para conteúdo ainda em cache do SO
        // (corrupção em caso de crash/queda de energia).
        {
            use std::io::Write;
            // Grava, faz flush do buffer e força fsync sequencialmente; qualquer falha
            // remove o arquivo temporário e propaga o erro. O fsync garante que os
            // bytes estão em disco ANTES do rename.
            let write_result = (|| -> std::io::Result<()> {
                let mut file = std::fs::File::create(&tmp)?;
                file.write_all(json.as_bytes())?;
                file.flush()?;
                file.sync_all()?;
                Ok(())
            })();
            if let Err(e) = write_result {
                let _ = std::fs::remove_file(&tmp);
                return Err(e.to_string());
            }
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(())
    }

    /// Adiciona ou atualiza peer no histórico (mantém max 10, ordenado por recência)
    pub fn add_recent_peer(&mut self, hostname: &str, addr: &str, port: u16) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.recent_peers.retain(|p| p.addr != addr);
        self.recent_peers.insert(
            0,
            RecentPeer {
                hostname: hostname.to_string(),
                addr: addr.to_string(),
                port,
                last_connected: now,
            },
        );
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
        assert_eq!(restored.schema_version, 7);
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
