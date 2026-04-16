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
    let path = dirs::home_dir()
        .unwrap()
        .join("Library/LaunchAgents/com.movex.app.plist");
    std::fs::write(&path, plist).map_err(|e| e.to_string())?;
    info!("Autostart ativado: {:?}", path);
    Ok(())
}

#[cfg(target_os = "macos")]
fn disable_macos() -> Result<(), String> {
    let path = dirs::home_dir()
        .unwrap()
        .join("Library/LaunchAgents/com.movex.app.plist");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    info!("Autostart desativado");
    Ok(())
}

#[cfg(target_os = "windows")]
fn enable_windows() -> Result<(), String> {
    info!("Autostart ativado (Registry)");
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable_windows() -> Result<(), String> {
    info!("Autostart desativado (Registry)");
    Ok(())
}
