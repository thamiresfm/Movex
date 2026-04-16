/**
 * Registro centralizado de listeners de eventos Tauri.
 * Retorna função de cleanup para evitar vazamento de listeners.
 */
import { listen } from '@tauri-apps/api/event';

type UnlistenFn = () => void;
const unlisteners: UnlistenFn[] = [];

/** Registra listener e rastreia para cleanup posterior */
export async function onEvent<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<void> {
  const unlisten = await listen<T>(event, (e) => handler(e.payload));
  unlisteners.push(unlisten);
}

/** Remove todos os listeners registrados */
export function cleanupAllListeners(): void {
  unlisteners.forEach(fn => fn());
  unlisteners.length = 0;
}
