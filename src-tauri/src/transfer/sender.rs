use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tracing::info;

use crate::network::protocol::{Message, FILE_CHUNK_SIZE};

/// Envia um arquivo para o peer em chunks de 64KB com checksum SHA-256
pub async fn send_file<W>(
    stream: &mut W,
    path: &Path,
    transfer_id: u32,
) -> Result<[u8; 32], String>
where
    W: tokio::io::AsyncWriteExt + Unpin,
{
    let mut file = File::open(path)
        .await
        .map_err(|e| format!("Erro ao abrir '{}': {}", path.display(), e))?;

    let size = file.metadata().await.map_err(|e| e.to_string())?.len();
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    info!("Enviando '{}' ({} bytes, id={})", name, size, transfer_id);

    let start = Message::FileStart { id: transfer_id, name: name.clone(), size };
    let bytes = start.encode().map_err(|e| e.to_string())?;
    tokio::io::AsyncWriteExt::write_all(stream, &bytes).await.map_err(|e| e.to_string())?;

    let mut hasher = Sha256::new();
    let mut seq = 0u32;
    let mut buf = vec![0u8; FILE_CHUNK_SIZE];

    loop {
        let n = file.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        hasher.update(chunk);

        let msg = Message::FileChunk { id: transfer_id, seq, data: chunk.to_vec() };
        let bytes = msg.encode().map_err(|e| e.to_string())?;
        tokio::io::AsyncWriteExt::write_all(stream, &bytes).await.map_err(|e| e.to_string())?;
        seq += 1;
    }

    let checksum: [u8; 32] = hasher.finalize().into();
    let end = Message::FileEnd { id: transfer_id, checksum };
    let bytes = end.encode().map_err(|e| e.to_string())?;
    tokio::io::AsyncWriteExt::write_all(stream, &bytes).await.map_err(|e| e.to_string())?;

    info!("'{}' enviado: {} chunks, SHA-256: {}", name, seq, hex::encode(checksum));
    Ok(checksum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn send_file_produces_valid_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        let content = b"hello movex transfer";
        tokio::fs::write(&path, content).await.unwrap();

        let mut buf: Vec<u8> = Vec::new();
        let checksum = send_file(&mut buf, &path, 1).await.unwrap();

        let mut hasher = Sha256::new();
        hasher.update(content);
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(checksum, expected);
    }

    #[tokio::test]
    async fn send_file_error_on_missing_file() {
        let mut buf: Vec<u8> = Vec::new();
        let result = send_file(&mut buf, Path::new("/nao/existe.txt"), 99).await;
        assert!(result.is_err());
    }
}
