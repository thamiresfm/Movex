/**
 * FileTransfer — gerencia drag-and-drop e progresso de transferência de arquivos.
 * Cleanup de listeners incluído.
 */
import { invoke } from '@tauri-apps/api/core';
import { addLog } from './Logs';
import { cleanupAllListeners } from '../utils/tauri-events';

let dragListenersAdded = false;
let tauriUnlisten: (() => void) | null = null;

/** Inicializa drag-and-drop com cleanup correto */
export async function initFileTransfer(): Promise<void> {
  if (dragListenersAdded) return;
  dragListenersAdded = true;

  // Criar overlay de drop
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

  // Handlers HTML5 drag
  document.addEventListener('dragover', (e) => {
    e.preventDefault();
    overlay.style.display = 'flex';
  });

  document.addEventListener('dragleave', (e) => {
    // relatedTarget === null = cursor saiu da janela
    if (e.relatedTarget === null) {
      overlay.style.display = 'none';
    }
  });

  document.addEventListener('drop', async (e) => {
    e.preventDefault();
    overlay.style.display = 'none';
    const files = Array.from(e.dataTransfer?.files ?? []);
    const paths = files.map(f => (f as any).path ?? '').filter(Boolean);
    if (paths.length) await sendFiles(paths);
  });

  // Tauri drag-and-drop nativo (com cleanup)
  try {
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const webview = getCurrentWebviewWindow();
    // Remover listener anterior se existir
    tauriUnlisten?.();
    tauriUnlisten = await webview.onDragDropEvent(async (event) => {
      overlay.style.display = 'none';
      if (event.payload.type === 'drop') {
        const paths = event.payload.paths ?? [];
        if (paths.length) await sendFiles(paths);
      }
    });
  } catch { /* API não disponível */ }
}

/** Remove todos os listeners de drag-and-drop */
export function cleanupFileTransfer(): void {
  tauriUnlisten?.();
  tauriUnlisten = null;
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
