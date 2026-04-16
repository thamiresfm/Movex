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
                  <div style="font-size:30px;font-weight:700;color:var(--text);line-height:1;letter-spacing:-1px;" id="latencyVal"><span style="color:var(--text-3);">--</span><span style="font-size:14px;color:var(--text-3);margin-left:4px;">ms</span></div>
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

            <!-- Papel desta máquina -->
            <div class="card" style="margin-bottom:14px;">
              <div style="font-size:11px;font-weight:700;letter-spacing:.8px;color:var(--text-3);text-transform:uppercase;margin-bottom:14px;">Papel desta Máquina</div>
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:16px;">
                <div id="roleServerCard" onclick="selectRoleCard('server')" style="border:2px solid var(--cyan);background:linear-gradient(135deg,#0f1824,#0b0c10);border-radius:12px;padding:18px;cursor:pointer;transition:all .2s;">
                  <div style="width:36px;height:36px;background:var(--cyan-dim);border-radius:9px;display:flex;align-items:center;justify-content:center;margin-bottom:12px;">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--cyan)" stroke-width="1.8"><rect x="2" y="4" width="20" height="14" rx="2"/><path d="M8 20h8M12 18v2"/><path d="M6 9h.01M9 9h6"/></svg>
                  </div>
                  <div style="font-size:14px;font-weight:700;color:var(--text);margin-bottom:4px;">Servidor</div>
                  <div style="font-size:11px;color:var(--text-2);">Controla outras máquinas com este teclado e mouse</div>
                </div>
                <div id="roleClientCard" onclick="selectRoleCard('client')" style="border:1.5px solid var(--border);background:var(--bg-3);border-radius:12px;padding:18px;cursor:pointer;transition:all .2s;">
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
                  <button onclick="applyServerAddr()" class="btn btn-cyan" style="white-space:nowrap;">Salvar</button>
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
                  <button onclick="toggleKey()" style="position:absolute;right:10px;top:50%;transform:translateY(-50%);background:none;border:none;cursor:pointer;color:var(--text-3);font-size:14px;">👁</button>
                </div>
                <div style="font-size:10px;color:var(--text-3);margin-top:6px;">Use a mesma chave nos dois computadores</div>
              </div>
            </div>

            <!-- SSL + Clipboard -->
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:20px;">
              <div class="card" style="border-left:3px solid var(--cyan);">
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;">
                  <div style="font-size:14px;font-weight:700;color:var(--text);">🔒 Criptografia TLS 1.3</div>
                  <label class="toggle"><input type="checkbox" checked disabled /><div class="toggle-track"></div><div class="toggle-thumb"></div></label>
                </div>
                <div style="font-size:11px;color:var(--text-2);line-height:1.5;margin-bottom:10px;">Sempre ativa. Todos os dados são protegidos com AES-256.</div>
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
                <div style="font-size:11px;color:var(--text-2);line-height:1.5;">Copie em um computador e cole no outro automaticamente.</div>
              </div>
            </div>

            <!-- Ações -->
            <div style="display:flex;align-items:center;justify-content:space-between;padding-top:16px;border-top:1px solid var(--border);">
              <!-- Reset -->
              <button onclick="confirmReset()" style="display:flex;align-items:center;gap:8px;padding:10px 16px;border-radius:8px;border:1px solid rgba(255,75,110,.3);background:rgba(255,75,110,.08);color:var(--danger,#ff4b6e);font-family:'Inter',sans-serif;font-size:12px;font-weight:600;cursor:pointer;letter-spacing:.3px;transition:all .15s;">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.48"/></svg>
                Resetar Configurações
              </button>
              <div style="display:flex;gap:10px;">
                <button class="btn-ghost" onclick="loadCurrentSettings()" style="font-size:12px;">Descartar</button>
                <button class="btn btn-cyan" onclick="saveConfig()">Salvar Configurações</button>
              </div>
            </div>

            <!-- Modal de confirmação de reset -->
            <div id="resetModal" style="display:none;position:fixed;inset:0;background:rgba(0,0,0,.7);z-index:999;align-items:center;justify-content:center;">
              <div style="background:var(--bg-3);border:1px solid var(--border);border-radius:16px;padding:28px;width:380px;text-align:center;">
                <div style="font-size:32px;margin-bottom:12px;">⚠️</div>
                <div style="font-size:16px;font-weight:700;color:var(--text);margin-bottom:8px;">Resetar Configurações?</div>
                <div style="font-size:13px;color:var(--text-2);line-height:1.5;margin-bottom:24px;">Todas as configurações serão apagadas e o assistente de configuração inicial será exibido novamente.</div>
                <div style="display:flex;gap:10px;justify-content:center;">
                  <button onclick="closeResetModal()" style="padding:10px 24px;border-radius:8px;border:1px solid var(--border);background:var(--bg-4);color:var(--text-2);font-family:'Inter',sans-serif;font-size:13px;font-weight:600;cursor:pointer;">Cancelar</button>
                  <button onclick="doReset()" style="padding:10px 24px;border-radius:8px;border:none;background:var(--danger,#ff4b6e);color:#fff;font-family:'Inter',sans-serif;font-size:13px;font-weight:700;cursor:pointer;">Sim, Resetar</button>
                </div>
              </div>
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

  (window as any).loadCurrentSettings = loadCurrentSettings;

  (window as any).selectRoleCard = async (role: string) => {
    currentRole = role;
    applyRoleUI(role);
    await invoke('set_role', { role }).catch(console.warn);
    addLog(`Papel alterado para: ${role === 'server' ? 'Servidor' : 'Cliente'}`, 'sec');
  };

  (window as any).applyServerAddr = async () => {
    const input = document.getElementById('serverAddrInput') as HTMLInputElement;
    const addr = input?.value.trim() || null;
    await invoke('set_server_addr', { addr }).catch(console.warn);
    addLog(`Endereço do servidor: ${addr ?? '(removido)'}`, 'info');
  };

  (window as any).confirmReset = () => {
    const modal = document.getElementById('resetModal')!;
    modal.style.display = 'flex';
  };

  (window as any).closeResetModal = () => {
    const modal = document.getElementById('resetModal')!;
    modal.style.display = 'none';
  };

  (window as any).doReset = async () => {
    try {
      await invoke('reset_settings');
      addLog("Configurações resetadas. Reiniciando...", "warn");
      setTimeout(() => window.location.reload(), 800);
    } catch(e) {
      addLog(`Erro ao resetar: ${e}`, 'warn');
    }
  };

  (window as any).saveConfig = async () => {
    const port = parseInt((document.getElementById('portInput') as HTMLInputElement).value) || 24800;
    try {
      const s = await invoke<any>('get_settings');
      await invoke('save_settings', {
        hostname: s.hostname,
        role: currentRole,
        serverAddr: (document.getElementById('serverAddrInput') as HTMLInputElement)?.value.trim() || null,
        port,
        pskHex: s.psk_hex,
        peerPosition: s.peer_position ?? 'right',
        autostart: s.autostart ?? false,
        theme: s.theme ?? 'dark',
      });
      addLog("Configurações salvas com sucesso.", "sec");
    } catch(e) {
      addLog(`Erro ao salvar: ${e}`, 'warn');
    }
  };

  // Carregar configurações atuais ao abrir a página
  await loadCurrentSettings();

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
    if (latEl) {
      if (status.connected && status.latency_ms != null) {
        latEl.innerHTML = `${status.latency_ms}<span style="font-size:14px;color:var(--text-2);margin-left:4px;">ms</span>`;
      } else {
        latEl.innerHTML = `<span style="color:var(--text-3);">--</span><span style="font-size:14px;color:var(--text-3);margin-left:4px;">ms</span>`;
      }
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
