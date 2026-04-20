//! Pedidos de permissão ao sistema e atalhos para painéis de Privacidade / Definições.

/// `macos` | `windows` | `linux` — para a UI mostrar atalhos corretos.
#[tauri::command]
pub fn get_platform_kind() -> String {
    #[cfg(target_os = "macos")]
    {
        "macos".into()
    }
    #[cfg(target_os = "windows")]
    {
        "windows".into()
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        "linux".into()
    }
}

/// No macOS, mostra o diálogo nativo que pede ao utilizador para autorizar Acessibilidade
/// (quando ainda não concedida). É a forma suportada pela Apple; não é possível conceder
/// automaticamente sem interação.
#[cfg(target_os = "macos")]
pub fn request_macos_accessibility_prompt() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    }

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let val = CFBoolean::true_value();
    let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), val.as_CFType())]);
    unsafe {
        let _already_trusted = AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef());
        tracing::info!(
            "macOS Acessibilidade: trusted={} (prompt nativo se ainda não autorizado)",
            _already_trusted != 0
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_macos_accessibility_prompt() {}

/// Abre o painel do sistema correspondente (Privacidade, Notificações, Firewall, etc.).
#[tauri::command]
pub fn open_system_settings(panel: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match panel.as_str() {
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "input_monitoring" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            "screen_capture" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "notifications" => "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
            "privacy" => "x-apple.systempreferences:com.apple.preference.security?Privacy",
            _ => return Err(format!("Painel macOS desconhecido: {}", panel)),
        };
        open_url_macos(url)
    }
    #[cfg(target_os = "windows")]
    {
        match panel.as_str() {
            "notifications" => cmd_exe(&["/C", "start", "ms-settings:notifications"]),
            // Painel clássico
            "firewall" => cmd_exe(&["/C", "start", "firewall.cpl"]),
            // Firewall e segurança (regras / exceções)
            "firewall_advanced" => {
                cmd_exe(&["/C", "start", "ms-settings:windowsdefender-firewall"])
            }
            // Proxy do sistema (útil com proxy corporativo)
            "proxy" => cmd_exe(&["/C", "start", "ms-settings:network-proxy"]),
            "privacy" => cmd_exe(&["/C", "start", "ms-settings:privacy"]),
            _ => Err(format!("Painel Windows desconhecido: {}", panel)),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = panel;
        Err("Abrir definições não suportado neste sistema.".into())
    }
}

#[cfg(target_os = "macos")]
fn open_url_macos(url: &str) -> Result<(), String> {
    let s = std::process::Command::new("open")
        .arg(url)
        .status()
        .map_err(|e| e.to_string())?;
    if s.success() {
        Ok(())
    } else {
        Err("comando «open» terminou com erro".into())
    }
}

#[cfg(target_os = "windows")]
fn cmd_exe(args: &[&str]) -> Result<(), String> {
    let s = std::process::Command::new("cmd.exe")
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if s.success() {
        Ok(())
    } else {
        Err("comando Windows terminou com erro".into())
    }
}

/// Adiciona regras no «Firewall do Windows» para o executável do Movex e para a porta TCP/UDP
/// usada pelo servidor. **Requer confirmação UAC (Administrador)** — o Windows não permite
/// alterar o firewall sem elevação.
///
/// Isto não altera políticas de proxy corporativo; use `open_system_settings("proxy")` para
/// o painel de proxy do sistema.
#[tauri::command]
pub fn windows_apply_firewall_rules_for_movex(port: u16) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        windows_apply_firewall_rules_impl(port)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = port;
        Err("Disponível apenas no Windows.".into())
    }
}

#[cfg(target_os = "windows")]
fn windows_apply_firewall_rules_impl(port: u16) -> Result<String, String> {
    use std::io::Write;
    use std::process::Command;

    if port == 0 {
        return Err("Porta inválida.".into());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_ps = exe.to_string_lossy().replace('\'', "''");

    let script = format!(
        r#"$ErrorActionPreference = 'Continue'
$port = {port}
$exe = '{exe_ps}'
netsh advfirewall firewall delete rule name="Movex TCP $port" 2>$null
netsh advfirewall firewall delete rule name="Movex UDP $port" 2>$null
netsh advfirewall firewall delete rule name="Movex App In" 2>$null
netsh advfirewall firewall delete rule name="Movex App Out" 2>$null
netsh advfirewall firewall add rule name="Movex TCP $port" dir=in action=allow protocol=TCP localport=$port profile=any
netsh advfirewall firewall add rule name="Movex UDP $port" dir=in action=allow protocol=UDP localport=$port profile=any
netsh advfirewall firewall add rule name="Movex App In" dir=in action=allow program="$exe" profile=any
netsh advfirewall firewall add rule name="Movex App Out" dir=out action=allow program="$exe" profile=any
Write-Host 'Regras Movex aplicadas (TCP/UDP porta' $port 'e programa).'
"#,
        port = port,
        exe_ps = exe_ps
    );

    let ps1 = std::env::temp_dir().join("movex-firewall-rules.ps1");
    let mut f = std::fs::File::create(&ps1).map_err(|e| e.to_string())?;
    f.write_all(script.as_bytes()).map_err(|e| e.to_string())?;
    drop(f);

    let path_arg = format!(
        "'{}'",
        ps1.to_string_lossy().replace('\'', "''")
    );
    let inner = format!(
        "Start-Process -FilePath powershell.exe -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',{}",
        path_arg
    );

    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &inner])
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(
            "Não foi possível pedir elevação (UAC). Execute o Movex como administrador ou confirme o prompt."
                .into(),
        );
    }

    Ok(
        "Se pedido, aceite o Controlo de Conta de Utilizador (UAC) para aplicar as regras de firewall."
            .into(),
    )
}
