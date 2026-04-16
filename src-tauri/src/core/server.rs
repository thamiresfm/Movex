use crate::core::SharedState;
use tracing::info;

/// Inicia o servidor Movex — implementado na Task 6
pub async fn start(_state: SharedState) -> Result<(), String> {
    info!("Servidor Movex (stub — Task 6)");
    Ok(())
}
