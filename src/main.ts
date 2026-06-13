import "./styles/global.css";
import { invoke } from "@tauri-apps/api/core";

/** Subconjunto mínimo das settings que o arranque precisa de conhecer. */
interface AppSettings {
  theme?: string;
  setup_complete?: boolean;
}

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
    const settings = await invoke<AppSettings>("get_settings");

    // Aplicar tema antes de renderizar qualquer componente
    applyTheme(settings.theme ?? "dark");

    if (!settings.setup_complete) {
      const { renderSetup } = await import("./components/Setup");
      await renderSetup();
    } else {
      const { renderDashboard } = await import("./components/Dashboard");
      await renderDashboard();
    }
  } catch (err) {
    // get_settings falhou de facto (IPC indisponível ou erro no backend).
    // NÃO assumir que é primeira execução: cair no Dashboard mascararia o erro
    // como setup concluído. Mostramos antes o Setup, que é o estado seguro
    // (re)inicial, registando o detalhe para diagnóstico.
    console.error("Falha ao carregar settings no arranque:", err);
    applyTheme("dark");
    const { renderSetup } = await import("./components/Setup");
    await renderSetup();
  }
}

export function applyTheme(theme: string) {
  document.documentElement.setAttribute("data-theme", theme === "light" ? "light" : "dark");
}

init();
