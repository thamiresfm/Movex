/// Verifica se há atualização disponível
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    match app.updater() {
        Ok(updater) => {
            match updater.check().await {
                // Há atualização disponível.
                Ok(Some(update)) => Ok(Some(update.version.to_string())),
                // Está atualizado — único caso que devolve Ok(None).
                Ok(None) => Ok(None),
                // Erro real de rede/servidor — não mascarar como "sem update".
                Err(e) => {
                    tracing::warn!("Erro ao verificar atualizações: {}", e);
                    Err("Falha ao verificar atualizações".to_string())
                }
            }
        }
        Err(e) => {
            tracing::warn!("Updater não disponível: {}", e);
            Err("Atualizador indisponível".to_string())
        }
    }
}

/// Instala a atualização disponível — retorna Err se não há update
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => {
            let version = update.version.to_string();
            update.download_and_install(|downloaded, total| {
                if let Some(t) = total {
                    tracing::info!("Update: {}/{} bytes ({:.0}%)", downloaded, t,
                        downloaded as f64 / t as f64 * 100.0);
                }
            }, || {
                tracing::info!("Update instalado — reiniciando...");
            }).await.map_err(|e| e.to_string())?;
            Ok(format!("v{} instalada com sucesso", version))
        }
        None => Err("Nenhuma atualização disponível".to_string()),
    }
}
