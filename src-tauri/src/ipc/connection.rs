use tauri::State;

use crate::config::{Role, Settings};
use crate::core::state::{ConnectionStatus, SharedState};

#[tauri::command]
pub async fn start_connection(state: State<'_, SharedState>) -> Result<(), String> {
    // Cancelar conexão anterior se existir
    state.cancel_connection().await;

    // Windows: verificar se as regras de firewall do Movex existem; se não, pedir UAC.
    // Em vez de confiar num flag persistido (que mantinha-se `true` mesmo depois do
    // utilizador remover as regras manualmente / reinstalar Windows / etc.),
    // consultamos o `netsh advfirewall firewall show rule` directamente. Isso é uma
    // leitura — não exige elevação — e dá-nos a verdade do sistema em cada Connect.
    #[cfg(target_os = "windows")]
    {
        let (port, role_hint) = {
            let s = state.settings.lock().await;
            (s.port, s.role.clone())
        };
        let need_fw = !crate::permissions::windows_firewall_rules_present_async(port).await;
        if need_fw {
            tracing::info!("Windows: regras Movex em falta — a pedir permissão (UAC).");
            if let Some(app) = state.app_handle.lock().await.as_ref() {
                let body = match role_hint {
                    Role::Server => "Vai abrir o pedido do Windows (UAC). Aceite «Sim» para permitir o Movex na porta da rede (servidor).",
                    Role::Client => "Vai abrir o pedido do Windows (UAC). Aceite «Sim» para permitir o Movex no Firewall (rede local).",
                };
                crate::core::notifications::notify(app, "Movex — Permissão do Windows", body);
            }
            match crate::permissions::windows_apply_firewall_rules_impl(port) {
                Ok(msg) => tracing::info!("{msg}"),
                Err(e) => tracing::warn!("Firewall (pedido automático): {e}"),
            }
            {
                let mut s = state.settings.lock().await;
                s.windows_firewall_prompt_done = true;
                if let Err(e) = s.save() {
                    tracing::warn!("Erro ao gravar windows_firewall_prompt_done: {e}");
                }
            }
        } else {
            tracing::debug!("Windows: regras Movex já presentes — sem pedido de UAC.");
        }
    }

    let role = { state.settings.lock().await.role.clone() };
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();

    match role {
        Role::Server => {
            tokio::spawn(async move {
                if let Err(e) = crate::core::server::start(state_clone, cancel).await {
                    tracing::error!("Servidor encerrou com erro: {}", e);
                }
            });
        }
        Role::Client => {
            tokio::spawn(async move {
                crate::core::client::connect(state_clone, cancel).await;
            });
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, SharedState>) -> Result<(), String> {
    // Enviar Disconnect ao peer se conectado
    if let Some(tx) = state.message_tx.lock().await.as_ref() {
        let _ = tx.try_send(crate::network::protocol::Message::Disconnect {
            reason: "usuário desconectou".into(),
        });
    }
    // Cancelar task de conexão
    state.cancel_connection().await;
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }
    {
        let mut t = state.session_started_at.lock().await;
        *t = None;
    }
    tracing::info!("Desconectado pelo usuário");
    crate::ipc::emit_status_to_main(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn reset_settings(state: State<'_, SharedState>) -> Result<(), String> {
    state.cancel_connection().await;
    let new_settings = Settings::default();
    new_settings.save()?;
    {
        let mut s = state.settings.lock().await;
        *s = new_settings;
    }
    let mut status = state.connection_status.lock().await;
    *status = ConnectionStatus::Disconnected;
    tracing::info!("Configurações resetadas");
    Ok(())
}

/// Troca papel E reinicia conexão automaticamente
#[tauri::command]
pub async fn switch_role(state: State<'_, SharedState>, role: String) -> Result<(), String> {
    // Desconectar sessão ativa
    if let Some(tx) = state.message_tx.lock().await.as_ref() {
        let _ = tx.try_send(crate::network::protocol::Message::Disconnect {
            reason: "troca de papel".into(),
        });
    }
    state.cancel_connection().await;
    {
        let mut status = state.connection_status.lock().await;
        *status = ConnectionStatus::Disconnected;
    }

    // Atualizar papel
    {
        let mut s = state.settings.lock().await;
        s.role = if role == "server" { Role::Server } else { Role::Client };
        s.save()?;
    }

    // Relançar com o novo papel
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();
    let role_enum = if role == "server" { Role::Server } else { Role::Client };
    match role_enum {
        Role::Server => {
            tokio::spawn(async move {
                if let Err(e) = crate::core::server::start(state_clone, cancel).await {
                    tracing::error!("Servidor: {}", e);
                }
            });
        }
        Role::Client => {
            tokio::spawn(async move {
                crate::core::client::connect(state_clone, cancel).await;
            });
        }
    }
    tracing::info!("Papel trocado para: {}", role);
    Ok(())
}

/// Conecta diretamente a um peer descoberto via mDNS.
/// O endereço é armazenado apenas na sessão — não sobrescreve settings persistidas.
#[tauri::command]
pub async fn connect_to_peer(
    state: State<'_, SharedState>,
    addr: String,
    port: u16,
) -> Result<(), String> {
    // Guardar endereço de sessão sem persistir
    { *state.session_server_addr.lock().await = Some((addr.clone(), port)); }

    state.cancel_connection().await;
    let cancel = state.new_cancel_token().await;
    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        crate::core::client::connect_to_addr(state_clone, addr, port, cancel).await;
    });
    Ok(())
}

/// Limpa o fingerprint TLS guardado (TOFU). Use no **cliente** quando o servidor foi reinstalado,
/// mudou de PC ou passou a apresentar outro certificado — sem apagar o resto das configurações.
#[tauri::command]
pub async fn clear_server_cert_trust(state: State<'_, SharedState>) -> Result<(), String> {
    {
        let mut s = state.settings.lock().await;
        s.server_cert_fingerprint = None;
        s.save()?;
    }
    tracing::info!("Confiança TLS do servidor (TOFU) limpa");
    Ok(())
}

/// Retorna o hostname do cliente aguardando aprovação — sempre `None` (aprovação automática).
#[tauri::command]
pub async fn get_pending_approval(_state: State<'_, SharedState>) -> Result<Option<String>, String> {
    Ok(None)
}

/// Mantido para compatibilidade com a UI antiga; a ligação já não exige aprovação manual.
#[tauri::command]
pub async fn approve_connection(_state: State<'_, SharedState>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn reject_connection(_state: State<'_, SharedState>) -> Result<(), String> {
    Ok(())
}
