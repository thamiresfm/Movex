use tauri::State;

use crate::core::state::SharedState;

/// Valida que color é um valor CSS seguro (hex ou nome simples)
fn is_safe_css_color(color: &str) -> bool {
    // Aceitar #RGB, #RRGGBB, #RRGGBBAA e nomes CSS simples (max 30 chars, alfanum)
    color.len() <= 30 && color.chars().all(|c| c.is_ascii_alphanumeric() || c == '#')
}

/// Retorna estatísticas da sessão atual
#[tauri::command]
pub async fn get_stats(state: State<'_, SharedState>) -> Result<crate::core::stats::StatsSnapshot, String> {
    Ok(state.stats.snapshot())
}

/// Retorna resolução real do monitor principal
#[tauri::command]
pub fn get_screen_resolution() -> (u32, u32) {
    crate::core::stats::get_primary_screen_size()
}

/// Retorna lista de monitores detectados localmente
#[tauri::command]
pub fn get_monitors() -> Vec<crate::screen::layout::Monitor> {
    crate::screen::layout::detect_monitors().monitors
}

/// Toggle do modo lock (pausar/retomar transição de cursor) — operação atômica
#[tauri::command]
pub async fn toggle_lock(state: State<'_, SharedState>) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    // fetch_xor é atômico — evita race condition entre atalho global e comando IPC
    let was_locked = state.lock_mode.fetch_xor(true, Ordering::AcqRel);
    let now_locked = !was_locked;
    tracing::info!("Modo lock: {}", if now_locked { "ATIVO" } else { "INATIVO" });
    Ok(now_locked)
}

/// Resultado estruturado do diagnóstico de conexão.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnoseResult {
    pub local_ips: Vec<String>,
    pub server_addr: Option<String>,
    pub server_port: u16,
    pub tcp_reachable: Option<bool>,
    pub tcp_error: Option<String>,
    pub mdns_peers: Vec<String>,
    pub firewall_rules_present: bool,
    pub macos_accessibility: bool,
    pub elapsed_ms: u32,
}

/// Diagnóstico de conexão: testa cada camada (local IP, mDNS, TCP) e devolve
/// o que está OK / em falta. Útil quando o utilizador "conecta" mas nada
/// acontece — mostra exatamente onde travou.
#[tauri::command]
pub async fn diagnose_connection(state: State<'_, SharedState>) -> Result<DiagnoseResult, String> {
    let started = std::time::Instant::now();

    let (server_addr_opt, port) = {
        let s = state.settings.lock().await;
        (s.server_addr.clone(), s.port)
    };

    let local_ips = crate::network::local_addrs::local_ipv4_strings();

    // mDNS: 2 segundos de browse — rápido o bastante para a UI não bloquear.
    let mdns_peers = crate::network::discovery::discover_peers(2)
        .await
        .into_iter()
        .map(|p| format!("{} @ {}:{}", p.hostname, p.addr, p.port))
        .collect();

    // TCP probe ao server_addr configurado (se houver).
    // Validar entrada antes de conectar: addr não vazio / sem espaços internos e port != 0.
    let (tcp_reachable, tcp_error) = if let Some(addr) = server_addr_opt.as_ref() {
        let addr_trim = addr.trim();
        if port == 0 || addr_trim.is_empty() || addr_trim.split_whitespace().count() != 1 {
            tracing::warn!("diagnose_connection: server_addr/port inválido — sem probe TCP");
            (Some(false), Some("TCP: entrada inválida (endereço ou porta)".to_string()))
        } else {
            let target = format!("{}:{}", addr_trim, port);
            let connect = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                tokio::net::TcpStream::connect(&target),
            )
            .await;
            match connect {
                Ok(Ok(_)) => (Some(true), None),
                Ok(Err(e)) => (Some(false), Some(format!("TCP: {}", e))),
                Err(_) => (Some(false), Some("TCP: timeout (3s)".to_string())),
            }
        }
    } else {
        (None, Some("server_addr não configurado".to_string()))
    };

    let firewall_rules_present = crate::permissions::windows_firewall_rules_present(port);
    let macos_accessibility = crate::permissions::macos_accessibility_trusted();

    Ok(DiagnoseResult {
        local_ips,
        server_addr: server_addr_opt,
        server_port: port,
        tcp_reachable,
        tcp_error,
        mdns_peers,
        firewall_rules_present,
        macos_accessibility,
        elapsed_ms: started.elapsed().as_millis() as u32,
    })
}

/// Verifica apenas se o macOS concedeu Acessibilidade (sem probe TCP nem mDNS).
/// Usado pelo banner periódico do Dashboard — leve e sem efeitos colaterais.
#[tauri::command]
pub fn check_accessibility() -> bool {
    crate::permissions::macos_accessibility_trusted()
}

/// Ativa/desativa borda luminosa no monitor via evento Tauri (sem eval/unsafe-inline)
#[tauri::command]
pub async fn set_screen_border(
    app: tauri::AppHandle,
    active: bool,
    color: String,
) -> Result<(), String> {
    if !is_safe_css_color(&color) {
        return Err(format!("Cor CSS inválida: '{}'", color));
    }
    // Emitir evento para o frontend — elimina necessidade de eval() e unsafe-inline na CSP
    use tauri::{Emitter, Manager};
    // Emitir apenas para a janela principal (não afeta futuras janelas)
    if let Some(win) = app.get_webview_window("main") {
        win.emit("movex://screen-border", serde_json::json!({ "active": active, "color": color }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
