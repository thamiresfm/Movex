/**
 * ScreenBorder — borda luminosa no monitor ativo via evento Tauri.
 * Sem eval(), sem unsafe-inline na CSP.
 */
import { onEvent } from '../utils/tauri-events';

let borderEl: HTMLDivElement | null = null;
let styleEl: HTMLStyleElement | null = null;

function removeBorder(): void {
  borderEl?.remove(); borderEl = null;
  styleEl?.remove();  styleEl = null;
}

function showBorder(color: string): void {
  removeBorder();

  // Validar cor no frontend também (defense in depth — backend já valida)
  const safeColor = /^#[0-9a-fA-F]{3,8}$|^[a-zA-Z]+$/.test(color) ? color : '#00d4ff';

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
  // Usar style.setProperty para garantir precedência e evitar qualquer injeção
  borderEl.style.setProperty('position',       'fixed');
  borderEl.style.setProperty('inset',          '0');
  borderEl.style.setProperty('pointer-events', 'none');
  borderEl.style.setProperty('z-index',        '999999');
  borderEl.style.setProperty('border',         `4px solid ${safeColor}`);
  borderEl.style.setProperty('box-shadow',     `inset 0 0 16px ${safeColor}88, 0 0 12px ${safeColor}44`);
  borderEl.style.setProperty('animation',      'movex_pulse 1.5s ease-in-out infinite');
  document.body.appendChild(borderEl);
}

export async function initScreenBorder(): Promise<void> {
  await onEvent<{ active: boolean; color: string }>(
    'movex://screen-border',
    ({ active, color }) => {
      if (active) showBorder(color);
      else        removeBorder();
    },
  );
}
