/**
 * ConnectionStatus — escuta eventos Tauri + polling de fallback.
 * Retorna funções de cleanup para evitar vazamentos.
 */
import { invoke } from '@tauri-apps/api/core';
import { onEvent } from '../utils/tauri-events';
import { addLog } from './Logs';

export interface StatusPayload {
  connected: boolean;
  /** Servidor à escuta, a ligar, ligado ou a reconectar — UI mostra «Desconectar». */
  in_session: boolean;
  status_text: string;
  peer_hostname?: string;
  peer_addr?: string;
  latency_ms?: number;
  active_screen: string;
  uptime_secs: number;
}

/** Tauri/serde pode enviar snake_case ou camelCase — normaliza antes da UI. */
export function normalizeStatusPayload(raw: unknown): StatusPayload {
  if (!raw || typeof raw !== "object") {
    return {
      connected: false,
      in_session: false,
      status_text: "",
      active_screen: "Local",
      uptime_secs: 0,
    };
  }
  const r = raw as Record<string, unknown>;
  const num = (v: unknown): number | undefined =>
    typeof v === "number" && !Number.isNaN(v) ? v : undefined;
  const statusText = String(r.status_text ?? r.statusText ?? "");
  const connected = Boolean(r.connected);
  const explicitSession = r.in_session ?? r.inSession;
  const in_session =
    typeof explicitSession === "boolean"
      ? explicitSession
      : connected || /aguardando|conectando|reconectando/i.test(statusText);
  return {
    connected,
    in_session,
    status_text: statusText,
    peer_hostname: (r.peer_hostname ?? r.peerHostname) as string | undefined,
    peer_addr: (r.peer_addr ?? r.peerAddr) as string | undefined,
    latency_ms: num(r.latency_ms ?? r.latencyMs),
    active_screen: String(r.active_screen ?? r.activeScreen ?? "Local"),
    uptime_secs: Number(r.uptime_secs ?? r.uptimeSecs ?? 0),
  };
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
      const raw = await invoke<unknown>("get_status");
      const status = normalizeStatusPayload(raw);
      handlers.forEach((h) => h(status));
    } catch {
      /* fora do Tauri */
    }
  }, 900);

  // Evento em tempo real quando o Rust emite (complementa o polling)
  await onEvent<unknown>("movex://status-changed", (raw) => {
    const status = normalizeStatusPayload(raw);
    handlers.forEach((h) => h(status));
  });

  // Falhas/sucessos de ligação: aparecem no painel de logs mesmo com notificações do SO desligadas (especialmente macOS).
  await onEvent<{ level: string; message: string }>("movex://connection-log", (p) => {
    const lv =
      p.level === "warn" || p.level === "warning" ? "warn" : p.level === "sec" ? "sec" : "info";
    addLog(p.message, lv);
  });

  // Carregar estado inicial (handlers já devem estar registados — ver Dashboard)
  try {
    const raw = await invoke<unknown>("get_status");
    const status = normalizeStatusPayload(raw);
    handlers.forEach((h) => h(status));
  } catch {
    /* fora do Tauri */
  }

  // Retornar cleanup
  return () => {
    if (pollId !== null) clearInterval(pollId);
  };
}

