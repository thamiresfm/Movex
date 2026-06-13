/**
 * FileTransfer — gerencia drag-and-drop com cleanup completo de todos os listeners.
 */
import { invoke } from '@tauri-apps/api/core';
import { addLog } from './Logs';

let dragListenersAdded = false;
let tauriUnlisten: (() => void) | null = null;

// Rastrear listeners HTML5 para remoção correta no cleanup
type DragHandler = { type: string; fn: EventListenerOrEventListenerObject };
const dragHandlers: DragHandler[] = [];

function addDragListener(type: string, fn: EventListenerOrEventListenerObject) {
  document.addEventListener(type, fn);
  dragHandlers.push({ type, fn });
}

export async function initFileTransfer(): Promise<void> {
  if (dragListenersAdded) return;
  dragListenersAdded = true;

  // Overlay de drop
  const existing = document.getElementById('dropOverlay');
  if (!existing) {
    const overlay = document.createElement('div');
    overlay.id = 'dropOverlay';
    Object.assign(overlay.style, {
      display:        'none',
      position:       'fixed',
      inset:          '0',
      zIndex:         '9998',
      background:     'rgba(0,212,255,.08)',
      border:         '3px dashed var(--cyan)',
      alignItems:     'center',
      justifyContent: 'center',
      pointerEvents:  'none',
    });
    overlay.innerHTML = `
      <div style="text-align:center;color:var(--cyan);">
        <div style="font-size:48px;margin-bottom:12px;">📁</div>
        <div style="font-size:18px;font-weight:700;">Soltar para transferir ao peer</div>
      </div>
    `;
    document.body.appendChild(overlay);
  }

  const getOverlay = () => document.getElementById('dropOverlay');

  const onDragOver = (e: Event) => {
    (e as DragEvent).preventDefault();
    const o = getOverlay();
    if (o) o.style.display = 'flex';
  };

  const onDragLeave = (e: Event) => {
    if ((e as DragEvent).relatedTarget === null) {
      const o = getOverlay();
      if (o) o.style.display = 'none';
    }
  };

  const onDrop = async (e: Event) => {
    (e as DragEvent).preventDefault();
    const o = getOverlay();
    if (o) o.style.display = 'none';
    const files = Array.from((e as DragEvent).dataTransfer?.files ?? []);
    // `File.path` não existe no padrão WebView; o caminho real chega pelo
    // evento nativo `onDragDropEvent` do Tauri. Tratamos como opcional aqui.
    const paths = files.map(f => (f as File & { path?: string }).path ?? '').filter(Boolean);
    if (paths.length) await sendFiles(paths);
  };

  addDragListener('dragover',   onDragOver);
  addDragListener('dragleave',  onDragLeave);
  addDragListener('drop',       onDrop);

  // Tauri drag-and-drop nativo (com cleanup correto)
  try {
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const webview = getCurrentWebviewWindow();
    tauriUnlisten?.(); // remover listener anterior se existir
    tauriUnlisten = await webview.onDragDropEvent(async (event) => {
      const o = getOverlay();
      if (o) o.style.display = 'none';
      if (event.payload.type === 'drop') {
        const paths = event.payload.paths ?? [];
        if (paths.length) await sendFiles(paths);
      }
    });
  } catch { /* API não disponível */ }
}

/** Remove todos os listeners de drag-and-drop (HTML5 + Tauri) */
export function cleanupFileTransfer(): void {
  tauriUnlisten?.();
  tauriUnlisten = null;
  // Remover listeners HTML5 rastreados
  dragHandlers.forEach(({ type, fn }) => document.removeEventListener(type, fn));
  dragHandlers.length = 0;
  dragListenersAdded = false;
}

async function sendFiles(paths: string[]): Promise<void> {
  addLog(`Transferindo ${paths.length} arquivo(s)...`, 'info');
  try {
    const count = await invoke<number>('drop_file_to_peer', { paths });
    if (count > 0) addLog(`${count} arquivo(s) em transferência.`, 'sec');
    else addLog('Nenhum arquivo válido para transferir.', 'warn');
  } catch (e) {
    addLog(`Erro na transferência: ${e}`, 'warn');
  }
}
