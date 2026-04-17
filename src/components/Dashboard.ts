import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { addLog, clearLogs } from "./Logs";
import { initScreenBorder } from "./ScreenBorder";
import { initFileTransfer, cleanupFileTransfer } from "./FileTransfer";
import { initStatusListener, onStatusChange, startApprovalPolling, cleanupStatusHandlers } from "./ConnectionStatus";
import { cleanupAllListeners } from "../utils/tauri-events";

function esc(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

interface StatusPayload {
  connected: boolean;
  status_text: string;
  peer_hostname?: string;
  latency_ms?: number;
  active_screen: string;
  uptime_secs: number;
}

interface PeerInfo {
  hostname: string;
  addr: string;
  port: number;
}

interface SettingsPayload {
  hostname: string;
  role: string;
  peer_position: string;
  setup_complete: boolean;
}

export async function renderDashboard(): Promise<void> {
  const app = document.getElementById("app")!;

  // Ler versão real do tauri.conf.json em tempo de execução
  const appVersion = await getVersion().catch(() => "0.1.0");

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
          <button class="btn-add">
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
                  <span style="font-size:13px;color:var(--text-2);" id="nodesLabel">3 Nós Ativos Conectados</span>
                </div>
              </div>
              <div style="display:flex;flex-direction:column;gap:12px;">
                <div class="card">
                  <div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:10px;display:flex;justify-content:space-between;">Rede <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--cyan)" stroke-width="1.8"><path d="M5 12.55a11 11 0 0114.08 0"/><circle cx="12" cy="20" r="1" fill="var(--cyan)"/></svg></div>
                  <div style="font-size:30px;font-weight:700;color:var(--text);line-height:1;letter-spacing:-1px;">1.2<span style="font-size:14px;color:var(--text-2);margin-left:4px;">Gbps</span></div>
                  <div style="font-size:11px;color:var(--text-3);margin-top:6px;">Vazão contínua</div>
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
                </div>
                <div style="display:flex;gap:10px;">
                  <button class="btn btn-outline" id="btnSendFile">📁 Enviar Arquivo</button>
                  <button class="btn btn-outline" id="btnDisconnect" style="display:none;">Desconectar</button>
                  <button class="btn btn-cyan" id="btnConnect">Conectar</button>
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
                <div class="section-sub" id="deviceSubtitle">Buscando na rede local...</div>
              </div>
              <div style="display:flex;gap:8px;">
                <button class="btn btn-outline" id="btnRefreshDevices">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.48"/></svg>
                  Atualizar
                </button>
                <button class="btn btn-cyan" id="btnAddManual">+ Manual</button>
              </div>
            </div>

            <!-- Modal IP manual -->
            <div id="manualModal" style="display:none;background:var(--bg-3);border:1px solid var(--border-c);border-radius:12px;padding:20px;margin-bottom:16px;">
              <div style="font-size:13px;font-weight:600;color:var(--text);margin-bottom:10px;">Conectar por IP</div>
              <div style="display:flex;gap:8px;">
                <input id="manualIp" type="text" placeholder="192.168.1.100" style="flex:1;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                <input id="manualPort" type="number" value="24800" style="width:90px;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
                <button class="btn btn-cyan" id="btnConnectManual">Conectar</button>
                <button class="btn btn-outline" id="btnCloseManual">✕</button>
              </div>
            </div>

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
              <button class="btn btn-outline">Gerar Relatório Completo</button>
              <button class="btn btn-cyan">Reiniciar Diagnósticos</button>
            </div>
          </div>

          <!-- CONFIGURAÇÕES -->
          <div class="page" id="page-configuracoes">

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
                  <span style="font-size:13px;font-weight:600;color:var(--text);">Chave de Segurança</span>
                </div>
                <div style="position:relative;">
                  <input type="password" id="keyInput" style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:10px 40px 10px 14px;font-family:'JetBrains Mono',monospace;font-size:13px;font-weight:600;color:var(--text);outline:none;letter-spacing:2px;" />
                  <button id="btnToggleKey" style="position:absolute;right:10px;top:50%;transform:translateY(-50%);background:none;border:none;cursor:pointer;color:var(--text-3);font-size:14px;">👁</button>
                </div>
                <div style="font-size:10px;color:var(--text-3);margin-top:6px;">Use a mesma chave nos dois computadores</div>
              </div>
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
                <div style="font-size:11px;color:var(--text-2);line-height:1.5;">Copie em um computador e cole no outro.</div>
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

  // ── Modal de aprovação de conexão (overlay global) ───────────────────────
  const approvalModalHtml = `
    <div id="approvalOverlay" style="
      display:none;
      position:fixed;inset:0;z-index:9999;
      background:rgba(0,0,0,.75);backdrop-filter:blur(4px);
      align-items:center;justify-content:center;
    ">
      <div style="
        background:var(--bg-3);
        border:1px solid var(--border-c);
        border-radius:20px;
        padding:32px 28px;
        width:400px;
        text-align:center;
        box-shadow:0 0 40px rgba(0,212,255,.15);
      ">
        <div style="font-size:48px;margin-bottom:16px;">🖥️</div>
        <div style="font-size:18px;font-weight:800;color:var(--text);margin-bottom:6px;">
          Solicitação de Conexão
        </div>
        <div style="font-size:14px;color:var(--text-2);margin-bottom:6px;">
          O computador
        </div>
        <div id="approvalHostname" style="
          font-size:22px;font-weight:700;color:var(--cyan);
          background:var(--cyan-dim);border:1px solid var(--border-c);
          border-radius:10px;padding:10px 18px;
          margin-bottom:12px;letter-spacing:.5px;
        ">—</div>
        <div style="font-size:13px;color:var(--text-2);margin-bottom:28px;line-height:1.5;">
          quer controlar seu teclado e mouse.<br>
          <span style="color:var(--text-3);font-size:11px;">Você terá controle total para desconectar a qualquer momento.</span>
        </div>
        <div style="display:flex;gap:12px;justify-content:center;">
          <button id="btnRejectConn" style="
            flex:1;padding:13px;border-radius:10px;
            background:rgba(255,75,110,.12);border:1px solid rgba(255,75,110,.3);
            color:var(--danger,#ff4b6e);font-family:'Inter',sans-serif;
            font-size:14px;font-weight:700;cursor:pointer;
            transition:all .15s;
          ">✕ Recusar</button>
          <button id="btnApproveConn" style="
            flex:1;padding:13px;border-radius:10px;
            background:var(--cyan);border:none;
            color:#0b0c10;font-family:'Inter',sans-serif;
            font-size:14px;font-weight:700;cursor:pointer;
            transition:filter .15s;
          ">✓ Permitir</button>
        </div>
        <div id="approvalCountdown" style="margin-top:16px;font-size:11px;color:var(--text-3);">
          Recusa automática em 60s
        </div>
      </div>
    </div>
  `;
  document.body.insertAdjacentHTML('beforeend', approvalModalHtml);

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
      updateLockButton?.();
      updateThemeButtons?.();
    }
  };

  // Navegação da sidebar
  document.querySelectorAll('.nav-item[data-nav]').forEach(el => {
    el.addEventListener('click', () => navTo((el as HTMLElement).dataset.nav!));
  });

  let approvalCountdownTimer: ReturnType<typeof setInterval> | null = null;
  let approvalSeconds = 60;

  const showApprovalModal = (hostname: string) => {
    const overlay = document.getElementById('approvalOverlay')!;
    const hostnameEl = document.getElementById('approvalHostname')!;
    const countdown = document.getElementById('approvalCountdown')!;
    hostnameEl.textContent = hostname;
    overlay.style.display = 'flex';
    approvalSeconds = 60;
    countdown.textContent = `Recusa automática em ${approvalSeconds}s`;
    if (approvalCountdownTimer) clearInterval(approvalCountdownTimer);
    approvalCountdownTimer = setInterval(async () => {
      approvalSeconds--;
      countdown.textContent = `Recusa automática em ${approvalSeconds}s`;
      if (approvalSeconds <= 0) {
        clearInterval(approvalCountdownTimer!);
        approvalCountdownTimer = null;
        hideApprovalModal();
        // Rejeitar no backend quando o countdown esgota
        await invoke('reject_connection').catch(() => {});
        addLog('Conexão recusada automaticamente (timeout)', 'warn');
      }
    }, 1000);
  };

  const hideApprovalModal = () => {
    const overlay = document.getElementById('approvalOverlay');
    if (overlay) overlay.style.display = 'none';
    if (approvalCountdownTimer) { clearInterval(approvalCountdownTimer); approvalCountdownTimer = null; }
  };

  const approveConn = async () => {
    hideApprovalModal();
    await invoke('approve_connection').catch(console.warn);
    addLog('Conexão aprovada ✓', 'sec');
  };

  const rejectConn = async () => {
    hideApprovalModal();
    await invoke('reject_connection').catch(console.warn);
    addLog('Conexão recusada ✕', 'warn');
  };

  // Polling de aprovação movido para ConnectionStatus.startApprovalPolling — chamado abaixo

  // ── Cache de settings (raramente muda — evitar invoke por evento de status) ──
  let cachedSettings: any = null;
  const refreshSettings = async () => {
    try { cachedSettings = await invoke<any>('get_settings'); } catch { /* fora do Tauri */ }
  };
  await refreshSettings();

  // ── Inicializar módulos com cleanup ────────────────────────────────────────
  await initScreenBorder();
  await initFileTransfer();
  const stopStatusPolling   = await initStatusListener();
  const stopApprovalPolling = startApprovalPolling(
    (hostname) => showApprovalModal(hostname),
    ()         => hideApprovalModal(),
  );

  // Expor cleanup global (usado no reset de configurações)
  (window as any).__movexCleanup = () => {
    stopStatusPolling();
    stopApprovalPolling();
    cleanupFileTransfer();
    cleanupAllListeners();
    cleanupStatusHandlers();
    // Limpar countdown de aprovação se modal ainda estiver aberto
    if (approvalCountdownTimer) {
      clearInterval(approvalCountdownTimer);
      approvalCountdownTimer = null;
    }
  };

  // Delegar status updates para o módulo ConnectionStatus
  onStatusChange(async (status) => {
    const settings = cachedSettings;
    if (!settings) return; // ainda carregando
    try {
      updateScreenMap(settings, status);
      const latEl = document.getElementById('latencyVal');
      if (latEl) {
        if (status.connected && status.latency_ms != null) {
          latEl.innerHTML = `${status.latency_ms}<span style="font-size:14px;color:var(--text-2);margin-left:4px;">ms</span>`;
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
      // Botões conectar/desconectar
      const btnConnect = document.getElementById('btnConnect') as HTMLButtonElement;
      const btnDisconnect = document.getElementById('btnDisconnect') as HTMLButtonElement;
      if (btnConnect && btnDisconnect) {
        btnConnect.style.display = status.connected ? 'none' : 'inline-flex';
        btnDisconnect.style.display = status.connected ? 'inline-flex' : 'none';
      }
      updateDevices(status, settings);
      // Mostrar seletor de posição quando conectado
      const posSelector = document.getElementById('peerPositionSelector');
      if (posSelector) posSelector.style.display = status.connected ? 'block' : 'none';
      // Borda luminosa — cliente ativo
      const isClient = settings.role === 'client';
      const isRemoteActive = status.active_screen === 'Remote';
      invoke('set_screen_border', { active: isClient && isRemoteActive && status.connected, color: '#00d4ff' }).catch(() => {});
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
        if (nodesEl && status.connected) nodesEl.textContent = `2 Nós Conectados · ${bytes(total)} transferidos`;
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
    } catch { /* fora do Tauri */ }
  });

  // wrapper de cache será instalado após saveConfig ser definido

  addLog("Movex iniciado.", "info");
  addLog("Aguardando conexões na porta 24800.", "info");

  const copyLogs = () => navigator.clipboard.writeText(document.getElementById('logBody')?.innerText ?? '');
  const toggleKey = () => {
    const inp = document.getElementById('keyInput') as HTMLInputElement;
    if (inp) inp.type = inp.type === 'password' ? 'text' : 'password';
  };
  // ── Configurações ──────────────────────────────────────────────────────────

  let currentRole = 'server';

  const loadCurrentSettings = async () => {
    try {
      const s = await invoke<any>('get_settings');
      currentRole = s.role ?? 'server';
      applyRoleUI(currentRole);

      const addrInput = document.getElementById('serverAddrInput') as HTMLInputElement;
      if (addrInput && s.server_addr) addrInput.value = s.server_addr;

      const keyInput = document.getElementById('keyInput') as HTMLInputElement;
      if (keyInput && s.psk_hex) {
        // Mostrar PSK formatada em grupos de 4
        keyInput.value = s.psk_hex;
      }

      const portInput = document.getElementById('portInput') as HTMLInputElement;
      if (portInput) portInput.value = String(s.port ?? 24800);
    } catch { /* fora do Tauri */ }
  };

  const applyRoleUI = (role: string) => {
    const serverCard = document.getElementById('roleServerCard');
    const clientCard = document.getElementById('roleClientCard');
    const addrSection = document.getElementById('serverAddrSection');
    if (!serverCard || !clientCard || !addrSection) return;

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

  const saveConfig = async () => {
    const port = parseInt((document.getElementById('portInput') as HTMLInputElement).value) || 24800;
    // Ler chave do campo da UI (usuário pode ter alterado)
    const keyVal = (document.getElementById('keyInput') as HTMLInputElement)?.value.trim();
    try {
      const s = await invoke<any>('get_settings');
      await invoke('save_settings', {
        hostname: s.hostname,
        role: currentRole,
        serverAddr: (document.getElementById('serverAddrInput') as HTMLInputElement)?.value.trim() || null,
        port,
        pskHex: keyVal || s.psk_hex, // usar valor digitado se não estiver vazio
        peerPosition: s.peer_position ?? 'right',
        autostart: s.autostart ?? false,
        theme: s.theme ?? 'dark',
      });
      addLog("Configurações salvas com sucesso.", "sec");
    } catch(e) {
      addLog(`Erro ao salvar: ${e}`, 'warn');
    }
  };
  const saveConfigAndRefresh = async () => {
    await saveConfig();
    await refreshSettings();
  };

  // Carregar configurações atuais ao abrir a página
  await loadCurrentSettings();

  document.getElementById('btnConnect')?.addEventListener('click', async () => {
    addLog("Iniciando conexão...", "info");
    await invoke('start_connection').catch((e: unknown) => addLog(`Erro: ${e}`, 'warn'));
    navTo('painel');
  });

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
      lockKey: (document.getElementById('lockKeyInput') as HTMLInputElement)?.value ?? 'ctrl+alt+l',
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


  const updateLockButton = async () => {
    try {
      const s = await invoke<any>('get_settings');
      const btn = document.getElementById('btnLockMode');
      if (btn) {
        const locked = s.lock_mode ?? false;
        btn.textContent = locked ? '🔒 Bloqueado' : '🔓 Desbloqueado';
        (btn as HTMLButtonElement).style.background = locked ? 'rgba(245,166,35,.15)' : '';
        (btn as HTMLButtonElement).style.borderColor = locked ? 'var(--warn)' : '';
        (btn as HTMLButtonElement).style.color = locked ? 'var(--warn)' : '';
      }
      const lockKeyEl = document.getElementById('lockKeyDisplay');
      const lockKeyInput = document.getElementById('lockKeyInput') as HTMLInputElement;
      if (lockKeyEl) lockKeyEl.textContent = s.lock_key ?? 'ctrl+alt+l';
      if (lockKeyInput) lockKeyInput.value = s.lock_key ?? 'ctrl+alt+l';
      const notifEl = document.getElementById('notifToggle') as HTMLInputElement;
      if (notifEl) notifEl.checked = s.notifications_enabled ?? true;
    } catch { /* fora do Tauri */ }
  };

  const updateThemeButtons = async () => {
    try {
      const s = await invoke<any>('get_settings');
      const theme = s.theme ?? 'dark';
      const dark  = document.getElementById('themeDark')  as HTMLButtonElement;
      const light = document.getElementById('themeLight') as HTMLButtonElement;
      if (dark && light) {
        dark.style.borderColor  = theme === 'dark'  ? 'var(--cyan)' : 'var(--border)';
        dark.style.color        = theme === 'dark'  ? 'var(--text)' : 'var(--text-2)';
        light.style.borderColor = theme === 'light' ? 'var(--cyan)' : 'var(--border)';
        light.style.color       = theme === 'light' ? 'var(--text)' : 'var(--text-2)';
      }
    } catch { /* fora do Tauri */ }
  };

  // ── Descoberta mDNS ────────────────────────────────────────────────────────
  const refreshDevices = async () => {
    const btn = document.getElementById('btnRefreshDevices') as HTMLButtonElement;
    const subtitle = document.getElementById('deviceSubtitle')!;
    if (btn) { btn.disabled = true; btn.textContent = '⟳ Buscando...'; }
    subtitle.textContent = 'Buscando na rede local (3s)...';
    addLog("Buscando dispositivos Movex na rede...", "info");

    try {
      const peers = await invoke<PeerInfo[]>('discover_peers');
      subtitle.textContent = `${peers.length} dispositivo(s) encontrado(s)`;
      addLog(`Descoberta concluída: ${peers.length} peer(s)`, peers.length > 0 ? 'sec' : 'info');
      renderDiscoveredDevices(peers);
    } catch (e) {
      subtitle.textContent = 'Erro na descoberta';
      addLog(`Erro na descoberta mDNS: ${e}`, 'warn');
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = '↻ Atualizar'; }
    }
  };

  const addManualDevice = () => {
    const modal = document.getElementById('manualModal')!;
    modal.style.display = modal.style.display === 'none' ? 'block' : 'none';
  };

  const goToPainel = () => navTo('painel');

  const connectManual = async () => {
    const ip   = (document.getElementById('manualIp') as HTMLInputElement).value.trim();
    const port = parseInt((document.getElementById('manualPort') as HTMLInputElement).value) || 24800;
    if (!ip) { addLog("Digite um endereço IP", 'warn'); return; }
    addLog(`Conectando a ${ip}:${port}...`, 'info');
    document.getElementById('manualModal')!.style.display = 'none';
    await invoke('connect_to_peer', { addr: ip, port }).catch((e: unknown) => addLog(`Erro: ${e}`, 'warn'));
    goToPainel();
  };

  const connectToPeer = async (addr: string, port: number) => {
    addLog(`Conectando a ${addr}:${port}...`, 'info');
    await invoke('connect_to_peer', { addr, port }).catch((e: unknown) => addLog(`Erro: ${e}`, 'warn'));
    goToPainel();
  };
  // Auto-descoberta
  setTimeout(async () => {
    try {
      const status = await invoke<StatusPayload>('get_status');
      if (!status.connected) refreshDevices();
    } catch { /* fora do Tauri */ }
  }, 1500);

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
  on('btnConnect',         addManualDevice);
  on('pos-above',          () => setPeerPosition('above'));
  on('pos-left',           () => setPeerPosition('left'));
  on('pos-right',          () => setPeerPosition('right'));
  on('pos-below',          () => setPeerPosition('below'));
  on('btnRefreshDevices',  refreshDevices);
  on('btnAddManual',       addManualDevice);
  on('btnConnectManual',   connectManual);
  on('btnCloseManual',     () => { const m = document.getElementById('manualModal'); if (m) m.style.display = 'none'; });
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
  on('btnDiscardSettings', loadCurrentSettings);
  on('btnSaveConfig',      saveConfigAndRefresh);
  on('btnApproveConn',     approveConn);
  on('btnRejectConn',      rejectConn);

  // Cards de papel (servidor/cliente) — usar referência direta
  document.querySelectorAll('[data-role]').forEach(el => {
    el.addEventListener('click', () => selectRoleCard((el as HTMLElement).dataset.role!));
  });

  // Atualizar referência global para loadRecentPeers (usado em renderDeviceCards)
  (window as any).connectToPeer = connectToPeer;
}

// updateStatus foi substituído por onStatusChange no módulo ConnectionStatus

function updateScreenMap(settings: SettingsPayload, status: StatusPayload) {
  const map = document.getElementById('screenMap');
  if (!map) return;
  const isActive = status.active_screen === 'Local';

  // Usar createElement + textContent para evitar XSS (hostname/peer_hostname vêm da rede)
  map.innerHTML = '';

  const makeCard = (isLocal: boolean, hostname: string, active: boolean, connected: boolean) => {
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
    statusEl.textContent = isLocal ? (active ? '● ATIVO' : 'em espera') : (connected ? '● CONECTADO' : 'desconectado');
    card.appendChild(icon);
    card.appendChild(name);
    card.appendChild(statusEl);
    return card;
  };

  map.appendChild(makeCard(true,  settings.hostname,                        isActive,  false));
  const arrow = document.createElement('div');
  arrow.style.cssText = 'color:var(--text-3);font-size:20px;';
  arrow.textContent = '⇄';
  map.appendChild(arrow);
  map.appendChild(makeCard(false, status.peer_hostname ?? 'Aguardando...', !isActive && status.connected, status.connected));
}

function updateDevices(status: StatusPayload, settings: SettingsPayload) {
  const grid = document.getElementById('deviceGrid');
  if (!grid) return;

  // Se conectado: sempre mostrar as máquinas reais (ignora descoberta anterior)
  if (status.connected) {
    grid.dataset.discovered = 'false'; // permite atualizar
    const devices = [
      { name: settings.hostname, ip: 'Esta máquina · Local', icon: '🖥️', online: true, addr: null as string|null, port: 0 },
      {
        name: status.peer_hostname ?? 'Peer',
        ip: '● Conectado agora',
        icon: '💻',
        online: true,
        addr: null as string|null,
        port: 0,
      },
    ];
    grid.innerHTML = deviceCards(devices);
    const subtitle = document.getElementById('deviceSubtitle');
    if (subtitle) subtitle.textContent = `Conectado a ${status.peer_hostname ?? 'peer'}`;
    return;
  }

  // Se não conectado e grid já foi preenchida pela descoberta mDNS, não sobrescrever
  if (grid.dataset.discovered === 'true') return;

  // Mostrar apenas a máquina local enquanto desconectado
  const devices = [
    { name: settings.hostname, ip: 'Esta máquina · Local', icon: '🖥️', online: true, addr: null as string|null, port: 0 },
  ];
  grid.innerHTML = deviceCards(devices);
}

function renderDiscoveredDevices(peers: PeerInfo[]) {
  const grid = document.getElementById('deviceGrid');
  if (!grid) return;
  // Só marcar como descoberto se encontrou peers — senão deixa o updateDevices atualizar
  grid.dataset.discovered = peers.length > 0 ? 'true' : 'false';

  if (peers.length === 0) {
    grid.innerHTML = `
      <div style="grid-column:1/-1;text-align:center;padding:40px;color:var(--text-3);">
        <div style="font-size:32px;margin-bottom:12px;">🔍</div>
        <div style="font-size:14px;font-weight:600;margin-bottom:6px;">Nenhum servidor Movex encontrado</div>
        <div style="font-size:12px;">Certifique-se de que o Movex está aberto e no modo Servidor no outro computador</div>
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
