/**
 * ConnectionStatus — escuta eventos Tauri + polling de fallback.
 * Retorna funções de cleanup para evitar vazamentos.
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

export function onStatusChange(handler: StatusHandler): void {
  handlers.push(handler);
}

/** Remove todos os handlers — chamar antes de re-renderizar o Dashboard */
export function cleanupStatusHandlers(): void {
  handlers.length = 0;
}

/**
 * Inicializa escuta de eventos de status.
 * Usa evento Tauri quando disponível; cai para polling como fallback.
 * Retorna função de cleanup.
 */
export async function initStatusListener(): Promise<() => void> {
  let pollId: ReturnType<typeof setInterval> | null = null;

  // Polling constante: garante UI alinhada ao backend mesmo se o evento Tauri falhar
  pollId = setInterval(async () => {
    try {
      const status = await invoke<StatusPayload>('get_status');
      handlers.forEach(h => h(status));
    } catch { /* fora do Tauri */ }
  }, 2000);

  // Evento em tempo real quando o Rust emite (complementa o polling)
  await onEvent<StatusPayload>('movex://status-changed', (status) => {
    handlers.forEach(h => h(status));
  });

  // Carregar estado inicial
  try {
    const status = await invoke<StatusPayload>('get_status');
    handlers.forEach(h => h(status));
  } catch { /* fora do Tauri */ }

  // Retornar cleanup
  return () => {
    if (pollId !== null) clearInterval(pollId);
  };
}

/**
 * Polling de aprovação de conexão pendente.
 * Retorna função de cleanup para o setInterval.
 */
export function startApprovalPolling(
  onPending: (hostname: string) => void,
  onCleared: () => void,
): () => void {
  let lastPending: string | null = null;
  const id = setInterval(async () => {
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
  return () => clearInterval(id);
}
