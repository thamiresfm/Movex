use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tracing::{debug, error};

use crate::network::protocol::{Message, MAGIC};

#[allow(dead_code)]
pub const DEFAULT_PORT: u16 = 24800;

/// Limite de segurança: pacotes > 100MB são rejeitados
const MAX_PAYLOAD_SIZE: usize = 100 * 1024 * 1024;

/// Erros de transporte
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),
    #[error("Encode error: {0}")]
    Encode(#[from] bincode::error::EncodeError),
    #[error("Decode error: {0}")]
    Decode(#[from] bincode::error::DecodeError),
    #[error("Magic bytes inválidos")]
    InvalidMagic,
    #[error("Payload excede limite de {0} bytes")]
    PayloadTooLarge(usize),
    #[error("Conexão encerrada")]
    ConnectionClosed,
}

/// Carrega ou gera o certificado TLS do servidor, persistindo em `~/.movex/`.
///
/// O certificado é reutilizado entre reinicializações para que o TOFU do cliente
/// não seja invalidado a cada restart do servidor. Se os arquivos estiverem
/// corrompidos ou ausentes, um novo par é gerado e salvo.
pub fn load_or_generate_server_cert(
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), String> {
    let base = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".movex");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    let cert_path = base.join("server.crt");
    let key_path  = base.join("server.key");

    if cert_path.exists() && key_path.exists() {
        if let (Ok(cert_bytes), Ok(key_bytes)) = (
            std::fs::read(&cert_path),
            std::fs::read(&key_path),
        ) {
            let cert_der = CertificateDer::from(cert_bytes);
            if let Ok(key_der) = PrivateKeyDer::try_from(key_bytes) {
                tracing::debug!("Certificado TLS do servidor carregado de {:?}", cert_path);
                return Ok((vec![cert_der], key_der));
            }
        }
        tracing::warn!("Certificado TLS corrompido — gerando novo par");
    }

    // Gerar novo par e persistir
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["movex.local".to_string()])
            .map_err(|e| e.to_string())?;

    let cert_der = cert.der().clone();
    let key_bytes = key_pair.serialize_der();

    std::fs::write(&cert_path, cert_der.as_ref()).map_err(|e| e.to_string())?;
    std::fs::write(&key_path, &key_bytes).map_err(|e| e.to_string())?;
    tracing::info!("Novo certificado TLS gerado e salvo em {:?}", cert_path);

    let key_der = PrivateKeyDer::try_from(key_bytes)
        .map_err(|_| "Falha ao converter chave privada".to_string())?;

    Ok((vec![cert_der], key_der))
}

/// Gera certificado TLS autoassinado efêmero (uso em testes).
#[allow(dead_code)]
pub fn generate_self_signed_cert(
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), rcgen::Error> {
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["movex.local".to_string()])?;

    let cert_der = cert.der().clone();
    let key_bytes = key_pair.serialize_der();
    let key_der = PrivateKeyDer::try_from(key_bytes)
        .map_err(|_| rcgen::Error::CouldNotParseKeyPair)?;

    Ok((vec![cert_der], key_der))
}

/// Cria `TlsAcceptor` (modo servidor)
pub fn create_tls_acceptor(
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
) -> Result<TlsAcceptor, rustls::Error> {
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Calcula o fingerprint SHA-256 de um certificado DER (hex lowercase, 64 chars)
pub fn cert_fingerprint(cert: &CertificateDer<'_>) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(cert.as_ref());
    hex::encode(hash)
}

/// Cria `TlsConnector` com política TOFU (Trust On First Use).
///
/// - `known_fingerprint = None`  → primeira conexão: aceita o cert e retorna o fingerprint
///   para ser persistido em `settings.server_cert_fingerprint`.
/// - `known_fingerprint = Some(fp)` → conexões subsequentes: rejeita se o cert divergir.
///
/// O fingerprint é retornado via `TofuVerifier::observed` após o handshake.
pub fn create_tls_connector(known_fingerprint: Option<String>) -> (TlsConnector, Arc<TofuVerifier>) {
    let verifier = Arc::new(TofuVerifier::new(known_fingerprint));
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    (TlsConnector::from(Arc::new(config)), verifier)
}

/// Verifier TOFU: na primeira conexão aceita qualquer cert e registra seu fingerprint.
/// Em conexões subsequentes, rejeita se o fingerprint não bater.
#[derive(Debug)]
pub struct TofuVerifier {
    /// Fingerprint conhecido (vindo de settings) — None = primeira vez
    known: Option<String>,
    /// Fingerprint observado durante o handshake (gravado para persistência)
    pub observed: std::sync::Mutex<Option<String>>,
}

impl TofuVerifier {
    pub fn new(known: Option<String>) -> Self {
        Self { known, observed: std::sync::Mutex::new(None) }
    }

    /// Retorna o fingerprint observado após o handshake (para persistir em settings)
    pub fn take_observed(&self) -> Option<String> {
        self.observed.lock().ok()?.take()
    }
}

impl rustls::client::danger::ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let fp = cert_fingerprint(end_entity);

        match &self.known {
            None => {
                // Primeira conexão: aceitar e gravar fingerprint para verificações futuras
                tracing::info!("TOFU: registrando certificado do servidor: {}", &fp[..16]);
                if let Ok(mut obs) = self.observed.lock() {
                    *obs = Some(fp);
                }
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            }
            Some(known_fp) if known_fp == &fp => {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            }
            Some(known_fp) => {
                tracing::error!(
                    "TOFU: certificado do servidor mudou! Esperado: {}… Recebido: {}…",
                    &known_fp[..16], &fp[..16]
                );
                Err(rustls::Error::General(
                    "Certificado TLS do servidor mudou desde a última conexão (possível MITM)".into()
                ))
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

/// Envia uma mensagem por qualquer stream assíncrono que implemente `AsyncWriteExt`
pub async fn send_message<W: AsyncWriteExt + Unpin>(
    stream: &mut W,
    msg: &Message,
) -> Result<(), TransportError> {
    let bytes = msg.encode()?;
    stream.write_all(&bytes).await?;
    debug!("Mensagem enviada: {} bytes", bytes.len());
    Ok(())
}

/// Recebe uma mensagem de qualquer stream assíncrono que implemente `AsyncReadExt`
///
/// Protocolo de framing: magic (4 bytes) + length (4 bytes big-endian) + payload (bincode)
pub async fn recv_message<R: AsyncReadExt + Unpin>(
    stream: &mut R,
) -> Result<Message, TransportError> {
    let mut magic = [0u8; 4];
    match stream.read_exact(&mut magic).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(TransportError::ConnectionClosed);
        }
        Err(e) => return Err(TransportError::Io(e)),
    }

    if magic != MAGIC {
        error!("Magic bytes inválidos: {:?}", magic);
        return Err(TransportError::InvalidMagic);
    }

    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    if len > MAX_PAYLOAD_SIZE {
        return Err(TransportError::PayloadTooLarge(len));
    }

    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;

    let msg = Message::decode(&payload)?;
    debug!("Mensagem recebida: {} bytes", len);
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn test_send_recv_roundtrip() {
        let (mut client, mut server) = duplex(4096);
        let original = Message::Ping;

        send_message(&mut client, &original).await.unwrap();
        let received = recv_message(&mut server).await.unwrap();

        assert!(matches!(received, Message::Ping));
    }

    #[tokio::test]
    async fn test_invalid_magic_rejected() {
        let (mut client, mut server) = duplex(4096);

        client.write_all(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04]).await.unwrap();
        drop(client);

        let result = recv_message(&mut server).await;
        assert!(matches!(result, Err(TransportError::InvalidMagic)));
    }

    #[tokio::test]
    async fn test_payload_too_large_rejected() {
        let (mut client, mut server) = duplex(4096);

        client.write_all(&MAGIC).await.unwrap();
        let huge: u32 = (MAX_PAYLOAD_SIZE + 1) as u32;
        client.write_all(&huge.to_be_bytes()).await.unwrap();
        drop(client);

        let result = recv_message(&mut server).await;
        assert!(matches!(result, Err(TransportError::PayloadTooLarge(_))));
    }

    #[tokio::test]
    async fn test_connection_closed_on_eof() {
        let (client, mut server) = duplex(4096);

        drop(client);

        let result = recv_message(&mut server).await;
        assert!(matches!(result, Err(TransportError::ConnectionClosed)));
    }

    #[test]
    fn test_generate_self_signed_cert() {
        let result = generate_self_signed_cert();
        assert!(result.is_ok(), "Geração de certificado falhou: {:?}", result.err());
        let (certs, _key) = result.unwrap();
        assert!(!certs.is_empty());
    }
}
