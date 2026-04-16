import "./styles/global.css";
import { invoke } from "@tauri-apps/api/core";

async function init() {
  try {
    const settings = await invoke<any>("get_settings");
    if (!settings.setup_complete) {
      const { renderSetup } = await import("./components/Setup");
      await renderSetup();
    } else {
      const { renderDashboard } = await import("./components/Dashboard");
      await renderDashboard();
    }
  } catch {
    // Fora do contexto Tauri (browser direto) — mostrar dashboard
    const { renderDashboard } = await import("./components/Dashboard");
    await renderDashboard();
  }
}

init();
