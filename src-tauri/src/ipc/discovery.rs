#[derive(serde::Serialize, Clone)]
pub struct PeerInfo {
    pub hostname: String,
    pub addr: String,
    pub port: u16,
}

/// Descobre servidores Movex na rede local via mDNS (timeout: 3s)
/// usa spawn_blocking porque discover_peers internamente faz I/O síncrono
#[tauri::command]
pub async fn discover_peers() -> Result<Vec<PeerInfo>, String> {
    let peers = crate::network::discovery::discover_peers(3).await;
    Ok(peers.into_iter().map(|p| PeerInfo {
        hostname: p.hostname,
        addr: p.addr,
        port: p.port,
    }).collect())
}

/// IPv4 desta máquina na LAN (para exibir em "Conectar rede" / diagnóstico).
#[tauri::command]
pub fn get_local_ipv4_addrs() -> Vec<String> {
    crate::network::local_addrs::local_ipv4_strings()
}
