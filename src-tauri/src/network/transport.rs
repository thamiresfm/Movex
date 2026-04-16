use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tracing::{debug, error};

use crate::network::protocol::{Message, MAGIC};

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

/// Gera certificado TLS autoassinado para esta instância
///
/// Usa a API do rcgen 0.13.x: `CertifiedKey { cert, key_pair }`
/// - `cert.der()` retorna `&CertificateDer<'static>`
/// - `key_pair.serialize_der()` retorna `Vec<u8>` no formato PKCS#8
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

/// Cria `TlsConnector` (modo cliente) que aceita certificados autoassinados de LAN
pub fn create_tls_connector() -> TlsConnector {
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

/// Verifier que aceita qualquer certificado — adequado para self-signed em LAN privada
#[derive(Debug)]
struct AcceptAnyCert;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
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
    // Ler magic bytes (4 bytes)
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

    // Ler comprimento do payload (4 bytes big-endian)
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    if len > MAX_PAYLOAD_SIZE {
        return Err(TransportError::PayloadTooLarge(len));
    }

    // Ler payload
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

        // Escrever bytes com magic inválido
        client.write_all(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04]).await.unwrap();
        drop(client);

        let result = recv_message(&mut server).await;
        assert!(matches!(result, Err(TransportError::InvalidMagic)));
    }

    #[tokio::test]
    async fn test_payload_too_large_rejected() {
        let (mut client, mut server) = duplex(4096);

        // Magic válido + tamanho > MAX_PAYLOAD_SIZE
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

        // Fechar sem escrever nada
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
