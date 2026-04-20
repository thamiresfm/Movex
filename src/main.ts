import "./styles/global.css";
import { invoke } from "@tauri-apps/api/core";

/** Pedido de permissão de notificações ao arranque (o SO mostra o diálogo nativo). */
async function requestNotificationPermissionOnStartup(): Promise<void> {
  try {
    const { isPermissionGranted, requestPermission } = await import("@tauri-apps/plugin-notification");
    const granted = await isPermissionGranted();
    if (!granted) {
      await requestPermission();
    }
  } catch {
    // Ambiente web / plugin indisponível — ignorar
  }
}

async function init() {
  void requestNotificationPermissionOnStartup();
  try {
    const settings = await invoke<any>("get_settings");

    // Aplicar tema antes de renderizar qualquer componente
    applyTheme(settings.theme ?? "dark");

    if (!settings.setup_complete) {
      const { renderSetup } = await import("./components/Setup");
      await renderSetup();
    } else {
      const { renderDashboard } = await import("./components/Dashboard");
      await renderDashboard();
    }
  } catch {
    applyTheme("dark");
    const { renderDashboard } = await import("./components/Dashboard");
    await renderDashboard();
  }
}

export function applyTheme(theme: string) {
  document.documentElement.setAttribute("data-theme", theme === "light" ? "light" : "dark");
}

init();
