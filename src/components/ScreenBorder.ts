/**
 * ScreenBorder — gerencia a borda luminosa física no monitor ativo.
 * Escuta o evento Tauri 'movex://screen-border' (sem eval/unsafe-inline).
 */
import { onEvent } from '../utils/tauri-events';

let borderEl: HTMLDivElement | null = null;
let styleEl: HTMLStyleElement | null = null;

function removeBorder(): void {
  borderEl?.remove();
  borderEl = null;
  styleEl?.remove();
  styleEl = null;
}

function showBorder(color: string): void {
  removeBorder();

  styleEl = document.createElement('style');
  styleEl.textContent = `
    @keyframes movex_pulse {
      0%, 100% { opacity: 1; }
      50%       { opacity: 0.55; }
    }
  `;
  document.head.appendChild(styleEl);

  borderEl = document.createElement('div');
  borderEl.id = '__movex_border__';
  Object.assign(borderEl.style, {
    position:      'fixed',
    inset:         '0',
    pointerEvents: 'none',
    zIndex:        '999999',
    border:        `4px solid ${CSS.escape(color)}`,
    boxShadow:     `inset 0 0 16px ${color}88, 0 0 12px ${color}44`,
    animation:     'movex_pulse 1.5s ease-in-out infinite',
  });
  document.body.appendChild(borderEl);
}

/** Inicia escuta do evento de borda luminosa */
export async function initScreenBorder(): Promise<void> {
  await onEvent<{ active: boolean; color: string }>(
    'movex://screen-border',
    ({ active, color }) => {
      if (active) {
        showBorder(color);
      } else {
        removeBorder();
      }
    },
  );
}
