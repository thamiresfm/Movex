use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{info, error};

struct Transfer {
    file: File,
    name: String,
    hasher: Sha256,
    received_bytes: u64,
}

pub struct FileReceiver {
    downloads_dir: PathBuf,
    transfers: HashMap<u32, Transfer>,
}

impl FileReceiver {
    pub async fn new() -> Result<Self, String> {
        let dir = dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
            .join("Movex");
        tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
        Ok(Self { downloads_dir: dir, transfers: HashMap::new() })
    }

    pub async fn on_file_start(&mut self, id: u32, name: String, size: u64) -> Result<(), String> {
        // Extrair apenas o componente final do nome para prevenir path traversal
        let safe_name = std::path::Path::new(&name)
            .file_name()
            .ok_or_else(|| format!("Nome de arquivo inválido: '{}'", name))?
            .to_string_lossy()
            .to_string();
        if safe_name.is_empty() || safe_name.starts_with('.') {
            return Err(format!("Nome de arquivo rejeitado: '{}'", safe_name));
        }

        let tmp = self.downloads_dir.join(format!(".movex-{}.tmp", id));
        let file = File::create(&tmp).await.map_err(|e| e.to_string())?;
        info!("Recebendo '{}' ({} bytes, id={}) → {:?}", safe_name, size, id, tmp);
        let name = safe_name;
        self.transfers.insert(
            id,
            Transfer { file, name, hasher: Sha256::new(), received_bytes: 0 },
        );
        Ok(())
    }

    pub async fn on_file_chunk(&mut self, id: u32, _seq: u32, data: Vec<u8>) -> Result<(), String> {
        let t = self
            .transfers
            .get_mut(&id)
            .ok_or_else(|| format!("Transferência {} não encontrada", id))?;
        t.hasher.update(&data);
        t.file.write_all(&data).await.map_err(|e| e.to_string())?;
        t.received_bytes += data.len() as u64;
        Ok(())
    }

    /// Verifica checksum e move para destino final. Retorna (nome, caminho).
    pub async fn on_file_end(
        &mut self,
        id: u32,
        checksum: [u8; 32],
    ) -> Result<(String, PathBuf), String> {
        let mut t = self
            .transfers
            .remove(&id)
            .ok_or_else(|| format!("Transferência {} não encontrada", id))?;
        let computed: [u8; 32] = t.hasher.finalize().into();

        if computed != checksum {
            let tmp = self.downloads_dir.join(format!(".movex-{}.tmp", id));
            let _ = tokio::fs::remove_file(&tmp).await;
            error!("Checksum inválido para '{}'", t.name);
            return Err(format!("Checksum inválido para '{}'", t.name));
        }

        // Garantir que dados estão no disco antes do rename. Sem isto o conteúdo
        // pode estar só no buffer da File handle, e uma leitura imediata após o
        // rename (ex.: o teste `receiver_full_roundtrip`) vê o arquivo vazio.
        t.file.flush().await.map_err(|e| e.to_string())?;
        t.file.sync_all().await.map_err(|e| e.to_string())?;

        let tmp = self.downloads_dir.join(format!(".movex-{}.tmp", id));
        let dest = self.unique_path(&self.downloads_dir.join(&t.name));
        tokio::fs::rename(&tmp, &dest).await.map_err(|e| e.to_string())?;
        info!("Arquivo '{}' recebido → {:?}", t.name, dest);
        Ok((t.name, dest))
    }

    fn unique_path(&self, path: &Path) -> PathBuf {
        if !path.exists() {
            return path.to_path_buf();
        }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        for i in 1..=999 {
            let p = path.with_file_name(format!("{} ({}){}", stem, i, ext));
            if !p.exists() {
                return p;
            }
        }
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn receiver_full_roundtrip() {
        let dir = tempfile::tempdir().unwrap();

        // Substituir downloads_dir pelo tempdir para o teste
        let mut receiver = FileReceiver {
            downloads_dir: dir.path().to_path_buf(),
            transfers: HashMap::new(),
        };

        let content = b"dados de teste do receiver";
        let mut hasher = Sha256::new();
        hasher.update(content);
        let checksum: [u8; 32] = hasher.finalize().into();

        receiver.on_file_start(1, "arquivo.txt".to_string(), content.len() as u64).await.unwrap();
        receiver.on_file_chunk(1, 0, content.to_vec()).await.unwrap();
        let (name, path) = receiver.on_file_end(1, checksum).await.unwrap();

        assert_eq!(name, "arquivo.txt");
        let written = tokio::fs::read(&path).await.unwrap();
        assert_eq!(written, content);
    }

    #[tokio::test]
    async fn receiver_rejects_bad_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let mut receiver = FileReceiver {
            downloads_dir: dir.path().to_path_buf(),
            transfers: HashMap::new(),
        };

        receiver.on_file_start(2, "bad.bin".to_string(), 5).await.unwrap();
        receiver.on_file_chunk(2, 0, b"hello".to_vec()).await.unwrap();

        let wrong_checksum = [0xFFu8; 32];
        let result = receiver.on_file_end(2, wrong_checksum).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Checksum inválido"));
    }

    #[tokio::test]
    async fn receiver_chunk_on_unknown_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut receiver = FileReceiver {
            downloads_dir: dir.path().to_path_buf(),
            transfers: HashMap::new(),
        };
        let result = receiver.on_file_chunk(99, 0, vec![1, 2, 3]).await;
        assert!(result.is_err());
    }
}
