use tauri::State;

use crate::core::state::SharedState;

/// Lê arquivo em chunks e enfileira mensagens; atualiza `sent_bytes` para barra de progresso
pub(crate) async fn send_file_via_channel(
    path: &std::path::Path,
    id: u32,
    size: u64,
    name: String,
    tx: &tokio::sync::mpsc::Sender<crate::network::protocol::Message>,
    transfers: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<u32, crate::transfer::TransferProgress>>>,
) -> Result<(), String> {
    use tokio::io::AsyncReadExt;
    use sha2::{Digest, Sha256};

    tx.send(crate::network::protocol::Message::FileStart { id, name, size })
        .await.map_err(|e| e.to_string())?;

    let mut file = tokio::fs::File::open(path).await.map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut seq = 0u32;
    let mut buf = vec![0u8; crate::network::protocol::FILE_CHUNK_SIZE];
    let mut sent: u64 = 0;

    loop {
        let n = file.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 { break; }
        let chunk = buf[..n].to_vec();
        hasher.update(&chunk);
        sent += n as u64;

        // Atualizar progresso para o frontend
        if let Ok(mut map) = transfers.try_lock() {
            if let Some(p) = map.get_mut(&id) {
                p.sent_bytes = sent;
            }
        }

        tx.send(crate::network::protocol::Message::FileChunk { id, seq, data: chunk })
            .await.map_err(|e| e.to_string())?;
        seq += 1;
    }

    let checksum: [u8; 32] = hasher.finalize().into();
    tx.send(crate::network::protocol::Message::FileEnd { id, checksum })
        .await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Envia um arquivo ao peer conectado
#[tauri::command]
pub async fn send_file_to_peer(
    state: State<'_, SharedState>,
    path: String,
) -> Result<(), String> {
    use std::path::Path;

    let tx = state.message_tx.lock().await.clone()
        .ok_or_else(|| "Não há conexão ativa".to_string())?;

    let transfer_id = state.next_transfer_id().await;

    // Canonicalizar path para evitar path traversal (consistente com drop_file_to_peer)
    let path = match tokio::fs::canonicalize(Path::new(&path)).await {
        Ok(p) => p,
        Err(e) => return Err(format!("Path inválido: {}", e)),
    };
    if path.is_dir() {
        return Err("Não é possível enviar diretórios".to_string());
    }

    let state_clone = state.inner().clone();

    tokio::spawn(async move {
        let file_name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let file_size = match tokio::fs::metadata(&path).await {
            Ok(m) => m.len(),
            Err(e) => {
                tracing::error!("Erro ao ler arquivo '{}': {}", path.display(), e);
                return;
            }
        };

        // Registrar progresso
        {
            let mut transfers = state_clone.transfers.lock().await;
            transfers.insert(transfer_id, crate::transfer::TransferProgress {
                id: transfer_id,
                name: file_name.clone(),
                total_bytes: file_size,
                sent_bytes: 0,
                direction: crate::transfer::TransferDirection::Sending,
            });
        }

        tracing::info!("Enviando '{}' ({} bytes) ao peer...", file_name, file_size);

        match send_file_via_channel(
            &path,
            transfer_id,
            file_size,
            file_name.clone(),
            &tx,
            state_clone.transfers.clone(),
        ).await {
            Ok(_) => {
                tracing::info!("Arquivo '{}' enviado com sucesso", file_name);
                state_clone.transfers.lock().await.remove(&transfer_id);
            }
            Err(e) => {
                tracing::error!("Erro ao enviar '{}': {}", file_name, e);
                state_clone.transfers.lock().await.remove(&transfer_id);
            }
        }
    });

    Ok(())
}

/// Envia arquivo ao peer via drag-and-drop ou caminho direto
#[tauri::command]
pub async fn drop_file_to_peer(
    state: State<'_, SharedState>,
    paths: Vec<String>,
) -> Result<u32, String> {
    let tx = state.message_tx.lock().await.clone()
        .ok_or_else(|| "Não há conexão ativa".to_string())?;

    let mut count = 0u32;
    for path_str in paths {
        let path = std::path::Path::new(&path_str).to_path_buf();

        // Canonicalizar para resolver symlinks e rejeitar path traversal
        let canonical = match tokio::fs::canonicalize(&path).await {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("drop_file_to_peer: path inválido ou inexistente: {:?}", path);
                continue;
            }
        };
        // Rejeitar diretórios e caminhos com ".." (pós-canonicalização)
        if canonical.is_dir() {
            tracing::warn!("drop_file_to_peer: rejeitando diretório: {:?}", canonical);
            continue;
        }
        if !canonical.is_absolute() {
            tracing::warn!("drop_file_to_peer: path não absoluto após canonicalização: {:?}", canonical);
            continue;
        }
        let path = canonical;

        let transfer_id = state.next_transfer_id().await;
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let file_size = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);

        {
            let mut transfers = state.transfers.lock().await;
            transfers.insert(transfer_id, crate::transfer::TransferProgress {
                id: transfer_id,
                name: file_name.clone(),
                total_bytes: file_size,
                sent_bytes: 0,
                direction: crate::transfer::TransferDirection::Sending,
            });
        }

        let tx_clone = tx.clone();
        let state_clone = state.inner().clone();
        tokio::spawn(async move {
            match send_file_via_channel(
                &path,
                transfer_id,
                file_size,
                file_name.clone(),
                &tx_clone,
                state_clone.transfers.clone(),
            ).await {
                Ok(_) => { state_clone.stats.inc_file_sent(); }
                Err(e) => tracing::error!("drop_file_to_peer: {}", e),
            }
            state_clone.transfers.lock().await.remove(&transfer_id);
        });
        count += 1;
    }
    Ok(count)
}

/// Retorna lista de transferências em andamento
#[tauri::command]
pub async fn get_transfers(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::transfer::TransferProgress>, String> {
    let transfers = state.transfers.lock().await;
    Ok(transfers.values().cloned().collect())
}
