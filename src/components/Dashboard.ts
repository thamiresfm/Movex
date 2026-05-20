declare const __APP_VERSION__: string;

import { invoke, isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { addLog, clearLogs } from "./Logs";
import { initScreenBorder } from "./ScreenBorder";
import { initFileTransfer, cleanupFileTransfer } from "./FileTransfer";
import {
  initStatusListener,
  onStatusChange,
  cleanupStatusHandlers,
  normalizeStatusPayload,
  type StatusPayload,
} from "./ConnectionStatus";
import { cleanupAllListeners } from "../utils/tauri-events";

function esc(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/** Preenchido ao carregar o painel e ao “Atualizar” na rede — usado na grade Conectar. */
let movexLocalIpv4Cache = '';

function formatLocalNetworkLine(port: number, ipv4Csv: string): string {
  if (ipv4Csv) {
    return `${ipv4Csv} · porta ${port} · esta máquina`;
  }
  return `Porta ${port} · esta máquina (IPv4 não detectado)`;
}

/** Sessão ativa no backend ou inferida do texto (fallback se o campo in_session falhar no IPC). */
function sessionLooksActive(status: StatusPayload): boolean {
  if (status.connected) return true;
  if (status.in_session) return true;
  const t = (status.status_text ?? '').toLowerCase();
  return /aguardando|conectando|reconectando/.test(t);
}

function applyPanelConnectionButtons(status: StatusPayload): void {
  const on = sessionLooksActive(status);
  const btnConnect = document.getElementById('btnConnect') as HTMLButtonElement | null;
  const btnDisconnect = document.getElementById('btnDisconnect') as HTMLButtonElement | null;
  if (btnConnect && btnDisconnect) {
    btnConnect.style.display = on ? 'none' : 'inline-flex';
    btnDisconnect.style.display = on ? 'inline-flex' : 'none';
  }
}

function updateMatrixHint(
  settings: { role?: string } | null,
  status: StatusPayload,
  lastPeer?: string | null,
): void {
  const hintMatriz = document.getElementById('matrixConnHint');
  if (!hintMatriz) return;

  if (status.connected) {
    hintMatriz.style.display = 'block';
    const role = (settings?.role ?? 'server').toLowerCase();
    if (role === 'server') {
      hintMatriz.innerHTML = [
        '<strong style="color:var(--cyan);">Servidor conectado</strong> — empurre o rato até à borda do ecrã na direção do outro PC (nas setas da Matriz).',
        'Se o cursor não passar: no macOS verifique <strong>Configurações → Privacidade → Acessibilidade</strong> (Movex deve estar ✓ ativado).',
        'No Windows: aceite o UAC do Firewall na 1.ª ligação ou use o botão «Aplicar regras no firewall» em Permissões.',
      ].join(' ');
    } else {
      hintMatriz.innerHTML = [
        '<strong style="color:var(--cyan);">Cliente conectado</strong> — quando o servidor empurrar o rato para este PC, o cursor passa automaticamente.',
        'Para voltar ao servidor, empurre o cursor até à borda oposta (espelho da Matriz).',
        'No macOS: verifique <strong>Privacidade → Acessibilidade</strong> e <strong>Monitorização de Entrada</strong>.',
      ].join(' ');
    }
    return;
  }

  const role = (settings?.role ?? 'server').toLowerCase();
  const tLower = (status.status_text ?? '').toLowerCase();
  const servidorEscuta = role === 'server' && /aguardando/.test(tLower);
  const clienteLiga = role === 'client' && /conectando|aguardando/.test(tLower);

  if (servidorEscuta) {
    hintMatriz.style.display = 'block';
    hintMatriz.textContent =
      'Este PC está como Servidor à escuta. No outro computador: papel Cliente, IP deste PC (painel «Rede») e «Conectar». Para parar, use «Desconectar».';
  } else if (clienteLiga) {
    hintMatriz.style.display = 'block';
    // Se já ligou antes a um peer, é reconexão — mensagem menos alarmista
    hintMatriz.textContent = lastPeer
      ? `A reconectar a "${lastPeer}"… Aguarde ou use «Desconectar» para cancelar.`
      : 'A ligar ao servidor… Se falhar, confira IP, porta, firewall e a mesma chave (PSK) nos dois PCs.';
  } else {
    hintMatriz.style.display = 'none';
    hintMatriz.textContent = '';
  }
}

interface PeerInfo {
  hostname: string;
  addr: string;
  port: number;
}

interface SettingsPayload {
  hostname: string;
  screen_name?: string;
  expected_client_screen_name?: string | null;
  launch_connection_on_startup?: boolean;
  role: string;
  peer_position: string;
  setup_complete: boolean;
  port?: number;
}

export async function renderDashboard(): Promise<void> {
  const app = document.getElementById("app")!;

  // Ler versão real do tauri.conf.json em tempo de execução
  const appVersion = await getVersion().catch(() => __APP_VERSION__);

  app.innerHTML = `
    <div class="app">
      <!-- Sidebar -->
      <aside class="sidebar">
        <div class="sidebar-logo">
          <div class="logo-icon">
            <svg viewBox="0 0 24 24" fill="currentColor"><path d="M3 3h8v8H3zM13 3h8v8h-8zM3 13h8v8H3zM13 13h8v8h-8z"/></svg>
          </div>
          <div>
            <div class="logo-name">Movex</div>
            <div class="logo-version">V ${appVersion} · Online</div>
          </div>
        </div>

        <nav class="nav">
          <div class="nav-item active" id="nav-painel" data-nav="painel">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
            Painel
          </div>
          <div class="nav-item" id="nav-dispositivos" data-nav="dispositivos">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="4" width="20" height="14" rx="2"/><path d="M8 20h8M12 18v2"/></svg>
            Dispositivos
          </div>
          <div class="nav-item" id="nav-seguranca" data-nav="seguranca">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
            Segurança
          </div>
          <div class="nav-item" id="nav-configuracoes" data-nav="configuracoes">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
            Configurações
          </div>
        </nav>

        <div class="sidebar-footer">
          <button type="button" class="btn-add" id="btnAddMachine" title="Abre Dispositivos e conexão por IP">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" width="14" height="14"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            Adicionar Máquina
          </button>
        </div>
      </aside>

      <!-- Main -->
      <div class="main">
        <div class="topbar">
          <div class="topbar-title">
            <span class="dot"></span>
            <span id="pageTitle">Painel Principal</span>
          </div>
          <div class="topbar-right">
            <div class="topbar-icons">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M5 12.55a11 11 0 0114.08 0"/><path d="M1.42 9a16 16 0 0121.16 0"/><path d="M8.53 16.11a6 6 0 016.95 0"/><circle cx="12" cy="20" r="1" fill="currentColor"/></svg>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/></svg>
            </div>
            <div id="topbarRight">
              <div style="display:flex;align-items:center;gap:8px;">
                <div>
                  <div class="status-label">Sistema Pronto</div>
                  <div class="status-text" style="text-align:right">Admin Node</div>
                </div>
                <div class="user-avatar">A</div>
              </div>
            </div>
          </div>
        </div>

        <div class="content">

          <!-- PAINEL -->
          <div class="page active" id="page-painel">
            <div style="display:grid;grid-template-columns:1fr 260px;gap:16px;margin-bottom:20px;">
              <div class="card" style="position:relative;overflow:hidden;">
                <div style="position:absolute;top:-40px;right:-40px;width:200px;height:200px;border-radius:50%;background:radial-gradient(circle,rgba(0,212,255,.08) 0%,transparent 70%);pointer-events:none;"></div>
                <div style="display:inline-flex;align-items:center;gap:6px;padding:4px 10px;background:var(--cyan-dim);border:1px solid var(--border-c);border-radius:20px;font-size:10px;font-weight:600;color:var(--cyan);letter-spacing:.8px;text-transform:uppercase;margin-bottom:16px;">
                  <span style="width:5px;height:5px;border-radius:50%;background:var(--cyan);"></span>
                  Configuração em Tempo Real
                </div>
                <div style="font-size:28px;font-weight:800;color:var(--text);line-height:1.2;letter-spacing:-.5px;margin-bottom:20px;">Organize seu ambiente<br>de trabalho virtual.</div>
                <div style="display:flex;align-items:center;gap:10px;">
                  <div style="display:flex;gap:6px;">
                    <div style="width:24px;height:16px;border-radius:4px;background:var(--cyan);border:1px solid var(--cyan);"></div>
                    <div style="width:24px;height:16px;border-radius:4px;background:var(--cyan);border:1px solid var(--cyan);"></div>
                    <div style="width:24px;height:16px;border-radius:4px;background:var(--cyan);border:1px solid var(--cyan);"></div>
                  </div>
                  <span style="font-size:13px;color:var(--text-2);" id="nodesLabel">Esta máquina · obtendo endereço…</span>
                </div>
              </div>
              <div style="display:flex;flex-direction:column;gap:12px;">
                <div class="card">
                  <div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:10px;display:flex;justify-content:space-between;">Rede <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--cyan)" stroke-width="1.8"><path d="M5 12.55a11 11 0 0114.08 0"/><circle cx="12" cy="20" r="1" fill="var(--cyan)"/></svg></div>
                  <div style="font-size:30px;font-weight:700;color:var(--text);line-height:1;letter-spacing:-1px;">1.2<span style="font-size:14px;color:var(--text-2);margin-left:4px;">Gbps</span></div>
                  <div style="font-size:11px;color:var(--text-3);margin-top:6px;">Vazão contínua (estimativa)</div>
                  <div style="font-size:11px;color:var(--cyan);margin-top:10px;line-height:1.45;font-family:'JetBrains Mono',monospace;word-break:break-all;" id="panelLocalIpLine">—</div>
                  <div style="font-size:10px;color:var(--text-2);margin-top:6px;line-height:1.4;display:none;" id="panelPeerLine"></div>
                </div>
                <div class="card">
                  <div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:10px;">Latência</div>
                  <div style="font-size:30px;font-weight:700;color:var(--text);line-height:1;letter-spacing:-1px;" id="latencyVal"><span style="color:var(--text-3);">--</span><span style="font-size:14px;color:var(--text-3);margin-left:4px;">ms</span></div>
                  <div style="font-size:11px;color:var(--text-3);margin-top:6px;">Sincronização ponta a ponta</div>
                </div>
              </div>
            </div>

            <div class="card">
              <div style="display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:20px;">
                <div>
                  <div class="section-title">Matriz de Telas</div>
                  <div class="section-sub">Clique nas setas para definir a posição do outro monitor</div>
                  <div id="panelConnStatus" style="font-size:12px;font-weight:600;color:var(--text-2);margin-top:8px;min-height:18px;">Desconectado</div>
                  <div id="matrixConnHint" style="font-size:11px;color:var(--text-3);margin-top:6px;max-width:440px;line-height:1.45;display:none;"></div>
                </div>
                <div style="display:flex;gap:10px;">
                  <button type="button" class="btn btn-outline" id="btnSendFile">📁 Enviar Arquivo</button>
                  <button type="button" class="btn btn-outline" id="btnDisconnect" style="display:none;">Desconectar</button>
                  <button type="button" class="btn btn-cyan" id="btnConnect">Conectar</button>
                </div>
              </div>
              <!-- Seletor visual de posição do peer -->
              <div id="peerPositionSelector" style="display:none;margin-bottom:12px;padding:12px;background:var(--bg-2);border-radius:10px;">
                <div style="font-size:11px;color:var(--text-3);text-align:center;margin-bottom:8px;">Posição do monitor remoto:</div>
                <div style="display:grid;grid-template-columns:1fr 1fr 1fr;grid-template-rows:1fr 1fr 1fr;gap:4px;width:120px;margin:0 auto;">
                  <div></div>
                  <button id="pos-above" class="btn btn-outline" style="padding:6px;font-size:14px;">↑</button>
                  <div></div>
                  <button id="pos-left"  class="btn btn-outline" style="padding:6px;font-size:14px;">←</button>
                  <div style="background:var(--cyan-dim);border:1px solid var(--border-c);border-radius:6px;display:flex;align-items:center;justify-content:center;font-size:12px;color:var(--cyan);">📍</div>
                  <button id="pos-right" class="btn btn-cyan"    style="padding:6px;font-size:14px;">→</button>
                  <div></div>
                  <button id="pos-below" class="btn btn-outline" style="padding:6px;font-size:14px;">↓</button>
                  <div></div>
                </div>
              </div>
              <div id="screenMap" style="display:flex;align-items:center;justify-content:center;gap:24px;padding:20px 0;"></div>
            </div>

            <!-- Transferências em andamento -->
            <div id="transfersSection" style="display:none;margin-top:14px;">
              <div class="card">
                <div style="font-size:11px;font-weight:700;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Transferências</div>
                <div id="transfersList"></div>
              </div>
            </div>
          </div>

          <!-- DISPOSITIVOS -->
          <div class="page" id="page-dispositivos">
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:20px;">
              <div>
                <div class="section-title">Dispositivos</div>
                <div class="section-sub" id="deviceSubtitle">Carregando esta máquina…</div>
            </div>
              <div style="display:flex;gap:8px;">
                <button type="button" class="btn btn-outline" id="btnRefreshDevices">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.48"/></svg>
                  Atualizar
                </button>
                <button type="button" class="btn btn-cyan" id="btnAddManual">+ Manual</button>
              </div>
            </div>

            <!-- IP manual: <details> abre/fecha sem depender de JS no WebView -->
            <details id="manualIpDetails" class="manual-ip-details" style="margin-bottom:16px;background:var(--bg-3);border:1px solid var(--border-c);border-radius:12px;overflow:hidden;">
              <summary style="padding:14px 18px;cursor:pointer;font-size:13px;font-weight:600;color:var(--text);user-select:none;">Conectar por IP (toque para expandir)</summary>
              <div style="padding:0 18px 18px;">
                <div style="display:flex;gap:8px;flex-wrap:wrap;align-items:center;">
                  <input id="manualIp" type="text" placeholder="192.168.1.100" style="flex:1;min-width:140px;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                  <input id="manualPort" type="number" value="24800" style="width:90px;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                  <button type="button" class="btn btn-cyan" id="btnConnectManual">Conectar</button>
                  <button type="button" class="btn btn-outline" id="btnCloseManual">Fechar</button>
                </div>
              </div>
            </details>

            <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:14px;" id="deviceGrid"></div>
          </div>

          <!-- SEGURANÇA — Logs -->
          <div class="page" id="page-seguranca">
            <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-bottom:20px;">
              <div class="card"><div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Tempo de Sessão</div><div style="font-size:28px;font-weight:700;color:var(--text);letter-spacing:-1px;" id="uptimeVal">--</div></div>
              <div class="card"><div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Dados Transferidos</div><div style="font-size:28px;font-weight:700;color:var(--text);letter-spacing:-1px;" id="bytesTransferred">0 B</div><div style="font-size:10px;color:var(--text-3);margin-top:4px;" id="eventsCount">0 eventos</div></div>
              <div class="card"><div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Arquivos Trocados</div><div style="font-size:28px;font-weight:700;color:var(--text);letter-spacing:-1px;" id="filesCount">0</div><div style="font-size:10px;color:var(--text-3);margin-top:4px;">enviados + recebidos</div></div>
            </div>

            <div class="card" style="overflow:hidden;padding:0;margin-bottom:20px;">
              <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 16px;border-bottom:1px solid var(--border);background:var(--bg-3);">
                <div style="display:flex;gap:6px;"><div style="width:10px;height:10px;border-radius:50%;background:#ff5f57;"></div><div style="width:10px;height:10px;border-radius:50%;background:#febc2e;"></div><div style="width:10px;height:10px;border-radius:50%;background:#28c840;"></div></div>
                <code style="font-family:'JetBrains Mono',monospace;font-size:11px;color:var(--text-2);background:var(--bg-4);padding:3px 10px;border-radius:5px;">/var/log/movex/connection.log</code>
                <div style="display:flex;gap:12px;">
                  <button id="btnCopyLogs" style="display:flex;align-items:center;gap:6px;font-size:11px;color:var(--text-3);cursor:pointer;background:none;border:none;font-family:'Inter',sans-serif;">📋 Copiar</button>
                  <button id="btnClearLogs" style="display:flex;align-items:center;gap:6px;font-size:11px;color:var(--text-3);cursor:pointer;background:none;border:none;font-family:'Inter',sans-serif;">🗑 Limpar Logs</button>
                </div>
              </div>
              <div id="logBody" style="padding:16px 20px;max-height:300px;overflow-y:auto;"></div>
              <div style="padding:12px 20px;border-top:1px solid var(--border);display:flex;align-items:center;gap:10px;">
                <span style="color:var(--cyan);font-family:'JetBrains Mono',monospace;font-size:13px;">$</span>
                <input id="cmdInput" style="flex:1;background:none;border:none;outline:none;font-family:'JetBrains Mono',monospace;font-size:12px;color:var(--text-2);" placeholder="Digite um comando (ex: netstat -a)..." />
              </div>
            </div>
            <div style="display:flex;justify-content:flex-end;gap:12px;">
              <button type="button" class="btn btn-outline" id="btnDiagReport">Gerar Relatório Completo</button>
              <button type="button" class="btn btn-cyan" id="btnDiagRestart">Reiniciar Diagnósticos</button>
            </div>
          </div>

          <!-- CONFIGURAÇÕES -->
          <div class="page" id="page-configuracoes">

            <div id="configRoleBanner" class="card" style="margin-bottom:14px;border-left:3px solid var(--cyan);">
              <div id="configRoleBannerTitle" style="font-size:12px;font-weight:700;color:var(--text);">A configurar como Servidor</div>
              <div id="configRoleBannerDesc" style="font-size:12px;color:var(--text-2);line-height:1.5;margin-top:4px;">Esta máquina fica à escuta na rede. No outro PC, use o papel Cliente e o IP deste computador.</div>
            </div>

            <!-- Papel desta máquina -->
            <div class="card" style="margin-bottom:14px;">
              <div style="font-size:11px;font-weight:700;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:14px;">Papel desta Máquina</div>
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:16px;">
                <div id="roleServerCard" data-role="server" style="border:2px solid var(--cyan);background:linear-gradient(135deg,#0f1824,#0b0c10);border-radius:12px;padding:18px;cursor:pointer;transition:all .2s;">
                  <div style="width:36px;height:36px;background:var(--cyan-dim);border-radius:9px;display:flex;align-items:center;justify-content:center;margin-bottom:12px;">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--cyan)" stroke-width="1.8"><rect x="2" y="4" width="20" height="14" rx="2"/><path d="M8 20h8M12 18v2"/><path d="M6 9h.01M9 9h6"/></svg>
                  </div>
                  <div style="font-size:14px;font-weight:700;color:var(--text);margin-bottom:4px;">Servidor</div>
                  <div style="font-size:11px;color:var(--text-2);">Controla outras máquinas com este teclado e mouse</div>
                </div>
                <div id="roleClientCard" data-role="client" style="border:1.5px solid var(--border);background:var(--bg-3);border-radius:12px;padding:18px;cursor:pointer;transition:all .2s;">
                  <div style="width:36px;height:36px;background:var(--bg-5);border-radius:9px;display:flex;align-items:center;justify-content:center;margin-bottom:12px;">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--text-2)" stroke-width="1.8"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
                  </div>
                  <div style="font-size:14px;font-weight:700;color:var(--text);margin-bottom:4px;">Cliente</div>
                  <div style="font-size:11px;color:var(--text-2);">Recebe o controle de outra máquina</div>
                </div>
              </div>

              <!-- Endereço do servidor (só visível no modo cliente) -->
              <div id="serverAddrSection" style="display:none;padding:14px;background:var(--bg-2);border:1px solid var(--border-c);border-radius:10px;">
                <div style="font-size:12px;font-weight:600;color:var(--text);margin-bottom:8px;">Endereço do Servidor</div>
                <div style="display:flex;gap:8px;">
                  <input id="serverAddrInput" type="text" placeholder="Ex: 192.168.1.100 ou nome-do-pc.local"
                    style="flex:1;background:var(--bg-input,var(--bg));border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                  <button id="btnApplyServerAddr" class="btn btn-cyan" style="white-space:nowrap;">Salvar</button>
                </div>
                <div style="font-size:11px;color:var(--text-3);margin-top:6px;">IP ou hostname do computador servidor na rede local</div>
              </div>
            </div>

            <!-- Nome do ecrã (comportamento tipo Barrier/Deskflow) -->
            <div class="card" style="margin-bottom:14px;border-left:3px solid var(--cyan);">
              <div style="font-size:11px;font-weight:700;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:10px;">Nomes dos ecrãs</div>
              <div style="margin-bottom:12px;">
                <label for="screenNameInput" style="font-size:12px;font-weight:600;color:var(--text);display:block;margin-bottom:6px;">Nome do ecrã neste PC</label>
                <input id="screenNameInput" type="text" placeholder="Ex.: MacBook-Pro ou PC-Sala"
                  style="width:100%;background:var(--bg-input,var(--bg));border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                <div id="screenNameHelpServer" style="font-size:10px;color:var(--text-3);margin-top:6px;line-height:1.45;">Identifica este computador no handshake (estilo «Screen name» do Barrier). No servidor, pode restringir o nome do cliente com a opção abaixo.</div>
                <div id="screenNameHelpClient" style="display:none;font-size:10px;color:var(--text-3);margin-top:6px;line-height:1.45;">Identifica este computador no handshake. O encaixe dos monitores segue o nome que o vê o servidor; não use o filtro «aceitar só cliente…» (é só do outro PC).</div>
              </div>
              <div id="expectedClientWrap" style="margin-bottom:12px;">
                <label for="expectedClientScreenInput" style="font-size:12px;font-weight:600;color:var(--text);display:block;margin-bottom:6px;">Aceitar só cliente com este nome (opcional)</label>
                <input id="expectedClientScreenInput" type="text" placeholder="Vazio = aceitar qualquer nome de cliente"
                  style="width:100%;background:var(--bg-input,var(--bg));border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                <div style="font-size:10px;color:var(--text-3);margin-top:6px;">Apenas no <strong style="color:var(--text-2);">Servidor</strong>: rejeita clientes cujo nome de ecrã não coincide (exato).</div>
              </div>
              <label style="display:flex;align-items:flex-start;gap:10px;cursor:pointer;font-size:12px;color:var(--text-2);">
                <input type="checkbox" id="launchConnectionOnStartup" style="width:16px;height:16px;min-width:16px;accent-color:var(--cyan);margin-top:2px;" />
                <span id="launchConnLabel">Ao abrir o app, entrar em modo anfitrião (escutar na porta; desligado = liga manualmente com «Conectar» no painel, como no Barrier).</span>
              </label>
            </div>

            <!-- Porta + Chave -->
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:14px;">
              <div class="card">
                <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;">
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" stroke-width="1.8"><rect x="2" y="7" width="20" height="14" rx="2"/><path d="M16 7V5a2 2 0 00-2-2h-4a2 2 0 00-2 2v2"/></svg>
                  <span style="font-size:13px;font-weight:600;color:var(--text);">Porta TCP</span>
                  </div>
                <div style="position:relative;">
                  <input type="number" value="24800" id="portInput" style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:10px 80px 10px 14px;font-family:'JetBrains Mono',monospace;font-size:14px;font-weight:600;color:var(--text);outline:none;" />
                  <span style="position:absolute;right:10px;top:50%;transform:translateY(-50%);font-size:9px;font-weight:700;color:var(--text-3);text-transform:uppercase;">Padrão</span>
                </div>
              </div>
              <div class="card">
                <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;">
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" stroke-width="1.8"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
                  <span style="font-size:13px;font-weight:600;color:var(--text);">Chave (opcional)</span>
                </div>
                <div style="position:relative;">
                  <input type="password" id="keyInput" style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:10px 40px 10px 14px;font-family:'JetBrains Mono',monospace;font-size:13px;font-weight:600;color:var(--text);outline:none;letter-spacing:2px;" />
                  <button id="btnToggleKey" style="position:absolute;right:10px;top:50%;transform:translateY(-50%);background:none;border:none;cursor:pointer;color:var(--text-3);font-size:14px;">👁</button>
                </div>
                <div style="font-size:10px;color:var(--text-3);margin-top:6px;line-height:1.45;">A ligação <strong style="color:var(--text-2);">não depende</strong> de ser a mesma chave nos dois PCs (TLS). Pode alterar ou deixar o valor gerado — só afeta o handshake legado.</div>
              </div>
            </div>

            <!-- Checklist rede — conteúdo partido por papel (applyRoleUI mostra o bloco certo) -->
            <div class="card" id="networkChecklistCard" style="margin-bottom:14px;border-left:3px solid var(--cyan);">
              <div style="font-size:11px;font-weight:700;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:10px;">Alinhar a rede</div>

              <div id="netChecklistServer" style="display:none;">
                <ol style="margin:0 0 12px 0;padding-left:18px;font-size:12px;color:var(--text-2);line-height:1.65;">
                  <li><strong style="color:var(--text);">Escutar na rede:</strong> confirme o papel <em>Servidor</em> acima e use <strong>Conectar</strong> no painel (esta máquina fica à escuta na porta indicada).</li>
                  <li><strong style="color:var(--text);">Firewall (entrada):</strong> nesta máquina, a porta TCP (ex.: 24800) tem de aceitar ligações de entrada. No Windows, use o botão <strong>«Aplicar regras no firewall (admin)»</strong> em Permissões abaixo; no macOS, permita o Movex no Firewall de aplicação.</li>
                </ol>
              </div>

              <div id="netChecklistClient" style="display:none;">
                <ol style="margin:0 0 12px 0;padding-left:18px;font-size:12px;color:var(--text-2);line-height:1.65;">
                  <li><strong style="color:var(--text);">Ligar ao anfitrião:</strong> papel <em>Cliente</em> e o IP ou hostname do <strong>servidor</strong> em <strong>Endereço do Servidor</strong> acima (este PC não fica a aceitar ligações nessa mesma lógica; quem escuta é o outro).</li>
                  <li><strong style="color:var(--text);">Certificado TLS:</strong> se o Movex foi reinstalado <em>no PC servidor</em> ou mudou a máquina anfitriã, toque no botão abaixo <em>neste</em> computador (cliente) antes de voltar a ligar.</li>
                  <li><strong style="color:var(--text);">Teste de alcance:</strong> no terminal: <code style="font-size:11px;color:var(--cyan);">ping &lt;IP-do-servidor&gt;</code> — se não houver resposta, o Movex também não alcança.</li>
                </ol>
                <div id="configClientTlsRow" style="display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-bottom:12px;">
                  <button type="button" id="btnClearServerCert" class="btn btn-outline" style="font-size:11px;padding:6px 10px;font-weight:700;">Esquecer certificado TLS do servidor</button>
                  <span style="font-size:10px;color:var(--text-3);">Limpa a confiança do certificado do anfitrião (útil após trocar o PC servidor ou reinstalar a app lá).</span>
                </div>
              </div>

              <div id="netChecklistCommon">
                <ol style="margin:0;padding-left:18px;font-size:12px;color:var(--text-2);line-height:1.65;">
                  <li><strong style="color:var(--text);">Rede:</strong> os dois PCs na mesma LAN (mesmo Wi‑Fi ou cabo no mesmo router).</li>
                  <li><strong style="color:var(--text);">Wi‑Fi / router:</strong> desative «isolamento de cliente» / «AP isolation» se existir.</li>
                  <li><strong style="color:var(--text);">VPN:</strong> desative VPN para testar na LAN, se tiver dúvidas de encaminhamento.</li>
                </ol>
              </div>
              <div style="font-size:11px;color:var(--text-3);margin-top:10px;">Se aparecer «Timeout» ou «Reconectando», use <strong>Desconectar</strong>, confira o IP, o papel (Servidor/Cliente) e tente <strong>Conectar</strong> de novo.</div>
            </div>

            <!-- SSL + Clipboard + Tema + Lock -->
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:14px;">
              <div class="card" style="border-left:3px solid var(--cyan);">
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;">
                  <div style="font-size:14px;font-weight:700;color:var(--text);">🔒 Criptografia TLS 1.3</div>
                  <label class="toggle"><input type="checkbox" checked disabled /><div class="toggle-track"></div><div class="toggle-thumb"></div></label>
                </div>
                <div style="font-size:11px;color:var(--text-2);line-height:1.5;margin-bottom:10px;">Sempre ativa. AES-256 em todos os dados.</div>
                <div style="display:flex;gap:6px;">
                  <span style="padding:2px 8px;border-radius:4px;font-size:9px;font-weight:700;background:var(--cyan-dim);border:1px solid var(--border-c);color:var(--cyan);">AES-256</span>
                  <span style="padding:2px 8px;border-radius:4px;font-size:9px;font-weight:700;background:var(--cyan-dim);border:1px solid var(--border-c);color:var(--cyan);">TLS 1.3</span>
                </div>
              </div>
              <div class="card" style="border-left:3px solid var(--border);">
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;">
                  <div style="font-size:14px;font-weight:700;color:var(--text);">📋 Clipboard Compartilhado</div>
                  <label class="toggle"><input type="checkbox" checked id="clipboardToggle"/><div class="toggle-track"></div><div class="toggle-thumb"></div></label>
                </div>
                <div style="font-size:11px;color:var(--text-2);line-height:1.5;">Copie em um computador e cole no outro. Somente texto.</div>
              </div>
            </div>

            <!-- Preferências -->
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:14px;">
              <!-- Tema -->
              <div class="card">
                <div style="font-size:13px;font-weight:600;color:var(--text);margin-bottom:12px;">🎨 Tema</div>
                <div style="display:flex;gap:8px;">
                  <button id="themeDark" data-theme="dark" style="flex:1;padding:8px;border-radius:8px;border:2px solid var(--cyan);background:var(--bg-2);color:var(--text);font-size:12px;font-weight:600;cursor:pointer;">🌙 Escuro</button>
                  <button id="themeLight" data-theme="light" style="flex:1;padding:8px;border-radius:8px;border:1px solid var(--border);background:var(--bg-2);color:var(--text-2);font-size:12px;font-weight:600;cursor:pointer;">☀️ Claro</button>
              </div>
              </div>
              <!-- Modo Lock -->
              <div class="card">
                <div style="font-size:13px;font-weight:600;color:var(--text);margin-bottom:4px;">🔐 Modo Lock</div>
                <div style="font-size:11px;color:var(--text-3);margin-bottom:12px;">Pausa a transição de cursor entre telas</div>
                <div style="display:flex;gap:8px;align-items:center;">
                  <button id="btnLockMode" class="btn btn-outline" style="flex:1;">
                    🔓 Desbloqueado
                  </button>
                  <div style="font-size:10px;color:var(--text-3);">Atalho:<br><code id="lockKeyDisplay" style="color:var(--cyan);">Ctrl+Alt+L</code></div>
                </div>
              </div>
            </div>

            <!-- Notificações + Atalho -->
            <div class="card" style="margin-bottom:14px;">
              <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:8px;">
                <div>
                  <div style="font-size:13px;font-weight:600;color:var(--text);">🔔 Notificações do Sistema</div>
                  <div style="font-size:11px;color:var(--text-3);">Avisos ao conectar, desconectar e receber arquivos</div>
              </div>
                <label class="toggle"><input type="checkbox" id="notifToggle" checked /><div class="toggle-track"></div><div class="toggle-thumb"></div></label>
              </div>
              <div style="display:flex;align-items:center;gap:12px;margin-top:8px;padding-top:8px;border-top:1px solid var(--border);">
                <div style="font-size:12px;font-weight:600;color:var(--text);flex-shrink:0;">Atalho Lock:</div>
                <input id="lockKeyInput" type="text" value="ctrl+alt+l"
                  style="flex:1;background:var(--bg-2);border:1px solid var(--border);border-radius:6px;padding:6px 10px;font-family:'JetBrains Mono',monospace;font-size:12px;color:var(--cyan);outline:none;" />
                <div style="font-size:10px;color:var(--text-3);">ex: ctrl+alt+l</div>
              </div>
            </div>

            <!-- Permissões do SO (atalhos para Definições) -->
            <div class="card" style="margin-bottom:14px;" id="permCard">
              <div style="font-size:13px;font-weight:600;color:var(--text);margin-bottom:6px;">🔐 Permissões do sistema</div>
              <div style="font-size:11px;color:var(--text-3);margin-bottom:10px;line-height:1.45;">
                O KVM precisa de permissões de entrada (macOS: Acessibilidade e, em alguns sistemas, Monitorização de entrada).
                As notificações podem ser pedidas automaticamente ao abrir a app; use os botões abaixo se precisar abrir os painéis manualmente.
              </div>
              <div id="permRowMac" style="display:none;flex-wrap:wrap;gap:8px;margin-bottom:8px;">
                <div id="accessibilityWarningBanner" style="display:none;width:100%;background:rgba(255,160,0,0.12);border:1px solid var(--warn,#f0a000);border-radius:6px;padding:8px 12px;margin-bottom:4px;font-size:11px;line-height:1.5;">
                  ⚠️ <strong>Acessibilidade não concedida</strong> — o mouse/teclado do outro PC não será injetado neste Mac.<br>
                  Após updates o macOS invalida a permissão anterior. Clique <strong>Corrigir</strong> para remover a entrada antiga e conceder novamente.
                  <div style="margin-top:6px;display:flex;gap:8px;flex-wrap:wrap;">
                    <button type="button" id="btnFixAccessibility" class="btn" style="font-size:11px;padding:5px 12px;background:var(--warn,#f0a000);color:#000;font-weight:700;border:none;">Corrigir automaticamente</button>
                    <button type="button" id="btnPermMacAccessibility2" class="btn btn-outline" style="font-size:11px;padding:5px 10px;">Abrir Acessibilidade</button>
                  </div>
                </div>
                <button type="button" id="btnPermMacAccessibility" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">macOS · Acessibilidade</button>
                <button type="button" id="btnPermMacInput" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">macOS · Monitorização de entrada</button>
                <button type="button" id="btnPermMacNotif" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">macOS · Notificações</button>
              </div>
              <div id="permRowWin" style="display:none;flex-direction:column;gap:10px;">
                <div id="permWinServerInbound" style="display:none;flex-direction:column;gap:8px;">
                  <div style="font-size:10px;color:var(--text-3);line-height:1.45;">
                    No papel <strong>Servidor</strong>, a primeira ligação pode pedir o UAC para regras de <em>entrada</em> no Firewall (aceite «Sim»).
                    Se tiver recusado antes, use o botão abaixo. Em proxy corporativo: <strong>Proxy do sistema</strong>.
                  </div>
                  <div style="display:flex;flex-wrap:wrap;gap:8px;">
                    <button type="button" id="btnPermWinApplyFw" class="btn btn-outline" style="font-size:11px;padding:6px 10px;font-weight:700;">Aplicar regras no firewall (admin)</button>
                  </div>
                </div>
                <div style="display:flex;flex-wrap:wrap;gap:8px;">
                  <button type="button" id="btnPermWinFirewallAdv" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">Firewall e segurança</button>
                  <button type="button" id="btnPermWinProxy" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">Proxy do sistema</button>
                  <button type="button" id="btnPermWinNotif" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">Notificações</button>
                  <button type="button" id="btnPermWinFirewall" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">Firewall (lista clássica)</button>
                  <button type="button" id="btnPermWinPrivacy" class="btn btn-outline" style="font-size:11px;padding:6px 10px;">Privacidade</button>
                </div>
            </div>

              <!-- Diagnóstico -->
              <div style="margin-top:10px;padding-top:10px;border-top:1px solid var(--border);">
                <div style="display:flex;align-items:center;justify-content:space-between;gap:8px;flex-wrap:wrap;">
                  <div style="font-size:11px;color:var(--text-3);line-height:1.45;flex:1;min-width:160px;">
                    Não conecta? Roda o diagnóstico — testa rede, mDNS, TCP e permissões; mostra exatamente onde travou.
                  </div>
                  <button type="button" id="btnDiagnose" class="btn btn-outline" style="font-size:11px;padding:6px 10px;font-weight:700;">🔍 Diagnosticar conexão</button>
                </div>
                <pre id="diagnoseOutput" style="display:none;margin-top:10px;padding:10px;background:var(--bg-2);border:1px solid var(--border);border-radius:6px;font-family:'JetBrains Mono',monospace;font-size:11px;color:var(--text);white-space:pre-wrap;word-break:break-all;max-height:300px;overflow:auto;"></pre>
              </div>
          </div>

            <!-- Peers recentes -->
            <div class="card" style="margin-bottom:14px;">
              <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:12px;">
                <div style="font-size:13px;font-weight:600;color:var(--text);">🕐 Conexões Recentes</div>
                <button id="btnClearHistory" class="btn btn-outline" style="font-size:10px;padding:4px 10px;">Limpar</button>
        </div>
              <div id="recentPeersList" style="display:flex;flex-direction:column;gap:6px;">
                <div style="font-size:11px;color:var(--text-3);">Nenhuma conexão ainda</div>
      </div>
    </div>

            <!-- Ações -->
            <div style="display:flex;align-items:center;justify-content:space-between;padding-top:16px;border-top:1px solid var(--border);">
              <!-- Reset -->
              <button id="btnConfirmReset" style="display:flex;align-items:center;gap:8px;padding:10px 16px;border-radius:8px;border:1px solid rgba(255,75,110,.3);background:rgba(255,75,110,.08);color:var(--danger,#ff4b6e);font-family:'Inter',sans-serif;font-size:12px;font-weight:600;cursor:pointer;letter-spacing:.3px;transition:all .15s;">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.48"/></svg>
                Resetar Configurações
              </button>
              <div style="display:flex;gap:10px;">
                <button id="btnDiscardSettings" class="btn-ghost" style="font-size:12px;">Descartar</button>
                <button id="btnSaveConfig" class="btn btn-cyan">Salvar Configurações</button>
              </div>
            </div>

            <!-- Modal de confirmação de reset -->
            <div id="resetModal" style="display:none;position:fixed;inset:0;background:rgba(0,0,0,.7);z-index:999;align-items:center;justify-content:center;">
              <div style="background:var(--bg-3);border:1px solid var(--border);border-radius:16px;padding:28px;width:380px;text-align:center;">
                <div style="font-size:32px;margin-bottom:12px;">⚠️</div>
                <div style="font-size:16px;font-weight:700;color:var(--text);margin-bottom:8px;">Resetar Configurações?</div>
                <div style="font-size:13px;color:var(--text-2);line-height:1.5;margin-bottom:24px;">Todas as configurações serão apagadas e o assistente de configuração inicial será exibido novamente.</div>
                <div style="display:flex;gap:10px;justify-content:center;">
                  <button id="btnCancelReset" style="padding:10px 24px;border-radius:8px;border:1px solid var(--border);background:var(--bg-4);color:var(--text-2);font-family:'Inter',sans-serif;font-size:13px;font-weight:600;cursor:pointer;">Cancelar</button>
                  <button id="btnDoReset" style="padding:10px 24px;border-radius:8px;border:none;background:var(--danger,#ff4b6e);color:#fff;font-family:'Inter',sans-serif;font-size:13px;font-weight:700;cursor:pointer;">Sim, Resetar</button>
                </div>
              </div>
            </div>
          </div>

        </div>
      </div>
    </div>
  `;

  // ── Sistema de navegação local (sem window.navTo) ───────────────────────────
  const navTo = (page: string) => {
    document.querySelectorAll('.page').forEach(p => (p as HTMLElement).classList.remove('active'));
    document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
    document.getElementById(`page-${page}`)?.classList.add('active');
    document.getElementById(`nav-${page}`)?.classList.add('active');
    const titles: Record<string, string> = {
      painel: 'Painel Principal',
      dispositivos: 'Dispositivos',
      seguranca: 'Logs de Segurança · Interface de Diagnóstico',
      configuracoes: 'Segurança & Conexão',
    };
    const titleEl = document.getElementById('pageTitle');
    if (titleEl) titleEl.textContent = titles[page] ?? page;
    // Carregar dados da aba ao navegar
    if (page === 'configuracoes') {
      loadRecentPeers?.();
      void loadCurrentSettings();
    }
    if (page === 'dispositivos') {
      queueMicrotask(() => {
        void refreshDevices();
      });
    }
  };

  // Navegação da sidebar
  document.querySelectorAll('.nav-item[data-nav]').forEach(el => {
    el.addEventListener('click', () => navTo((el as HTMLElement).dataset.nav!));
  });

  // ── Cache de settings (raramente muda — evitar invoke por evento de status) ──
  let cachedSettings: any = null;
  const refreshSettings = async () => {
    try { cachedSettings = await invoke<any>('get_settings'); } catch { /* fora do Tauri */ }
  };
  await refreshSettings();
  try {
    const ips = await invoke<string[]>('get_local_ipv4_addrs');
    movexLocalIpv4Cache = ips?.length ? ips.join(' · ') : '';
  } catch {
    movexLocalIpv4Cache = '';
  }

  const paintPanelNetworkImmediate = () => {
    const port = cachedSettings?.port ?? 24800;
    const localLine = formatLocalNetworkLine(port, movexLocalIpv4Cache);
    const lip = document.getElementById('panelLocalIpLine');
    if (lip) lip.textContent = localLine;
    const nodesSummary = document.getElementById('nodesLabel');
    if (nodesSummary) {
      const parts = movexLocalIpv4Cache.split(' · ').filter(Boolean);
      nodesSummary.textContent =
        parts.length > 0
          ? `Esta máquina · ${parts.length} IPv4 na LAN · aguardando par`
          : 'Esta máquina · IPv4 não detectado (rede ou permissões)';
    }
  };
  paintPanelNetworkImmediate();

  let stopStatusPolling: () => void = () => {};

  // Último peer com ligação bem-sucedida — usado para distinguir reconexão de 1.ª ligação
  let lastConnectedPeer: string | null = null;

  // Handlers registados ANTES de initStatusListener — o primeiro get_status já preenche IP/latência
  onStatusChange(async (status) => {
    let settings = cachedSettings;
    if (!settings) {
      await refreshSettings();
      settings = cachedSettings;
    }
    {
      const line = document.getElementById('panelConnStatus');
      if (line) {
        const t = (status.status_text ?? '').trim();
        // Mostrar texto claro: quando conectado nunca mostrar "conectando"
        if (status.connected) {
          const peer = (status.peer_hostname ?? t.replace(/^Conectado a\s*/i, '').split('@')[0].trim()) || 'Par';
          lastConnectedPeer = peer;
          line.textContent = `● Conectado a ${peer}`;
          line.style.color = 'var(--cyan)';
        } else if (!t) {
          line.textContent = lastConnectedPeer ? `● Desconectado (era: ${lastConnectedPeer})` : 'Desconectado';
          line.style.color = 'var(--text-2)';
        } else {
          // Aguardando, Conectando..., Reconectando...
          // Se já ligou antes, mostrar "Reconectando a [peer]" em vez do genérico "Conectando..."
          const isReconnect = lastConnectedPeer && /conectando/i.test(t);
          line.textContent = isReconnect ? `● Reconectando a ${lastConnectedPeer}…` : t;
          const isTransient = /aguardando|conectando|reconect/i.test(t);
          line.style.color = isTransient ? 'var(--cyan)' : 'var(--text-2)';
        }
      }
    }
    try {
      try {
        const ips = await invoke<string[]>('get_local_ipv4_addrs');
        movexLocalIpv4Cache = ips?.length ? ips.join(' · ') : '';
      } catch {
        /* ignora */
      }

      const port = settings?.port ?? 24800;
      const localLine = formatLocalNetworkLine(port, movexLocalIpv4Cache);
      const localIpLine = document.getElementById('panelLocalIpLine');
      if (localIpLine) {
        localIpLine.textContent = localLine;
      }

      const peerLine = document.getElementById('panelPeerLine');
      if (peerLine) {
        if (status.connected && (status.peer_hostname || status.peer_addr)) {
          peerLine.style.display = 'block';
          const host = status.peer_hostname ?? 'Par';
          const addr = status.peer_addr?.trim();
          peerLine.textContent = addr ? `${host} · ${addr}` : host;
        } else {
          peerLine.style.display = 'none';
          peerLine.textContent = '';
        }
      }

      const nodesSummary = document.getElementById('nodesLabel');
      if (nodesSummary && !status.connected && !sessionLooksActive(status)) {
        const parts = movexLocalIpv4Cache.split(' · ').filter(Boolean);
        nodesSummary.textContent =
          parts.length > 0
            ? `Esta máquina · ${parts.length} IPv4 na LAN · aguardando par`
            : 'Esta máquina · IPv4 não detectado (rede ou permissões)';
      }

      const settingsForMap = settings ?? cachedSettings;
      if (settingsForMap) {
        updateScreenMap(settingsForMap, status);
      }
      const latEl = document.getElementById('latencyVal');
      if (latEl) {
        if (status.connected && status.latency_ms != null) {
          if (status.latency_ms > 0) {
            latEl.innerHTML = `${status.latency_ms}<span style="font-size:14px;color:var(--text-2);margin-left:4px;">ms</span>`;
          } else {
            latEl.innerHTML = `<span style="color:var(--text-3);font-size:20px;">medição…</span><span style="font-size:14px;color:var(--text-3);margin-left:4px;">ms</span>`;
          }
        } else {
          latEl.innerHTML = `<span style="color:var(--text-3);">--</span><span style="font-size:14px;color:var(--text-3);margin-left:4px;">ms</span>`;
        }
      }
      const uptimeEl = document.getElementById('uptimeVal');
      if (uptimeEl) {
        const secs = status.uptime_secs ?? 0;
        const hrs  = Math.floor(secs / 3600);
        const mins = Math.floor((secs % 3600) / 60);
        const uptimeStr = hrs > 0 ? `${hrs}h ${mins}m` : secs > 0 ? `${mins}m ${secs % 60}s` : '--';
        uptimeEl.innerHTML = `<span style="font-size:${hrs > 0 ? '28' : '22'}px">${uptimeStr}</span><span style="font-size:12px;color:var(--text-2);margin-left:4px;">uptime</span>`;
      }
      applyPanelConnectionButtons(status);
      updateMatrixHint(settings ?? null, status, lastConnectedPeer);
      updateDevices(status, settings ?? null);
      // Mostrar seletor de posição quando em sessão (à escuta ou conectado ao par)
      const posSelector = document.getElementById('peerPositionSelector');
      if (posSelector) posSelector.style.display = sessionLooksActive(status) ? 'block' : 'none';
      // Borda luminosa — cliente ativo
      {
        const role = settings?.role ?? 'server';
        const isClient = role === 'client';
        const isRemoteActive = status.active_screen === 'Remote';
        invoke('set_screen_border', { active: isClient && isRemoteActive && status.connected, color: '#00d4ff' }).catch(() => {});
      }
      // Stats
      try {
        const stats = await invoke<any>('get_stats');
        const bytes = (n: number) => {
          if (n >= 1073741824) return `${(n/1073741824).toFixed(1)} GB`;
          if (n >= 1048576)    return `${(n/1048576).toFixed(1)} MB`;
          if (n >= 1024)       return `${(n/1024).toFixed(0)} KB`;
          return `${n} B`;
        };
        const total = (stats.bytes_sent ?? 0) + (stats.bytes_received ?? 0);
        const totalEvents = (stats.events_sent ?? 0) + (stats.events_received ?? 0);
        const totalFiles = (stats.files_sent ?? 0) + (stats.files_received ?? 0);
        // Painel principal
        const nodesEl = document.getElementById('nodesLabel');
        if (nodesEl && status.connected) {
          const peerHint = status.peer_addr ? ` · ${status.peer_addr}` : '';
          nodesEl.textContent = `2 nós ativos${peerHint} · ${bytes(total)} transferidos`;
        }
        // Aba Segurança — dados reais
        const bytesEl = document.getElementById('bytesTransferred');
        if (bytesEl) bytesEl.textContent = bytes(total);
        const eventsEl = document.getElementById('eventsCount');
        if (eventsEl) eventsEl.textContent = `${totalEvents.toLocaleString()} eventos`;
        const filesEl = document.getElementById('filesCount');
        if (filesEl) filesEl.textContent = String(totalFiles);
      } catch { /* sem stats */ }
      // Transferências
      try {
        const transfers = await invoke<any[]>('get_transfers');
        const section = document.getElementById('transfersSection');
        const list = document.getElementById('transfersList');
        if (section && list) {
          section.style.display = transfers.length > 0 ? 'block' : 'none';
          list.innerHTML = transfers.map(t => `
            <div style="display:flex;align-items:center;gap:12px;padding:8px 0;border-bottom:1px solid var(--border);">
              <span style="font-size:18px;">${t.direction === 'Sending' ? '📤' : '📥'}</span>
              <div style="flex:1;">
                <div style="font-size:13px;font-weight:600;color:var(--text);">${esc(t.name)}</div>
                <div style="background:var(--bg-5);border-radius:3px;height:4px;margin-top:4px;">
                  <div style="background:var(--cyan);height:4px;border-radius:3px;width:${t.percent ?? 0}%;transition:width .3s;"></div>
                </div>
              </div>
              <span style="font-size:11px;color:var(--text-3);">${t.percent ?? 0}%</span>
            </div>
          `).join('');
        }
      } catch { /* sem transferências */ }
    } catch (err) {
      console.warn('[Movex] atualização do painel:', err);
    }
  });

  try {
    await initScreenBorder();
    await initFileTransfer();
    stopStatusPolling = await initStatusListener();
  } catch (e) {
    console.warn("[Movex] Inicialização Tauri (eventos/status) — modo browser ou API indisponível:", e);
    addLog(
      isTauri()
        ? "Parte da integração nativa não iniciou; reinicie a app se o painel ficar estranho."
        : "Modo pré-visualização: abra pelo app Movex (Tauri), não só pelo navegador em localhost — os botões de conexão precisam do backend.",
      "warn",
    );
  }

  (window as any).__movexCleanup = () => {
    stopStatusPolling();
    cleanupFileTransfer();
    cleanupAllListeners();
    cleanupStatusHandlers();
  };

  // wrapper de cache será instalado após saveConfig ser definido

  addLog("Movex iniciado.", "info");
  addLog("Aguardando conexões na porta 24800.", "info");

  const copyLogs = async () => {
    const text = document.getElementById('logBody')?.innerText ?? '';
    try {
      await navigator.clipboard.writeText(text);
      addLog('Logs copiados para a área de transferência.', 'sec');
    } catch {
      try {
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.left = '-9999px';
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        document.body.removeChild(ta);
        addLog('Logs copiados (método alternativo).', 'sec');
      } catch (e) {
        addLog(`Não foi possível copiar logs: ${e}`, 'warn');
      }
    }
  };
  const toggleKey = () => {
    const inp = document.getElementById('keyInput') as HTMLInputElement;
    if (inp) inp.type = inp.type === 'password' ? 'text' : 'password';
  };
  // ── Configurações ──────────────────────────────────────────────────────────

  let currentRole = (cachedSettings?.role as string | undefined) ?? 'server';

  const loadCurrentSettings = async () => {
    try {
      const s = await invoke<any>('get_settings');
      currentRole = s.role ?? 'server';
      applyRoleUI(currentRole);

      const addrInput = document.getElementById('serverAddrInput') as HTMLInputElement;
      if (addrInput) addrInput.value = s.server_addr ?? '';

      const keyInput = document.getElementById('keyInput') as HTMLInputElement;
      if (keyInput && s.psk_hex) {
        keyInput.value = s.psk_hex;
      }

      const portInput = document.getElementById('portInput') as HTMLInputElement;
      if (portInput) portInput.value = String(s.port ?? 24800);

      const screenEl = document.getElementById('screenNameInput') as HTMLInputElement;
      if (screenEl) screenEl.value = s.screen_name ?? s.hostname ?? '';

      const expEl = document.getElementById('expectedClientScreenInput') as HTMLInputElement;
      if (expEl) expEl.value = s.expected_client_screen_name ?? '';

      const launchEl = document.getElementById('launchConnectionOnStartup') as HTMLInputElement;
      if (launchEl) launchEl.checked = !!s.launch_connection_on_startup;

      const notifEl = document.getElementById('notifToggle') as HTMLInputElement;
      if (notifEl) notifEl.checked = s.notifications_enabled ?? true;
      const clipEl = document.getElementById('clipboardToggle') as HTMLInputElement;
      if (clipEl) clipEl.checked = s.clipboard_sync_enabled ?? true;

      const lockKeyEl = document.getElementById('lockKeyDisplay');
      const lockKeyInput = document.getElementById('lockKeyInput') as HTMLInputElement;
      const lk = s.lock_key ?? 'ctrl+alt+l';
      if (lockKeyEl) lockKeyEl.textContent = lk;
      if (lockKeyInput) lockKeyInput.value = lk;

      const btnLock = document.getElementById('btnLockMode') as HTMLButtonElement | null;
      if (btnLock) {
        const locked = s.lock_mode ?? false;
        btnLock.textContent = locked ? '🔒 Bloqueado' : '🔓 Desbloqueado';
        btnLock.style.background = locked ? 'rgba(245,166,35,.15)' : '';
        btnLock.style.borderColor = locked ? 'var(--warn)' : '';
        btnLock.style.color = locked ? 'var(--warn)' : '';
      }

      const theme = s.theme ?? 'dark';
      const { applyTheme } = await import('../main');
      applyTheme(theme);
      const dark = document.getElementById('themeDark') as HTMLButtonElement | null;
      const light = document.getElementById('themeLight') as HTMLButtonElement | null;
      if (dark && light) {
        dark.style.borderColor = theme === 'dark' ? 'var(--cyan)' : 'var(--border)';
        dark.style.color = theme === 'dark' ? 'var(--text)' : 'var(--text-2)';
        light.style.borderColor = theme === 'light' ? 'var(--cyan)' : 'var(--border)';
        light.style.color = theme === 'light' ? 'var(--text)' : 'var(--text-2)';
      }
    } catch { /* fora do Tauri */ }
  };

  const applyRoleUI = (role: string) => {
    const serverCard = document.getElementById('roleServerCard');
    const clientCard = document.getElementById('roleClientCard');
    const addrSection = document.getElementById('serverAddrSection');
    const expectedWrap = document.getElementById('expectedClientWrap') as HTMLElement | null;
    if (!serverCard || !clientCard || !addrSection) return;

    if (expectedWrap) expectedWrap.style.display = role === 'server' ? 'block' : 'none';

    const bannerTitle = document.getElementById('configRoleBannerTitle');
    const bannerDesc = document.getElementById('configRoleBannerDesc');
    if (bannerTitle && bannerDesc) {
      if (role === 'server') {
        bannerTitle.textContent = 'A configurar como Servidor';
        bannerDesc.textContent =
          'Esta máquina fica à escuta na rede. No outro PC, use o papel Cliente e o IP deste computador.';
      } else {
        bannerTitle.textContent = 'A configurar como Cliente';
        bannerDesc.textContent =
          'Esta máquina liga-se ao anfitrião. Preencha «Endereço do Servidor» com o IP do PC em modo Servidor e use Conectar no painel.';
      }
    }

    const helpSrv = document.getElementById('screenNameHelpServer') as HTMLElement | null;
    const helpCli = document.getElementById('screenNameHelpClient') as HTMLElement | null;
    if (helpSrv && helpCli) {
      helpSrv.style.display = role === 'server' ? 'block' : 'none';
      helpCli.style.display = role === 'client' ? 'block' : 'none';
    }

    const launchLbl = document.getElementById('launchConnLabel');
    if (launchLbl) {
      launchLbl.textContent =
        role === 'server'
          ? 'Ao abrir o app, entrar em modo anfitrião (escutar na porta; desligado = liga manualmente com «Conectar» no painel, como no Barrier).'
          : 'Ao abrir o app, tentar ligar ao servidor automaticamente (use o endereço em «Endereço do Servidor»; desligado = liga só ao premir «Conectar» no painel).';
    }

    const netS = document.getElementById('netChecklistServer') as HTMLElement | null;
    const netC = document.getElementById('netChecklistClient') as HTMLElement | null;
    if (netS) netS.style.display = role === 'server' ? 'block' : 'none';
    if (netC) netC.style.display = role === 'client' ? 'block' : 'none';

    const permSrv = document.getElementById('permWinServerInbound') as HTMLElement | null;
    if (permSrv) {
      permSrv.style.display = role === 'server' ? 'flex' : 'none';
      permSrv.style.flexDirection = 'column';
      permSrv.style.gap = '8px';
    }

    if (role === 'server') {
      serverCard.style.cssText = serverCard.style.cssText.replace(/border:[^;]+/, 'border:2px solid var(--cyan)');
      serverCard.style.background = 'linear-gradient(135deg,#0f1824,#0b0c10)';
      clientCard.style.cssText = clientCard.style.cssText.replace(/border:[^;]+/, 'border:1.5px solid var(--border)');
      clientCard.style.background = 'var(--bg-3)';
      addrSection.style.display = 'none';
    } else {
      clientCard.style.cssText = clientCard.style.cssText.replace(/border:[^;]+/, 'border:2px solid var(--cyan)');
      clientCard.style.background = 'linear-gradient(135deg,#0f1824,#0b0c10)';
      serverCard.style.cssText = serverCard.style.cssText.replace(/border:[^;]+/, 'border:1.5px solid var(--border)');
      serverCard.style.background = 'var(--bg-3)';
      addrSection.style.display = 'block';
    }
  };

  const selectRoleCard = async (role: string) => {
    currentRole = role;
    applyRoleUI(role);
    // switch_role cancela conexão ativa, salva e relança automaticamente
    await invoke('switch_role', { role }).catch(console.warn);
    await refreshSettings();
    try {
      const raw = await invoke<unknown>('get_status');
      updateDevices(normalizeStatusPayload(raw), cachedSettings ?? null);
    } catch {
      /* ignorar */
    }
    addLog(`Papel alterado para: ${role === 'server' ? 'Servidor' : 'Cliente'} (reconectando...)`, 'sec');
  };

  const applyServerAddr = async () => {
    const input = document.getElementById('serverAddrInput') as HTMLInputElement;
    const addr = input?.value.trim() || null;
    await invoke('set_server_addr', { addr }).catch(console.warn);
    addLog(`Endereço do servidor: ${addr ?? '(removido)'}`, 'info');
  };

  const confirmReset = () => {
    const modal = document.getElementById('resetModal')!;
    if (modal) modal.style.display = 'flex';
  };

  const closeResetModal = () => {
    const modal = document.getElementById('resetModal')!;
    if (modal) modal.style.display = 'none';
  };

  const doReset = async () => {
    try {
      await invoke('reset_settings');
      addLog("Configurações resetadas. Reiniciando...", "warn");
      // Limpar todos os listeners antes de recarregar
      (window as any).__movexCleanup?.(); // limpar listeners registrados em módulos externos
      setTimeout(() => window.location.reload(), 800);
    } catch(e) {
      addLog(`Erro ao resetar: ${e}`, 'warn');
    }
  };

  const getThemeFromUI = (): string =>
    document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark';

  const saveConfig = async () => {
    const port = parseInt((document.getElementById('portInput') as HTMLInputElement).value) || 24800;
    const keyVal = (document.getElementById('keyInput') as HTMLInputElement)?.value.trim();
    const theme = getThemeFromUI();
    const notif = (document.getElementById('notifToggle') as HTMLInputElement)?.checked ?? true;
    const clipboard = (document.getElementById('clipboardToggle') as HTMLInputElement)?.checked ?? true;
    const lockKey =
      (document.getElementById('lockKeyInput') as HTMLInputElement)?.value?.trim() || 'ctrl+alt+l';
    try {
      await invoke('update_preferences', {
        notificationsEnabled: notif,
        lockKey,
        clipboardSyncEnabled: clipboard,
        theme,
      });
      const s = await invoke<any>('get_settings');
      const screenName =
        (document.getElementById('screenNameInput') as HTMLInputElement)?.value?.trim() || s.hostname;
      const expectedRaw =
        (document.getElementById('expectedClientScreenInput') as HTMLInputElement)?.value?.trim() || '';
    await invoke('save_settings', {
        hostname: s.hostname,
        screenName,
        expectedClientScreenName: expectedRaw ? expectedRaw : null,
        launchConnectionOnStartup:
          (document.getElementById('launchConnectionOnStartup') as HTMLInputElement)?.checked ?? false,
        role: currentRole,
        serverAddr: (document.getElementById('serverAddrInput') as HTMLInputElement)?.value.trim() || null,
        port,
        pskHex: keyVal || s.psk_hex,
        peerPosition: s.peer_position ?? 'right',
        autostart: s.autostart ?? false,
        theme,
      });
      addLog('Configurações salvas com sucesso.', 'sec');
    } catch (e) {
      addLog(`Erro ao salvar: ${e}`, 'warn');
    }
  };
  const saveConfigAndRefresh = async () => {
    await saveConfig();
    await refreshSettings();
  };

  const discardSettings = async () => {
    await refreshSettings();
    await loadCurrentSettings();
    addLog('Alterações descartadas.', 'info');
  };

  await loadCurrentSettings();

  const openSystemPanel = async (panel: string) => {
    if (!isTauri()) {
      addLog('Use a aplicação Movex instalada para abrir as definições do sistema.', 'warn');
      return;
    }
    try {
      await invoke('open_system_settings', { panel });
      addLog(`Definições do sistema abertas (${panel}).`, 'info');
    } catch (e) {
      addLog(`Não foi possível abrir definições: ${e}`, 'warn');
    }
  };

  const applyWindowsFirewallRules = async () => {
    if (!isTauri()) {
      addLog('Use a aplicação Movex instalada para aplicar regras de firewall.', 'warn');
      return;
    }
    await refreshSettings();
    const port = Number((cachedSettings as { port?: number } | null)?.port ?? 24800);
    if (!Number.isFinite(port) || port < 1 || port > 65535) {
      addLog('Porta inválida nas configurações.', 'warn');
      return;
    }
    try {
      const msg = await invoke<string>('windows_apply_firewall_rules_for_movex', { port });
      addLog(msg, 'sec');
    } catch (e) {
      addLog(`Firewall: ${e}`, 'warn');
    }
  };

  const runDiagnose = async () => {
    if (!isTauri()) {
      addLog('Diagnóstico só funciona dentro da aplicação Movex.', 'warn');
      return;
    }
    const out = document.getElementById('diagnoseOutput') as HTMLPreElement | null;
    const btn = document.getElementById('btnDiagnose') as HTMLButtonElement | null;
    if (out) {
      out.style.display = 'block';
      out.textContent = '⏳ Diagnosticando rede e permissões…';
    }
    if (btn) btn.disabled = true;
    try {
      type Diag = {
        local_ips: string[];
        server_addr: string | null;
        server_port: number;
        tcp_reachable: boolean | null;
        tcp_error: string | null;
        mdns_peers: string[];
        firewall_rules_present: boolean;
        macos_accessibility: boolean;
        elapsed_ms: number;
      };
      const r = await invoke<Diag>('diagnose_connection');
      const lines: string[] = [];
      lines.push('═══ Diagnóstico Movex ═══');
      lines.push(`tempo total: ${r.elapsed_ms} ms`);
      lines.push('');
      lines.push('▸ Rede local');
      lines.push(`  IPs deste PC: ${r.local_ips.join(', ') || '(nenhum)'}`);
      lines.push('');
      lines.push('▸ Servidor configurado');
      if (r.server_addr) {
        lines.push(`  endereço: ${r.server_addr}:${r.server_port}`);
        if (r.tcp_reachable === true) {
          lines.push('  TCP: ✅ alcançável');
        } else {
          lines.push(`  TCP: ❌ ${r.tcp_error ?? 'falhou'}`);
        }
      } else {
        lines.push('  ⚠ Sem server_addr — papel Servidor ou ainda não configurado.');
      }
      lines.push('');
      lines.push('▸ mDNS (descoberta automática)');
      if (r.mdns_peers.length > 0) {
        for (const p of r.mdns_peers) lines.push(`  ✅ ${p}`);
      } else {
        lines.push('  ⚠ nenhum peer Movex anunciado nesta LAN');
      }
      lines.push('');
      lines.push('▸ Permissões locais');
      lines.push(`  Firewall (Movex rules): ${r.firewall_rules_present ? '✅ presentes' : '❌ ausentes (Windows)'}`);
      lines.push(`  Acessibilidade macOS:    ${r.macos_accessibility ? '✅ trusted' : '❌ negada (macOS)'}`);
      lines.push('');
      lines.push('═══ Sugestões ═══');
      if (r.tcp_reachable === false) {
        lines.push('• TCP falhou → firewall do peer (Privada vs Pública), peer offline ou IP errado.');
      }
      if (r.mdns_peers.length === 0) {
        lines.push('• Sem mDNS → o outro PC não está na mesma LAN ou bloqueia UDP 5353.');
      }
      if (!r.firewall_rules_present) {
        lines.push('• Regras de firewall ausentes → use «Aplicar regras no firewall (admin)» (Windows).');
      }
      if (!r.macos_accessibility) {
        lines.push('• Sem Acessibilidade no macOS → cursor/teclado do peer não move este Mac.');
      }
      if (r.tcp_reachable === true && r.firewall_rules_present && r.macos_accessibility) {
        lines.push('• Tudo OK: clica «Conectar» no painel.');
      }
      if (out) out.textContent = lines.join('\n');
    } catch (e) {
      if (out) out.textContent = `Erro no diagnóstico: ${e}`;
    } finally {
      if (btn) btn.disabled = false;
    }
  };

  const clearServerCertTrust = async () => {
    if (!isTauri()) {
      addLog('Use a aplicação Movex instalada.', 'warn');
      return;
    }
    try {
      await invoke('clear_server_cert_trust');
      addLog('Certificado TLS do servidor esquecido. Tente Conectar de novo.', 'info');
      await refreshSettings();
    } catch (e) {
      addLog(`Erro: ${e}`, 'warn');
    }
  };

  try {
    const plat = await invoke<string>('get_platform_kind');
    const rowMac = document.getElementById('permRowMac');
    const rowWin = document.getElementById('permRowWin');
    if (rowMac && rowWin) {
      rowMac.style.display = plat === 'macos' ? 'flex' : 'none';
      rowWin.style.display = plat === 'windows' ? 'flex' : 'none';
      rowWin.style.flexDirection = 'column';
    }
    if (plat === 'macos') {
      // Verificar se acessibilidade está trusted e mostrar banner de aviso
      const updateAccessibilityBanner = async () => {
        try {
          const trusted = await invoke<boolean>('check_accessibility');
          const banner = document.getElementById('accessibilityWarningBanner');
          if (banner) banner.style.display = trusted ? 'none' : 'block';
        } catch { /* silenciar */ }
      };
      await updateAccessibilityBanner();
      // Re-verificar a cada 5s enquanto o app está aberto (detecta quando o utilizador concede)
      setInterval(updateAccessibilityBanner, 5000);
    }
  } catch {
    /* ignorar se o comando não existir */
  }

  const setManualIpDetailsOpen = (open: boolean) => {
    const el = document.getElementById('manualIpDetails') as HTMLDetailsElement | null;
    if (el) el.open = open;
  };

  /** Abre a aba Dispositivos e expande o bloco «Conectar por IP» (details nativo do browser). */
  const revealManualIpForm = () => {
    navTo('dispositivos');
    setManualIpDetailsOpen(true);
    setTimeout(() => {
      (document.getElementById('manualIp') as HTMLInputElement | null)?.focus();
    }, 200);
  };

  const handlePanelConnect = async () => {
    const lineEl = document.getElementById('panelConnStatus');
    if (!isTauri()) {
      const msg =
        'Abra o Movex pela aplicação instalada (janela nativa). No navegador (localhost) não há backend Tauri.';
      addLog(msg, 'warn');
      if (lineEl) {
        lineEl.textContent = msg;
        lineEl.style.color = 'var(--warn)';
      }
      return;
    }
    await refreshSettings();
    const s = cachedSettings as { role?: string; server_addr?: string | null } | null;
    const role = (s?.role ?? 'server').toLowerCase();
    const serverAddr = (s?.server_addr ?? '').trim();
    if (role === 'client' && !serverAddr) {
      addLog('Cliente: informe o IP do servidor (aba Dispositivos) ou em Configurações.', 'warn');
      revealManualIpForm();
      if (lineEl) {
        lineEl.textContent = 'Defina o IP do servidor (Dispositivos)';
        lineEl.style.color = 'var(--warn)';
      }
      return;
    }
    addLog('Iniciando conexão…', 'info');
    if (lineEl) {
      lineEl.textContent = 'A iniciar…';
      lineEl.style.color = 'var(--cyan)';
    }
    try {
      await invoke('start_connection');
    } catch (e: unknown) {
      addLog(`Erro: ${e}`, 'warn');
      if (lineEl) {
        lineEl.textContent = `Erro: ${e}`;
        lineEl.style.color = 'var(--warn)';
      }
      navTo('painel');
      return;
    }
    // O servidor arranca numa task; vários `get_status` até refletir Listening/Connecting.
    let st = normalizeStatusPayload(await invoke<unknown>('get_status'));
    for (let i = 0; i < 40 && !sessionLooksActive(st); i++) {
      await new Promise((r) => setTimeout(r, 100));
      st = normalizeStatusPayload(await invoke<unknown>('get_status'));
    }
    {
      const el = document.getElementById('panelConnStatus');
      if (el) {
        const t = (st.status_text ?? '').trim();
        el.textContent = t || (sessionLooksActive(st) ? 'A aguardar…' : 'Desconectado');
        const waiting = /aguardando|conectando|reconect/i.test(t);
        el.style.color = sessionLooksActive(st) || waiting ? 'var(--cyan)' : 'var(--text-2)';
      }
      applyPanelConnectionButtons(st);
      updateMatrixHint(cachedSettings ?? null, st, lastConnectedPeer);
    }
    navTo('painel');
  };
  document.getElementById('btnConnect')?.addEventListener('click', () => void handlePanelConnect());

  // Drag-and-drop, borda luminosa e status → módulos FileTransfer, ScreenBorder, ConnectionStatus

  // ── Verificar atualização ao iniciar ──────────────────────────────────────
  setTimeout(async () => {
    try {
      const version = await invoke<string | null>('check_for_update');
      if (version) {
        addLog(`Nova versão disponível: v${version}`, 'sec');
        showUpdateNotification(version);
      }
    } catch { /* atualizações indisponíveis */ }
  }, 5000);

  const showUpdateNotification = (version: string) => {
    // Usar createElement + textContent para evitar XSS com version do backend
    const banner = document.createElement('div');
    banner.id = 'updateBanner';
    banner.style.cssText = 'position:fixed;bottom:20px;left:50%;transform:translateX(-50%);background:var(--bg-3);border:1px solid var(--border-c);border-radius:12px;padding:14px 20px;display:flex;align-items:center;gap:16px;z-index:9997;box-shadow:0 4px 24px rgba(0,212,255,.15);';

    const info = document.createElement('div');
    const title = document.createElement('div');
    title.style.cssText = 'font-size:13px;font-weight:700;color:var(--text);';
    title.textContent = `🆕 Movex v${version} disponível`; // textContent — sem XSS
    const sub = document.createElement('div');
    sub.style.cssText = 'font-size:11px;color:var(--text-3);';
    sub.textContent = 'Clique para instalar e reiniciar';
    info.appendChild(title);
    info.appendChild(sub);

    const installBtn = document.createElement('button');
    installBtn.className = 'btn btn-cyan';
    installBtn.style.cssText = 'white-space:nowrap;font-size:11px;';
    installBtn.textContent = 'Instalar';
    installBtn.addEventListener('click', installUpdate);

    const closeBtn = document.createElement('button');
    closeBtn.style.cssText = 'background:none;border:none;color:var(--text-3);cursor:pointer;font-size:16px;';
    closeBtn.textContent = '✕';
    closeBtn.addEventListener('click', () => banner.remove());

    banner.appendChild(info);
    banner.appendChild(installBtn);
    banner.appendChild(closeBtn);
    document.body.appendChild(banner);
  };

  const installUpdate = async () => {
    addLog('Baixando e instalando atualização...', 'sec');
    document.getElementById('updateBanner')?.remove();
    const result = await invoke<string>('install_update')
      .catch((e: unknown) => { addLog(`Erro na atualização: ${e}`, 'warn'); return null; });
    if (result) addLog(`✓ ${result}`, 'sec');
  };

  // ── Transferência de arquivos ───────────────────────────────────────────────
  const sendFileDialog = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        title: 'Selecionar arquivo para enviar ao peer',
      });
      if (!selected) return;
      const path = typeof selected === 'string' ? selected : (selected as any).path ?? selected;
      addLog(`Enviando arquivo: ${path}...`, 'info');
      await invoke('send_file_to_peer', { path });
      addLog('Transferência iniciada.', 'sec');
    } catch (e) {
      addLog(`Erro ao enviar arquivo: ${e}`, 'warn');
    }
  };

  const doDisconnect = async () => {
    await invoke('disconnect').catch(console.warn);
    addLog('Desconectado.', 'info');
  };

  // ── Posicionamento do peer no mapa de telas ────────────────────────────────
  const setPeerPosition = async (position: string) => {
    if (!cachedSettings) {
      addLog('Configurações ainda carregando — tente novamente em instantes.', 'warn');
      return;
    }
    await invoke('save_settings', {
      hostname: cachedSettings.hostname,
      screenName: cachedSettings.screen_name ?? cachedSettings.hostname,
      expectedClientScreenName: cachedSettings.expected_client_screen_name ?? null,
      launchConnectionOnStartup: !!cachedSettings.launch_connection_on_startup,
      role: cachedSettings.role,
      serverAddr: cachedSettings.server_addr ?? null,
      port: cachedSettings.port,
      pskHex: cachedSettings.psk_hex, // nunca vazio — PSK preservada
      peerPosition: position,
      autostart: cachedSettings.autostart ?? false,
      theme: cachedSettings.theme ?? 'dark',
    }).catch(console.warn);
    await refreshSettings();
    addLog(`Posição do monitor remoto: ${position}`, 'info');
    // Atualizar visual dos botões
    ['above','below','left','right'].forEach(p => {
      const btn = document.getElementById(`pos-${p}`);
      if (btn) {
        btn.className = p === position ? 'btn btn-cyan' : 'btn btn-outline';
        (btn as HTMLButtonElement).style.cssText = 'padding:6px;font-size:14px;';
      }
    });
  };

  // Mostrar seletor de posição quando conectado
  const posSelector = document.getElementById('peerPositionSelector');
  if (posSelector && cachedSettings) {
    // Marcar posição atual
    const currentPos = cachedSettings.peer_position ?? 'right';
    ['above','below','left','right'].forEach(p => {
      const btn = document.getElementById(`pos-${p}`);
      if (btn) btn.className = p === currentPos ? 'btn btn-cyan' : 'btn btn-outline';
    });
  }

  // ── Tema claro/escuro ──────────────────────────────────────────────────────
  const setTheme = async (theme: string) => {
    const { applyTheme } = await import('../main');
    applyTheme(theme);
    const dark = document.getElementById('themeDark')!;
    const light = document.getElementById('themeLight')!;
    dark.style.borderColor  = theme === 'dark'  ? 'var(--cyan)' : 'var(--border)';
    dark.style.color        = theme === 'dark'  ? 'var(--text)' : 'var(--text-2)';
    light.style.borderColor = theme === 'light' ? 'var(--cyan)' : 'var(--border)';
    light.style.color       = theme === 'light' ? 'var(--text)' : 'var(--text-2)';
    await invoke('update_preferences', {
      notificationsEnabled: (document.getElementById('notifToggle') as HTMLInputElement)?.checked ?? true,
      lockKey:
        (document.getElementById('lockKeyInput') as HTMLInputElement)?.value?.trim() || 'ctrl+alt+l',
      clipboardSyncEnabled: (document.getElementById('clipboardToggle') as HTMLInputElement)?.checked ?? true,
      theme,
    }).catch(console.warn);
    addLog(`Tema alterado: ${theme}`, 'info');
  };

  // ── Modo Lock ──────────────────────────────────────────────────────────────
  const toggleLockMode = async () => {
    const active = await invoke<boolean>('toggle_lock').catch(() => false);
    const btn = document.getElementById('btnLockMode')!;
    btn.textContent = active ? '🔒 Bloqueado' : '🔓 Desbloqueado';
    btn.style.background = active ? 'rgba(245,166,35,.15)' : '';
    btn.style.borderColor = active ? 'var(--warn)' : '';
    btn.style.color = active ? 'var(--warn)' : '';
    addLog(`Modo lock: ${active ? 'ATIVO' : 'inativo'}`, active ? 'warn' : 'info');
  };

  // ── Histórico de conexões ──────────────────────────────────────────────────
  const loadRecentPeers = async () => {
    try {
      const peers = await invoke<any[]>('get_recent_peers');
      const list = document.getElementById('recentPeersList')!;
      if (!peers || peers.length === 0) {
        list.innerHTML = '<div style="font-size:11px;color:var(--text-3);">Nenhuma conexão ainda</div>';
        return;
      }
      // Usar createElement + textContent para evitar XSS com dados de rede (hostname, addr)
      list.innerHTML = '';
      peers.forEach(p => {
        const date = new Date(p.last_connected * 1000).toLocaleString('pt-BR', {day:'2-digit',month:'2-digit',hour:'2-digit',minute:'2-digit'});

        const row = document.createElement('div');
        row.style.cssText = 'display:flex;align-items:center;justify-content:space-between;padding:8px 12px;background:var(--bg-2);border-radius:8px;cursor:pointer;border:1px solid var(--border);transition:border-color .15s;';
        row.addEventListener('mouseover', () => { row.style.borderColor = 'var(--border-c)'; });
        row.addEventListener('mouseout', () => { row.style.borderColor = 'var(--border)'; });
        // Capturar addr e port sem interpolação de string em onclick
        const addr = p.addr as string;
        const port = Number(p.port);
        row.addEventListener('click', () => connectToPeer(addr, port));

        const left = document.createElement('div');
        const nameEl = document.createElement('div');
        nameEl.style.cssText = 'font-size:13px;font-weight:600;color:var(--text);';
        nameEl.textContent = p.hostname; // textContent — sem XSS
        const ipEl = document.createElement('div');
        ipEl.style.cssText = 'font-size:10px;color:var(--text-3);';
        ipEl.textContent = `${p.addr}:${p.port}`; // textContent — sem XSS
        left.appendChild(nameEl);
        left.appendChild(ipEl);

        const right = document.createElement('div');
        right.style.cssText = 'text-align:right;';
        const dateEl = document.createElement('div');
        dateEl.style.cssText = 'font-size:10px;color:var(--text-3);';
        dateEl.textContent = date;
        const connectEl = document.createElement('div');
        connectEl.style.cssText = 'font-size:10px;color:var(--cyan);margin-top:2px;';
        connectEl.textContent = 'Conectar →';
        right.appendChild(dateEl);
        right.appendChild(connectEl);

        row.appendChild(left);
        row.appendChild(right);
        list.appendChild(row);
      });
    } catch { /* fora do Tauri */ }
  };

  const clearHistory = async () => {
    await invoke('clear_recent_peers').catch(console.warn);
    loadRecentPeers();
    addLog('Histórico de conexões limpo.', 'info');
  };


  // ── Descoberta mDNS ────────────────────────────────────────────────────────
  const refreshDevices = async () => {
    const btn = document.getElementById('btnRefreshDevices') as HTMLButtonElement;
    const subtitle = document.getElementById('deviceSubtitle')!;
    if (btn) { btn.disabled = true; btn.textContent = '⟳ Buscando...'; }
    subtitle.textContent = 'Buscando na rede local (3s)...';
    addLog("Buscando dispositivos Movex na rede...", "info");

    try {
      const ips = await invoke<string[]>('get_local_ipv4_addrs');
      movexLocalIpv4Cache = ips?.length ? ips.join(' · ') : '';
    } catch {
      /* ignora */
    }

    try {
      const peers = await invoke<PeerInfo[]>('discover_peers');
      subtitle.textContent = `${peers.length} dispositivo(s) encontrado(s)`;
      addLog(`Descoberta concluída: ${peers.length} peer(s)`, peers.length > 0 ? 'sec' : 'info');
      renderDiscoveredDevices(peers, currentRole);
      try {
        const raw = await invoke<unknown>('get_status');
        updateDevices(normalizeStatusPayload(raw), cachedSettings ?? null);
      } catch {
        /* ignora */
      }
    } catch (e) {
      subtitle.textContent = 'Erro na descoberta';
      addLog(`Erro na descoberta mDNS: ${e}`, 'warn');
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = '↻ Atualizar'; }
    }
  };

  const openManualIpFromToolbar = () => {
    revealManualIpForm();
    addLog('Conectar por IP: use o endereço do PC em Servidor (à escuta). O Cliente não aceita conexões nesta porta.', 'info');
  };

  const goToPainel = () => navTo('painel');

  const connectManual = async () => {
    const ip   = (document.getElementById('manualIp') as HTMLInputElement).value.trim();
    const port = parseInt((document.getElementById('manualPort') as HTMLInputElement).value) || 24800;
    if (!ip) { addLog("Digite um endereço IP", 'warn'); return; }
    addLog(`Conectando a ${ip}:${port}...`, 'info');
    setManualIpDetailsOpen(false);
    await invoke('connect_to_peer', { addr: ip, port }).catch((e: unknown) => addLog(`Erro: ${e}`, 'warn'));
    goToPainel();
  };

  const connectToPeer = async (addr: string, port: number) => {
    addLog(`Conectando a ${addr}:${port}...`, 'info');
    await invoke('connect_to_peer', { addr, port }).catch((e: unknown) => addLog(`Erro: ${e}`, 'warn'));
    goToPainel();
  };

  /** Fluxo «Adicionar Máquina»: Dispositivos + formulário IP + busca na rede. */
  const addNewMachine = () => {
    revealManualIpForm();
    void refreshDevices();
    addLog('Adicionar máquina: use o IP abaixo ou um computador listado após «Atualizar».', 'info');
    setTimeout(() => {
      (document.getElementById('manualIp') as HTMLInputElement | null)?.focus();
    }, 200);
  };

  // Busca inicial na rede (não depende só do poll de status)
  setTimeout(() => {
    void refreshDevices();
  }, 1200);

  document.getElementById('cmdInput')?.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter') return;
    const input = e.target as HTMLInputElement;
    const cmd = input.value.trim();
    if (!cmd) return;
    addLog(`$ ${cmd}`, 'info');
    addLog(simulateCmd(cmd), 'sec');
    input.value = '';
  });

  // ── Conectar TODOS os botões via addEventListener (referência direta) ────────
  const on = (id: string, fn: () => void) =>
    document.getElementById(id)?.addEventListener('click', fn);

  on('btnSendFile',        sendFileDialog);
  on('btnDisconnect',      doDisconnect);
  // btnConnect: só handlePanelConnect (acima) — não usar addManualDevice aqui
  on('pos-above',          () => setPeerPosition('above'));
  on('pos-left',           () => setPeerPosition('left'));
  on('pos-right',          () => setPeerPosition('right'));
  on('pos-below',          () => setPeerPosition('below'));
  on('btnRefreshDevices',  refreshDevices);
  on('btnAddManual',       openManualIpFromToolbar);
  on('btnConnectManual',   connectManual);
  on('btnCloseManual',     () => setManualIpDetailsOpen(false));
  on('btnCopyLogs',        copyLogs);
  on('btnClearLogs',       () => clearLogs());
  on('btnApplyServerAddr', applyServerAddr);
  on('btnToggleKey',       toggleKey);
  on('themeDark',          () => setTheme('dark'));
  on('themeLight',         () => setTheme('light'));
  on('btnLockMode',        toggleLockMode);
  on('btnClearHistory',    clearHistory);
  on('btnConfirmReset',    confirmReset);
  on('btnCancelReset',     closeResetModal);
  on('btnDoReset',         doReset);
  on('btnDiscardSettings', () => void discardSettings());
  on('btnSaveConfig',      saveConfigAndRefresh);
  on('btnPermMacAccessibility',  () => void openSystemPanel('accessibility'));
  on('btnPermMacAccessibility2', () => void openSystemPanel('accessibility'));
  on('btnPermMacInput',          () => void openSystemPanel('input_monitoring'));
  on('btnPermMacNotif',          () => void openSystemPanel('notifications'));
  on('btnFixAccessibility', async () => {
    addLog('A corrigir permissão de Acessibilidade (reset TCC + novo prompt)…', 'info');
    try {
      await invoke('fix_macos_accessibility');
      addLog('Prompt de Acessibilidade enviado. Se o diálogo não aparecer, abra Ajustes → Privacidade → Acessibilidade, remova Movex e reative.', 'info');
    } catch (e) {
      addLog(`Erro ao corrigir acessibilidade: ${e}`, 'warn');
    }
  });
  on('btnPermWinApplyFw',  () => void applyWindowsFirewallRules());
  on('btnClearServerCert', () => void clearServerCertTrust());
  on('btnPermWinFirewallAdv', () => void openSystemPanel('firewall_advanced'));
  on('btnPermWinProxy',    () => void openSystemPanel('proxy'));
  on('btnPermWinNotif',    () => void openSystemPanel('notifications'));
  on('btnPermWinFirewall', () => void openSystemPanel('firewall'));
  on('btnPermWinPrivacy',  () => void openSystemPanel('privacy'));
  on('btnDiagnose',        () => void runDiagnose());
  on('btnAddMachine',      addNewMachine);
  on('btnDiagReport',      () => addLog('Relatório completo: em desenvolvimento (use os logs abaixo para diagnóstico).', 'info'));
  on('btnDiagRestart',     () => addLog('Reinício de diagnósticos: apenas registo no log (sem ação no sistema).', 'info'));

  // Cards de papel (servidor/cliente) — usar referência direta
  document.querySelectorAll('[data-role]').forEach(el => {
    el.addEventListener('click', () => selectRoleCard((el as HTMLElement).dataset.role!));
  });

  // Atualizar referência global para loadRecentPeers (usado em renderDeviceCards)
  (window as any).connectToPeer = connectToPeer;

  void (async () => {
    try {
      const raw = await invoke<unknown>('get_status');
      const st = normalizeStatusPayload(raw);
      if (!cachedSettings) await refreshSettings();
      updateDevices(st, cachedSettings ?? null);
    } catch (e) {
      console.warn('[Movex] sync inicial dispositivos:', e);
    }
  })();
}

// updateStatus foi substituído por onStatusChange no módulo ConnectionStatus

function updateScreenMap(settings: SettingsPayload, status: StatusPayload) {
  const map = document.getElementById('screenMap');
  if (!map) return;
  const isActive = status.active_screen === 'Local';

  // Usar createElement + textContent para evitar XSS (hostname/peer_hostname vêm da rede)
  map.innerHTML = '';

  const makeCard = (isLocal: boolean, hostname: string, active: boolean, remoteLine: string) => {
    const card = document.createElement('div');
    card.draggable = true;
    card.style.cssText = `background:${active?'linear-gradient(160deg,#0f1824,#0d1520)':'var(--bg-4)'};border:1.5px solid ${active?'var(--cyan)':'var(--border)'};border-radius:12px;padding:18px 20px 14px;width:148px;text-align:center;cursor:grab;${active?'box-shadow:0 0 20px rgba(0,212,255,.12);':''}`;
    const icon = document.createElement('div');
    icon.style.cssText = 'font-size:28px;margin-bottom:8px;';
    icon.textContent = '🖥️';
    const name = document.createElement('div');
    name.style.cssText = 'font-size:11px;font-weight:700;color:var(--text);letter-spacing:.5px;text-transform:uppercase;';
    name.textContent = hostname; // textContent — sem XSS
    const statusEl = document.createElement('div');
    statusEl.style.cssText = `font-size:10px;color:${active?'var(--cyan)':'var(--text-3)'};margin-top:3px;`;
    statusEl.textContent = isLocal ? (active ? '● ATIVO' : 'em espera') : remoteLine;
    card.appendChild(icon);
    card.appendChild(name);
    card.appendChild(statusEl);
    return card;
  };

  const remoteLine = status.connected
    ? '● LIGADO AO PAR'
    : status.in_session
      ? '◐ sessão (à escuta / a ligar…)'
      : 'desconectado';

  const localLabel = (settings.screen_name?.trim() || settings.hostname).trim();
  map.appendChild(makeCard(true, localLabel, isActive, ''));
  const arrow = document.createElement('div');
  arrow.style.cssText = 'color:var(--text-3);font-size:20px;';
  arrow.textContent = '⇄';
  map.appendChild(arrow);
  map.appendChild(makeCard(false, status.peer_hostname ?? 'Outro ecrã', !isActive && status.connected, remoteLine));
}

function updateDevices(status: StatusPayload, settings: SettingsPayload | null) {
  const grid = document.getElementById('deviceGrid');
  if (!grid) return;

  const hostname =
    settings?.screen_name?.trim() || settings?.hostname?.trim() || 'Este computador';
  const port = settings && typeof (settings as { port?: number }).port === 'number'
    ? (settings as { port: number }).port
    : 24800;
  const localIpText = formatLocalNetworkLine(port, movexLocalIpv4Cache);

  // Se conectado: sempre mostrar as máquinas reais (ignora descoberta anterior)
  if (status.connected) {
    grid.dataset.discovered = 'false'; // permite atualizar
  const devices = [
      { name: hostname, ip: localIpText, icon: '🖥️', online: true, addr: null as string|null, port: 0 },
      {
        name: status.peer_hostname ?? 'Peer',
        ip: status.peer_addr?.trim() ? `● ${status.peer_addr}` : '● Conectado agora',
        icon: '💻',
        online: true,
        addr: null as string|null,
        port: 0,
      },
    ];
    grid.innerHTML = '';
    renderDeviceCards(grid, devices);
    const subtitle = document.getElementById('deviceSubtitle');
    if (subtitle) subtitle.textContent = `Conectado a ${status.peer_hostname ?? 'peer'}`;
    return;
  }

  // Se não conectado e grid já foi preenchida pela descoberta mDNS, não sobrescrever
  if (grid.dataset.discovered === 'true') return;

  const role = settings?.role ?? 'server';

  // Mostrar sempre a máquina local (mesmo sem get_settings ainda)
  const devices = [
    { name: hostname, ip: localIpText, icon: '🖥️', online: true, addr: null as string|null, port: 0 },
  ];
  grid.innerHTML = '';
  renderDeviceCards(grid, devices);
  const subtitle = document.getElementById('deviceSubtitle');
  if (subtitle) {
    if (role === 'client') {
      subtitle.textContent = `${hostname} · ${localIpText} · Papel Cliente: use o IP do anfitrião em Configurações · «Atualizar» para descoberta na LAN`;
    } else {
      subtitle.textContent = `${hostname} · ${localIpText} · «Atualizar» para ver PCs em Servidor à escuta na rede`;
    }
  }
}

function renderDiscoveredDevices(peers: PeerInfo[], role: string = 'server') {
  const grid = document.getElementById('deviceGrid');
  if (!grid) return;
  // Só marcar como descoberto se encontrou peers — senão deixa o updateDevices atualizar
  grid.dataset.discovered = peers.length > 0 ? 'true' : 'false';

  if (peers.length === 0) {
    const emptyClient =
      'A descoberta lista anfitriões (papel Servidor) à escuta. Se continuar vazio, confira o IP do servidor em Configurações → «Endereço do Servidor» e use Conectar no painel — o destino é sempre o PC em Servidor, não este.';
    const emptyServer =
      'Só aparecem PCs em papel Servidor com Conectar ativo. No computador que recebe o controlo, use papel Cliente e o IP da máquina que está em Servidor — não tente ligar ao IP do Cliente.';
    grid.innerHTML = `
      <div style="grid-column:1/-1;text-align:center;padding:40px;color:var(--text-3);">
        <div style="font-size:32px;margin-bottom:12px;">🔍</div>
        <div style="font-size:14px;font-weight:600;margin-bottom:6px;">Nenhum Movex à escuta na rede</div>
        <div style="font-size:12px;">${role === 'client' ? emptyClient : emptyServer}</div>
      </div>`;
    return;
  }

  const devices = peers.map(p => ({
    name: p.hostname,
    ip: `${p.addr}:${p.port}`,
    icon: '🖥️',
    online: true,
    addr: p.addr,
    port: p.port,
  }));

  grid.innerHTML = '';
  renderDeviceCards(grid, devices);
}

function renderDeviceCards(
  container: HTMLElement,
  devices: { name: string; ip: string; icon: string; online: boolean; addr: string | null; port: number }[]
): void {
  devices.forEach(d => {
    const card = document.createElement('div');
    card.style.cssText = [
      'background:var(--bg-3)',
      `border:1px solid ${d.online ? 'rgba(0,212,255,.2)' : 'var(--border)'}`,
      'border-radius:14px',
      'padding:20px',
      d.addr ? 'cursor:pointer' : '',
      'transition:all .2s',
    ].filter(Boolean).join(';');

    // Ícone
    const iconEl = document.createElement('div');
    iconEl.style.cssText = 'width:44px;height:44px;background:var(--bg-4);border-radius:11px;display:flex;align-items:center;justify-content:center;margin-bottom:14px;font-size:22px;';
    iconEl.textContent = d.icon;
    card.appendChild(iconEl);

    // Nome
    const nameEl = document.createElement('div');
    nameEl.style.cssText = 'font-size:14px;font-weight:700;color:var(--text);margin-bottom:4px;';
    nameEl.textContent = d.name;
    card.appendChild(nameEl);

    // IP
    const ipEl = document.createElement('div');
    ipEl.style.cssText = 'font-size:11px;color:var(--text-3);margin-bottom:10px;';
    ipEl.textContent = d.ip;
    card.appendChild(ipEl);

    // Rodapé: badge online/offline + link conectar
    const footer = document.createElement('div');
    footer.style.cssText = 'display:flex;align-items:center;justify-content:space-between;';

    const badge = document.createElement('span');
    badge.style.cssText = [
      'display:inline-flex;align-items:center;gap:5px;padding:3px 9px;border-radius:20px',
      'font-size:10px;font-weight:600',
      `background:${d.online ? 'rgba(0,212,255,.12)' : 'rgba(255,75,110,.1)'}`,
      `color:${d.online ? 'var(--cyan)' : 'var(--danger)'}`,
    ].join(';');
    const dot = document.createElement('span');
    dot.style.cssText = 'width:5px;height:5px;border-radius:50%;background:currentColor;';
    badge.appendChild(dot);
    badge.appendChild(document.createTextNode(d.online ? ' Online' : ' Offline'));
    footer.appendChild(badge);

    if (d.addr) {
      const link = document.createElement('span');
      link.style.cssText = 'font-size:10px;color:var(--cyan);font-weight:600;';
      link.textContent = 'Conectar →';
      footer.appendChild(link);
    }
    card.appendChild(footer);

    // Registrar click via addEventListener (sem onclick inline — previne XSS)
    if (d.addr) {
      const addr = d.addr;
      const port = d.port;
      card.addEventListener('click', () => {
        // connectToPeer exposto via window para acesso de funções fora do escopo
        (window as any).connectToPeer?.(addr, port);
      });
    }

    container.appendChild(card);
  });
}

function simulateCmd(cmd: string): string {
  if (cmd === 'clear') { clearLogs(); return ''; }
  if (cmd === 'help') return 'Comandos disponíveis: clear, status';
  if (cmd === 'status') return (document.getElementById('pageTitle')?.textContent ?? '') + ' · rodando';
  return `Comando '${cmd}' não reconhecido. Digite 'help'.`;
}
