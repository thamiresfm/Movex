use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tracing::{debug, info, warn};
use std::collections::HashMap;

const SERVICE_TYPE: &str = "_movex._tcp.local.";

/// Prefere IPv4 na LAN; evita mostrar só IPv6 (comum no Windows) quando há IPv4 disponível.
fn pick_preferred_lan_address(addrs: &[std::net::IpAddr]) -> Option<std::net::IpAddr> {
    addrs
        .iter()
        .copied()
        .find(std::net::IpAddr::is_ipv4)
        .or_else(|| addrs.first().copied())
}

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub hostname: String,
    pub addr: String,
    pub port: u16,
}

/// Anuncia este servidor na rede local via mDNS
pub fn announce_server(hostname: &str, port: u16) -> Result<ServiceDaemon, String> {
    let mdns = ServiceDaemon::new().map_err(|e| format!("mDNS daemon: {}", e))?;
    let name = format!("movex-{}", hostname.replace(' ', "-").to_lowercase());
    let service = ServiceInfo::new(SERVICE_TYPE, &name, hostname, "", port, None)
        .map_err(|e| format!("ServiceInfo: {}", e))?;
    mdns.register(service).map_err(|e| format!("mDNS register: {}", e))?;
    info!("Servidor anunciado via mDNS: {} porta {}", hostname, port);
    Ok(mdns)
}

/// Descobre servidores Movex na rede local (aguarda até timeout_secs)
/// Usa spawn_blocking para não bloquear o runtime Tokio
pub async fn discover_peers(timeout_secs: u64) -> Vec<DiscoveredPeer> {
    tokio::task::spawn_blocking(move || {
        discover_peers_blocking(timeout_secs)
    })
    .await
    .unwrap_or_default()
}

fn discover_peers_blocking(timeout_secs: u64) -> Vec<DiscoveredPeer> {
    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => { warn!("mDNS não disponível: {}", e); return vec![]; }
    };
    let receiver = match mdns.browse(SERVICE_TYPE) {
        Ok(r) => r,
        Err(e) => { warn!("Erro browse mDNS: {}", e); return vec![]; }
    };

    let mut peers: HashMap<String, DiscoveredPeer> = HashMap::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let addrs: Vec<std::net::IpAddr> =
                    info.get_addresses().iter().copied().collect();
                if let Some(addr) = pick_preferred_lan_address(&addrs) {
                    let peer = DiscoveredPeer {
                        hostname: info.get_hostname().trim_end_matches('.').to_string(),
                        addr: addr.to_string(),
                        port: info.get_port(),
                    };
                    info!("Peer descoberto: {} ({}:{})", peer.hostname, peer.addr, peer.port);
                    peers.insert(peer.hostname.clone(), peer);
                }
            }
            Ok(ServiceEvent::ServiceRemoved(_, name)) => {
                peers.remove(&name);
            }
            Ok(other) => {
                debug!("mDNS: evento {:?}", other);
            }
            Err(recv_err) => {
                let msg = format!("{:?}", recv_err);
                if msg.contains("Timeout") || msg.contains("timeout") {
                    // recv_timeout retorna Timeout normalmente a cada 100ms — não é erro
                } else {
                    warn!("mDNS canal fechado inesperadamente: {} — encerrando descoberta", msg);
                    break;
                }
            }
        }
    }
    if let Err(e) = mdns.stop_browse(SERVICE_TYPE) {
        warn!("mDNS: falha ao parar browse: {}", e);
    }
    peers.into_values().collect()
}
