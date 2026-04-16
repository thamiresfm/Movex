use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::core::state::{ConnectionStatus, SharedState, ActiveScreen};
use crate::network::protocol::{Message, PROTOCOL_VERSION};
use crate::network::transport::{
    create_tls_acceptor, generate_self_signed_cert, recv_message, send_message,
};

/// Inicia o servidor Movex e aguarda conexão de um cliente
pub async fn start(state: SharedState) -> Result<(), String> {
    let port = {
        let s = state.settings.lock().await;
        s.port
    };

    let (certs, key) = generate_self_signed_cert()
        .map_err(|e| format!("Erro ao gerar certificado TLS: {}", e))?;
    let acceptor = create_tls_acceptor(certs, key)
        .map_err(|e| format!("Erro ao criar TLS acceptor: {}", e))?;

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Falha ao escutar em {}: {}", addr, e))?;

    info!("Servidor Movex escutando em {}", addr);
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Connecting;
    }

    loop {
        match listener.accept().await {
            Ok((tcp_stream, peer_addr)) => {
                info!("Nova conexão TCP de {}", peer_addr);
                let tls_stream = match acceptor.accept(tcp_stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Falha no TLS handshake com {}: {}", peer_addr, e);
                        continue;
                    }
                };
                let state_clone = state.clone();
                tokio::spawn(async move {
                    handle_client(tls_stream, peer_addr, state_clone).await;
                });
            }
            Err(e) => error!("Erro ao aceitar conexão: {}", e),
        }
    }
}

async fn handle_client<S>(
    mut stream: S,
    peer_addr: SocketAddr,
    state: SharedState,
) where
    S: tokio::io::AsyncReadExt + tokio::io::AsyncWriteExt + Unpin,
{
    let hello = match recv_message(&mut stream).await {
        Ok(m) => m,
        Err(e) => {
            warn!("Erro ao receber Hello de {}: {}", peer_addr, e);
            return;
        }
    };

    let peer_hostname = match hello {
        Message::Hello { version, hostname, .. } => {
            if version != PROTOCOL_VERSION {
                let _ = send_message(&mut stream, &Message::HelloReject {
                    reason: format!(
                        "Versão incompatível: esperado {}, recebido {}",
                        PROTOCOL_VERSION, version
                    ),
                }).await;
                return;
            }
            hostname
        }
        _ => {
            warn!("Esperava Hello de {}", peer_addr);
            return;
        }
    };

    let our_hostname = {
        let s = state.settings.lock().await;
        s.hostname.clone()
    };

    let _ = send_message(&mut stream, &Message::HelloAck {
        version: PROTOCOL_VERSION,
        hostname: our_hostname,
        nonce: hex::encode(rand::random::<[u8; 16]>()),
    }).await;

    info!("Cliente autenticado: {} ({})", peer_hostname, peer_addr);
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Connected {
            peer_hostname: peer_hostname.clone(),
            latency_ms: 0,
        };
    }

    loop {
        match recv_message(&mut stream).await {
            Ok(Message::Ping) => {
                let _ = send_message(&mut stream, &Message::Pong).await;
            }
            Ok(Message::Disconnect { reason }) => {
                info!("Cliente desconectou: {}", reason);
                break;
            }
            Ok(Message::LeaveScreen) => {
                let mut active = state.active_screen.lock().await;
                *active = ActiveScreen::Local;
            }
            Ok(_) => {}
            Err(e) => {
                warn!("Erro ao receber de {}: {}", peer_addr, e);
                break;
            }
        }
    }

    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }
    info!("Conexão com {} encerrada", peer_addr);
}
