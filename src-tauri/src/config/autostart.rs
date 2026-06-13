pub fn enable() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return enable_macos();
    #[cfg(target_os = "windows")]
    return enable_windows();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err("Plataforma não suportada para autostart".to_string())
}

pub fn disable() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return disable_macos();
    #[cfg(target_os = "windows")]
    return disable_windows();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err("Plataforma não suportada para autostart".to_string())
}

/// Escapa os caracteres especiais de XML para interpolar com segurança em
/// `<string>...</string>` do plist (ex.: caminhos com & < >).
#[cfg(target_os = "macos")]
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(target_os = "macos")]
fn enable_macos() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let home = dirs::home_dir().ok_or_else(|| "Diretório home não encontrado".to_string())?;
    let exe_escaped = escape_xml(&exe.to_string_lossy());
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.movex.app</string>
    <key>ProgramArguments</key>
    <array><string>{}</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><false/>
</dict>
</plist>"#,
        exe_escaped
    );
    let agents_dir = home.join("Library/LaunchAgents");
    std::fs::create_dir_all(&agents_dir).map_err(|e| e.to_string())?;
    let path = agents_dir.join("com.movex.app.plist");
    std::fs::write(&path, plist).map_err(|e| e.to_string())?;
    tracing::info!("Autostart ativado (macOS LaunchAgent): {:?}", path);
    Ok(())
}

#[cfg(target_os = "macos")]
fn disable_macos() -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "Diretório home não encontrado".to_string())?;
    let path = home.join("Library/LaunchAgents/com.movex.app.plist");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    tracing::info!("Autostart desativado (macOS)");
    Ok(())
}

#[cfg(target_os = "windows")]
fn enable_windows() -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let raw = exe.to_string_lossy().to_string();
    // Se o caminho contiver espaços, deve ir entre aspas para o Windows
    // interpretar corretamente o executável na chave Run.
    let exe_str = if raw.contains(' ') && !raw.starts_with('"') {
        format!("\"{}\"", raw)
    } else {
        raw
    };

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| format!("Falha ao abrir chave de autostart: {}", e))?;

    run_key
        .set_value("Movex", &exe_str)
        .map_err(|e| format!("Falha ao definir valor de autostart: {}", e))?;

    tracing::info!(
        "Autostart ativado (Windows Registry via winreg): {}",
        exe_str
    );
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable_windows() -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| format!("Falha ao abrir chave de autostart: {}", e))?;

    // `delete_value` não retorna erro se o valor não existir
    run_key.delete_value("Movex").unwrap_or_default();
    tracing::info!("Autostart desativado (Windows Registry)");
    Ok(())
}
