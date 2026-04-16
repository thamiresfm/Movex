use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::core::state::{ActiveScreen, ConnectionStatus, SharedState};
use crate::input::inject::inject_event;
use crate::network::protocol::{Message, PROTOCOL_VERSION};
use crate::network::reconnect::ReconnectPolicy;
use crate::network::transport::{create_tls_connector, recv_message, send_message};

/// Conecta ao servidor com reconexão automática
pub async fn connect(state: SharedState) {
    let policy = ReconnectPolicy::default();

    loop {
        let (server_addr, port) = {
            let s = state.settings.lock().await;
            (
                s.server_addr.clone().unwrap_or_else(|| "127.0.0.1".to_string()),
                s.port,
            )
        };

        let addr = format!("{}:{}", server_addr, port);
        info!("Conectando ao servidor Movex em {}...", addr);
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Connecting;
        }

        match TcpStream::connect(&addr).await {
            Ok(tcp) => {
                let connector = create_tls_connector();
                let domain = ServerName::try_from("movex.local")
                    .expect("domínio inválido");

                match connector.connect(domain, tcp).await {
                    Ok(mut tls) => {
                        let hostname = {
                            let s = state.settings.lock().await;
                            s.hostname.clone()
                        };

                        let hello = Message::Hello {
                            version: PROTOCOL_VERSION,
                            hostname,
                            nonce: hex::encode(rand::random::<[u8; 16]>()),
                        };

                        if let Err(e) = send_message(&mut tls, &hello).await {
                            warn!("Erro ao enviar Hello: {}", e);
                            continue;
                        }

                        match recv_message(&mut tls).await {
                            Ok(Message::HelloAck { hostname: peer, .. }) => {
                                info!("Conectado ao servidor: {}", peer);
                                policy.reset();
                                {
                                    let mut status = state.connection_status.lock().await;
                                    *status = ConnectionStatus::Connected {
                                        peer_hostname: peer,
                                        latency_ms: 0,
                                    };
                                }
                                run_session(&mut tls, state.clone()).await;
                            }
                            Ok(Message::HelloReject { reason }) => {
                                warn!("Rejeitado: {}", reason);
                            }
                            Ok(_) => warn!("Resposta inesperada ao Hello"),
                            Err(e) => warn!("Erro ao receber HelloAck: {}", e),
                        }
                    }
                    Err(e) => warn!("Falha no TLS handshake: {}", e),
                }
            }
            Err(e) => warn!("Falha ao conectar em {}: {}", addr, e),
        }

        let attempt = policy.attempt();
        {
            let mut status = state.connection_status.lock().await;
            *status = ConnectionStatus::Reconnecting { attempt };
        }
        let wait = policy.next_delay();
        info!("Reconectando em {}s...", wait.as_secs());
        tokio::time::sleep(wait).await;
    }
}

async fn run_session<S>(stream: &mut S, state: SharedState)
where
    S: tokio::io::AsyncReadExt + tokio::io::AsyncWriteExt + Unpin,
{
    loop {
        match recv_message(stream).await {
            Ok(Message::EnterScreen) => {
                let mut active = state.active_screen.lock().await;
                *active = ActiveScreen::Remote;
            }
            Ok(Message::LeaveScreen) => {
                let mut active = state.active_screen.lock().await;
                *active = ActiveScreen::Local;
            }
            Ok(Message::Input(event)) => inject_event(event),
            Ok(Message::Ping) => {
                let _ = send_message(stream, &Message::Pong).await;
            }
            Ok(Message::Disconnect { reason }) => {
                info!("Servidor desconectou: {}", reason);
                break;
            }
            Ok(_) => {}
            Err(e) => {
                warn!("Erro na sessão: {}", e);
                break;
            }
        }
    }
    let mut status = state.connection_status.lock().await;
    *status = ConnectionStatus::Disconnected;
}
