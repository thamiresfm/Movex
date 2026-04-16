use tracing::info;

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

#[cfg(target_os = "macos")]
fn enable_macos() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let home = dirs::home_dir().ok_or_else(|| "Diretório home não encontrado".to_string())?;
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
        exe.display()
    );
    let agents_dir = home.join("Library/LaunchAgents");
    std::fs::create_dir_all(&agents_dir).map_err(|e| e.to_string())?;
    let path = agents_dir.join("com.movex.app.plist");
    std::fs::write(&path, plist).map_err(|e| e.to_string())?;
    info!("Autostart ativado (macOS LaunchAgent): {:?}", path);
    Ok(())
}

#[cfg(target_os = "macos")]
fn disable_macos() -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "Diretório home não encontrado".to_string())?;
    let path = home.join("Library/LaunchAgents/com.movex.app.plist");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    info!("Autostart desativado (macOS)");
    Ok(())
}

#[cfg(target_os = "windows")]
fn enable_windows() -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy();

    // Usar reg.exe para escrever no Registry (disponível em todo Windows)
    let output = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v", "Movex",
            "/t", "REG_SZ",
            "/d", &exe_str,
            "/f",
        ])
        .output()
        .map_err(|e| format!("Erro ao executar reg.exe: {}", e))?;

    if output.status.success() {
        info!("Autostart ativado (Windows Registry)");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Falha ao escrever no Registry: {}", stderr))
    }
}

#[cfg(target_os = "windows")]
fn disable_windows() -> Result<(), String> {
    let output = std::process::Command::new("reg")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v", "Movex",
            "/f",
        ])
        .output()
        .map_err(|e| format!("Erro ao executar reg.exe: {}", e))?;

    info!("Autostart desativado (Windows Registry)");
    Ok(())
}
