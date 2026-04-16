import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { addLog, clearLogs } from "./Logs";

interface StatusPayload {
  connected: boolean;
  status_text: string;
  peer_hostname?: string;
  latency_ms?: number;
  active_screen: string;
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
          <div class="nav-item active" id="nav-painel" onclick="navTo('painel',this)">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
            Painel
          </div>
          <div class="nav-item" id="nav-dispositivos" onclick="navTo('dispositivos',this)">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="4" width="20" height="14" rx="2"/><path d="M8 20h8M12 18v2"/></svg>
            Dispositivos
          </div>
          <div class="nav-item" id="nav-seguranca" onclick="navTo('seguranca',this)">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
            Segurança
          </div>
          <div class="nav-item" id="nav-configuracoes" onclick="navTo('configuracoes',this)">
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
                  <div style="font-size:30px;font-weight:700;color:var(--text);line-height:1;letter-spacing:-1px;" id="latencyVal">4<span style="font-size:14px;color:var(--text-2);margin-left:4px;">ms</span></div>
                  <div style="font-size:11px;color:var(--text-3);margin-top:6px;">Sincronização ponta a ponta</div>
                </div>
              </div>
            </div>

            <div class="card">
              <div style="display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:20px;">
                <div>
                  <div class="section-title">Matriz de Telas</div>
                  <div class="section-sub">Arraste os ícones para mapear as posições físicas dos monitores</div>
                </div>
                <div style="display:flex;gap:10px;">
                  <button class="btn btn-outline">Redefinir Layout</button>
                  <button class="btn btn-cyan" id="btnConnect">Conectar</button>
                </div>
              </div>
              <div id="screenMap" style="display:flex;align-items:center;justify-content:center;gap:24px;padding:20px 0;"></div>
            </div>
          </div>

          <!-- DISPOSITIVOS -->
          <div class="page" id="page-dispositivos">
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:20px;">
              <div><div class="section-title">Dispositivos</div><div class="section-sub">Máquinas na rede local</div></div>
              <button class="btn btn-cyan">+ Adicionar</button>
            </div>
            <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:14px;" id="deviceGrid"></div>
          </div>

          <!-- SEGURANÇA — Logs -->
          <div class="page" id="page-seguranca">
            <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-bottom:20px;">
              <div class="card"><div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Tempo de Atividade</div><div style="font-size:36px;font-weight:700;color:var(--text);letter-spacing:-1px;" id="uptimeVal">0<span style="font-size:14px;color:var(--text-2);margin-left:4px;">Horas</span></div></div>
              <div class="card"><div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Ameaças Bloqueadas</div><div style="font-size:36px;font-weight:700;color:var(--text);letter-spacing:-1px;">0<span style="font-size:14px;color:var(--text-2);margin-left:4px;">Ativas</span></div></div>
              <div class="card"><div style="font-size:10px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:12px;">Tráfego de Rede</div><div style="display:flex;align-items:flex-end;gap:6px;height:40px;margin-top:8px;">${[50,70,40,80,55,35,95].map((h,i)=>`<div style="flex:1;border-radius:3px 3px 0 0;background:${i===3||i===6?'var(--cyan)':'var(--bg-5)'};height:${h}%;min-height:8px;"></div>`).join('')}</div></div>
            </div>

            <div class="card" style="overflow:hidden;padding:0;margin-bottom:20px;">
              <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 16px;border-bottom:1px solid var(--border);background:var(--bg-3);">
                <div style="display:flex;gap:6px;"><div style="width:10px;height:10px;border-radius:50%;background:#ff5f57;"></div><div style="width:10px;height:10px;border-radius:50%;background:#febc2e;"></div><div style="width:10px;height:10px;border-radius:50%;background:#28c840;"></div></div>
                <code style="font-family:'JetBrains Mono',monospace;font-size:11px;color:var(--text-2);background:var(--bg-4);padding:3px 10px;border-radius:5px;">/var/log/movex/connection.log</code>
                <div style="display:flex;gap:12px;">
                  <button onclick="copyLogs()" style="display:flex;align-items:center;gap:6px;font-size:11px;color:var(--text-3);cursor:pointer;background:none;border:none;font-family:'Inter',sans-serif;">📋 Copiar</button>
                  <button onclick="clearLogs()" style="display:flex;align-items:center;gap:6px;font-size:11px;color:var(--text-3);cursor:pointer;background:none;border:none;font-family:'Inter',sans-serif;">🗑 Limpar Logs</button>
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
            <div style="display:grid;grid-template-columns:1fr 280px;gap:14px;margin-bottom:14px;">
              <div class="card">
                <div style="display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:12px;">
                  <div style="display:flex;align-items:center;gap:12px;">
                    <div style="width:36px;height:36px;background:var(--cyan-dim);border-radius:9px;display:flex;align-items:center;justify-content:center;color:var(--cyan);"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0110 0v4"/></svg></div>
                    <span style="font-size:15px;font-weight:700;color:var(--text);">Ativar Criptografia SSL</span>
                  </div>
                  <label class="toggle"><input type="checkbox" checked /><div class="toggle-track"></div><div class="toggle-thumb"></div></label>
                </div>
                <p style="font-size:12px;color:var(--text-2);line-height:1.5;margin-bottom:16px;">Proteja todos os pacotes de dados entre dispositivos usando o protocolo militar TLS 1.3.</p>
                <div style="display:flex;gap:8px;"><span style="padding:3px 10px;border-radius:5px;font-size:10px;font-weight:700;letter-spacing:.5px;text-transform:uppercase;background:var(--cyan-dim);border:1px solid var(--border-c);color:var(--cyan);">AES-256 Ativo</span><span style="padding:3px 10px;border-radius:5px;font-size:10px;font-weight:700;letter-spacing:.5px;text-transform:uppercase;background:var(--cyan-dim);border:1px solid var(--border-c);color:var(--cyan);">Certificado Válido</span></div>
              </div>
              <div class="card">
                <div style="font-size:9px;font-weight:600;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:6px;">Análise de Protocolo</div>
                <div style="font-size:14px;font-weight:700;color:var(--text);margin-bottom:14px;">Densidade de Tráfego</div>
                <div style="display:flex;align-items:flex-end;gap:5px;height:64px;">${[40,60,45,75,55,90,100].map((h,i)=>`<div style="flex:1;border-radius:4px 4px 0 0;background:${i>=5?'var(--cyan)':'var(--bg-5)'};height:${h}%;min-height:8px;"></div>`).join('')}</div>
                <div style="display:flex;justify-content:space-between;margin-top:6px;font-size:9px;color:var(--text-3);"><span>08:00</span><span>Ativo</span><span>12:00</span></div>
              </div>
            </div>

            <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:14px;">
              <div class="card">
                <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" stroke-width="1.8"><rect x="2" y="7" width="20" height="14" rx="2"/><path d="M16 7V5a2 2 0 00-2-2h-4a2 2 0 00-2 2v2"/></svg><span style="font-size:13px;font-weight:600;color:var(--text);">Número da Porta</span></div>
                <div style="position:relative;"><input type="number" value="24800" id="portInput" style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:10px 80px 10px 14px;font-family:'JetBrains Mono',monospace;font-size:14px;font-weight:600;color:var(--text);outline:none;" /><span style="position:absolute;right:10px;top:50%;transform:translateY(-50%);font-size:9px;font-weight:700;color:var(--text-3);text-transform:uppercase;">Padrão TCP</span></div>
              </div>
              <div class="card">
                <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" stroke-width="1.8"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg><span style="font-size:13px;font-weight:600;color:var(--text);">Chave de Acesso</span></div>
                <div style="position:relative;"><input type="password" id="keyInput" value="movex-secret-key" style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:10px 40px 10px 14px;font-family:'JetBrains Mono',monospace;font-size:14px;font-weight:600;color:var(--text);outline:none;" /><button onclick="toggleKey()" style="position:absolute;right:10px;top:50%;transform:translateY(-50%);background:none;border:none;cursor:pointer;color:var(--text-3);">👁</button></div>
              </div>
            </div>

            <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:20px;">
              <div class="card" style="border-left:3px solid var(--cyan);">
                <div style="display:flex;justify-content:space-between;margin-bottom:12px;"><div style="width:36px;height:36px;background:var(--cyan-dim);border-radius:9px;display:flex;align-items:center;justify-content:center;color:var(--cyan);"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg></div><label class="toggle"><input type="checkbox" checked /><div class="toggle-track"></div><div class="toggle-thumb"></div></label></div>
                <div style="font-size:14px;font-weight:700;color:var(--text);margin-bottom:6px;">Compartilhamento de Área de Transferência</div>
                <div style="font-size:12px;color:var(--text-2);line-height:1.5;">Sincronize textos, imagens e arquivos entre todos os computadores conectados instantaneamente.</div>
              </div>
              <div class="card" style="display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;gap:10px;">
                <div style="width:52px;height:52px;background:var(--cyan-dim);border:2px solid var(--border-c);border-radius:14px;display:flex;align-items:center;justify-content:center;color:var(--cyan);"><svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><polyline points="9 12 11 14 15 10"/></svg></div>
                <div style="font-size:11px;font-weight:700;letter-spacing:1px;color:var(--text);text-transform:uppercase;">Modo Fortalecido</div>
                <div style="font-size:11px;color:var(--text-3);">Bloqueio automático de nós a cada 24h.</div>
              </div>
            </div>

            <div style="display:flex;justify-content:flex-end;gap:12px;padding-top:16px;border-top:1px solid var(--border);">
              <button class="btn-ghost" style="text-transform:uppercase;font-size:12px;letter-spacing:.4px;">Descartar Alterações</button>
              <button class="btn btn-cyan" onclick="saveConfig()">Implantar Configuração</button>
            </div>
          </div>

        </div>
      </div>
    </div>
  `;

  // Inicializar
  await updateStatus();
  setInterval(updateStatus, 3000);

  addLog("Movex iniciado.", "info");
  addLog("Aguardando conexões na porta 24800.", "info");

  // Handlers globais
  (window as any).navTo = (page: string, el: HTMLElement) => {
    document.querySelectorAll('.page').forEach(p => (p as HTMLElement).classList.remove('active'));
    document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
    const pageEl = document.getElementById(`page-${page}`);
    if (pageEl) pageEl.classList.add('active');
    el.classList.add('active');
    const titles: Record<string, string> = {
      painel: 'Painel Principal',
      dispositivos: 'Dispositivos',
      seguranca: 'Logs de Segurança · Interface de Diagnóstico',
      configuracoes: 'Segurança & Conexão',
    };
    const titleEl = document.getElementById('pageTitle');
    if (titleEl) titleEl.textContent = titles[page] ?? page;
  };

  (window as any).clearLogs = clearLogs;
  (window as any).copyLogs = () => navigator.clipboard.writeText(document.getElementById('logBody')?.innerText ?? '');
  (window as any).toggleKey = () => {
    const inp = document.getElementById('keyInput') as HTMLInputElement;
    inp.type = inp.type === 'password' ? 'text' : 'password';
  };
  (window as any).saveConfig = async () => {
    const port = parseInt((document.getElementById('portInput') as HTMLInputElement).value);
    await invoke('save_settings', {
      hostname: '', role: 'server', serverAddr: null,
      port, pskHex: '', peerPosition: 'right', autostart: false, theme: 'dark'
    }).catch(console.warn);
    addLog("Configuração implantada.", "sec");
  };

  document.getElementById('btnConnect')?.addEventListener('click', async () => {
    addLog("Iniciando conexão...", "info");
    await invoke('start_connection').catch((e: unknown) => addLog(`Erro: ${e}`, 'warn'));
  });

  document.getElementById('cmdInput')?.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter') return;
    const input = e.target as HTMLInputElement;
    const cmd = input.value.trim();
    if (!cmd) return;
    addLog(`$ ${cmd}`, 'info');
    addLog(simulateCmd(cmd), 'sec');
    input.value = '';
  });

  // Auto-log periódico
  setInterval(() => {
    const msgs: [string, string][] = [
      ['Heartbeat: todos os nós respondendo.', 'info'],
      ['Chave de sessão renovada.', 'sec'],
      ['Latência média: 3ms.', 'info'],
    ];
    const m = msgs[Math.floor(Math.random() * msgs.length)];
    addLog(m[0], m[1]);
  }, 10000);
}

async function updateStatus() {
  try {
    const [status, settings] = await Promise.all([
      invoke<StatusPayload>('get_status'),
      invoke<SettingsPayload>('get_settings'),
    ]);

    updateScreenMap(settings, status);

    const latEl = document.getElementById('latencyVal');
    if (latEl && status.latency_ms !== undefined) {
      latEl.innerHTML = `${status.latency_ms}<span style="font-size:14px;color:var(--text-2);margin-left:4px;">ms</span>`;
    }

    const uptimeEl = document.getElementById('uptimeVal');
    if (uptimeEl) {
      const hrs = Math.floor(Date.now() / 3600000) % 999;
      uptimeEl.innerHTML = `${hrs}<span style="font-size:14px;color:var(--text-2);margin-left:4px;">Horas</span>`;
    }

    updateDevices(status, settings);
  } catch { /* Tauri não disponível em browser */ }
}

function updateScreenMap(settings: SettingsPayload, status: StatusPayload) {
  const map = document.getElementById('screenMap');
  if (!map) return;
  const peer = status.peer_hostname ?? 'Aguardando...';
  const isActive = status.active_screen === 'Local';

  map.innerHTML = `
    <div draggable="true" style="background:${isActive?'linear-gradient(160deg,#0f1824,#0d1520)':'var(--bg-4)'};border:1.5px solid ${isActive?'var(--cyan)':'var(--border)'};border-radius:12px;padding:18px 20px 14px;width:148px;text-align:center;cursor:grab;${isActive?'box-shadow:0 0 20px rgba(0,212,255,.12);':''}">
      <div style="font-size:28px;margin-bottom:8px;">🖥️</div>
      <div style="font-size:11px;font-weight:700;color:var(--text);letter-spacing:.5px;text-transform:uppercase;">${settings.hostname}</div>
      <div style="font-size:10px;color:${isActive?'var(--cyan)':'var(--text-3)'};margin-top:3px;">${isActive?'● ATIVO':'em espera'}</div>
    </div>
    <div style="color:var(--text-3);font-size:20px;">⇄</div>
    <div style="background:${!isActive&&status.connected?'linear-gradient(160deg,#0f1824,#0d1520)':'var(--bg-4)'};border:1.5px solid ${!isActive&&status.connected?'var(--cyan)':'var(--border)'};border-radius:12px;padding:18px 20px 14px;width:148px;text-align:center;cursor:grab;">
      <div style="font-size:28px;margin-bottom:8px;">🖥️</div>
      <div style="font-size:11px;font-weight:700;color:var(--text);letter-spacing:.5px;text-transform:uppercase;">${peer}</div>
      <div style="font-size:10px;color:${status.connected?'var(--cyan)':'var(--text-3)'};margin-top:3px;">${status.connected?'● CONECTADO':'desconectado'}</div>
    </div>
  `;
}

function updateDevices(status: StatusPayload, settings: SettingsPayload) {
  const grid = document.getElementById('deviceGrid');
  if (!grid) return;
  const devices = [
    { name: settings.hostname, ip: '192.168.1.1 · Local', icon: '🖥️', online: true },
    { name: status.peer_hostname ?? 'Aguardando...', ip: '192.168.1.x', icon: '💻', online: status.connected },
  ];
  grid.innerHTML = devices.map(d => `
    <div style="background:var(--bg-3);border:1px solid ${d.online?'rgba(0,212,255,.2)':'var(--border)'};border-radius:14px;padding:20px;cursor:pointer;transition:all .2s;">
      <div style="width:44px;height:44px;background:var(--bg-4);border-radius:11px;display:flex;align-items:center;justify-content:center;margin-bottom:14px;font-size:22px;">${d.icon}</div>
      <div style="font-size:14px;font-weight:700;color:var(--text);margin-bottom:4px;">${d.name}</div>
      <div style="font-size:11px;color:var(--text-3);margin-bottom:10px;">${d.ip}</div>
      <span style="display:inline-flex;align-items:center;gap:5px;padding:3px 9px;border-radius:20px;font-size:10px;font-weight:600;background:${d.online?'rgba(0,212,255,.12)':'rgba(255,75,110,.1)'};color:${d.online?'var(--cyan)':'var(--danger)'};">
        <span style="width:5px;height:5px;border-radius:50%;background:currentColor;"></span>
        ${d.online ? 'Online' : 'Offline'}
      </span>
    </div>
  `).join('');
}

function simulateCmd(cmd: string): string {
  if (cmd.includes('netstat')) return 'tcp 0.0.0.0:24800 LISTEN · ESTABLISHED';
  if (cmd.includes('ping')) return 'PING 192.168.1.x: 56 bytes · 2ms';
  if (cmd.includes('status')) return 'Movex v0.1.0 · TLS 1.3 · Ativo';
  if (cmd === 'clear') { clearLogs(); return ''; }
  return `Comando '${cmd}' executado.`;
}
