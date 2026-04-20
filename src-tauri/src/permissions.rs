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
            "firewall" => cmd_exe(&["/C", "start", "firewall.cpl"]),
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
