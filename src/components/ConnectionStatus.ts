/**
 * ConnectionStatus — substitui polling de status por eventos Tauri nativos.
 * O backend emite 'movex://status-changed' ao mudar de estado.
 */
import { invoke } from '@tauri-apps/api/core';
import { onEvent } from '../utils/tauri-events';
import { addLog } from './Logs';

export interface StatusPayload {
  connected: boolean;
  status_text: string;
  peer_hostname?: string;
  latency_ms?: number;
  active_screen: string;
  uptime_secs: number;
}

type StatusHandler = (status: StatusPayload) => void;
const handlers: StatusHandler[] = [];

/** Registra callback que será chamado em mudanças de status */
export function onStatusChange(handler: StatusHandler): void {
  handlers.push(handler);
}

/** Inicializa escuta de eventos de status (substitui polling) */
export async function initStatusListener(): Promise<void> {
  // Escutar evento Tauri nativo
  await onEvent<StatusPayload>('movex://status-changed', (status) => {
    handlers.forEach(h => h(status));
  });

  // Fallback: polling a 3s para compatibilidade (reduzido de chamada constante)
  setInterval(async () => {
    try {
      const status = await invoke<StatusPayload>('get_status');
      handlers.forEach(h => h(status));
    } catch { /* fora do Tauri */ }
  }, 3000);

  // Carregar estado inicial
  try {
    const status = await invoke<StatusPayload>('get_status');
    handlers.forEach(h => h(status));
  } catch { /* fora do Tauri */ }
}

/** Obtém aprovação pendente via polling leve (500ms) */
export function startApprovalPolling(
  onPending: (hostname: string) => void,
  onCleared: () => void,
): void {
  let lastPending: string | null = null;
  setInterval(async () => {
    try {
      const pending = await invoke<string | null>('get_pending_approval');
      if (pending && pending !== lastPending) {
        lastPending = pending;
        onPending(pending);
        addLog(`Solicitação de conexão de: ${pending}`, 'warn');
      } else if (!pending && lastPending) {
        lastPending = null;
        onCleared();
      }
    } catch { /* fora do Tauri */ }
  }, 500);
}
