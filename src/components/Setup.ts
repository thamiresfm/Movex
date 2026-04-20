import { invoke } from "@tauri-apps/api/core";

export async function renderSetup(): Promise<void> {
  const app = document.getElementById("app")!;

  const settings = await invoke<any>("get_settings").catch(() => ({
    hostname: "Meu Computador",
    psk_hex: "0000000000000000000000000000000000000000000000000000000000000000",
  }));

  const formatPsk = (hex: string) =>
    [hex.slice(0,8), hex.slice(8,16), hex.slice(16,24), hex.slice(24,32)]
      .map(s => s.toUpperCase())
      .join("-");

  app.innerHTML = `
    <div style="min-height:100vh;display:flex;align-items:flex-start;justify-content:center;padding:32px 24px 100px;background:var(--bg);position:relative;overflow-y:auto;">
      <div style="position:fixed;bottom:-100px;right:-100px;width:500px;height:500px;border-radius:50%;background:radial-gradient(circle,rgba(0,212,255,.08) 0%,transparent 60%);pointer-events:none;"></div>

      <div style="width:640px;max-width:100%;margin-top:auto;margin-bottom:auto;">
        <div style="text-align:center;margin-bottom:20px;">
          <div style="font-size:28px;font-weight:800;color:var(--cyan);letter-spacing:-1px;">Movex</div>
        </div>

        <div style="display:flex;align-items:center;justify-content:center;gap:7px;font-size:10px;font-weight:600;letter-spacing:1.2px;color:var(--text-3);text-transform:uppercase;margin-bottom:16px;">
          <span style="width:6px;height:6px;border-radius:50%;background:var(--cyan);display:inline-block;"></span>
          Fase de Configuração 01
        </div>

        <h1 style="font-size:36px;font-weight:800;color:var(--text);letter-spacing:-1px;text-align:center;margin-bottom:12px;">Escolha seu Modo</h1>
        <p style="font-size:14px;color:var(--text-2);text-align:center;max-width:400px;margin:0 auto 28px;line-height:1.6;">
          Selecione como esta máquina funcionará no ecossistema Movex. Você pode alterar isso depois nas configurações.
        </p>

        <div style="display:flex;gap:8px;justify-content:center;margin-bottom:36px;">
          <div style="width:48px;height:3px;border-radius:2px;background:var(--cyan);"></div>
          <div style="width:48px;height:3px;border-radius:2px;background:var(--bg-5);"></div>
          <div style="width:48px;height:3px;border-radius:2px;background:var(--bg-5);"></div>
        </div>

        <div id="step-1">
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;margin-bottom:36px;">
            <div class="mode-card selected" id="modeServer" data-mode="server" style="background:var(--bg-3);border:1.5px solid var(--cyan);border-radius:14px;padding:28px 24px;cursor:pointer;transition:all .2s;background:linear-gradient(135deg,#0f1824,#0b0c10);box-shadow:0 0 24px rgba(0,212,255,.1);">
              <div style="width:40px;height:40px;background:var(--cyan-dim);border-radius:10px;display:flex;align-items:center;justify-content:center;margin-bottom:16px;">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--cyan)" stroke-width="1.5"><rect x="2" y="4" width="20" height="14" rx="2"/><path d="M8 20h8M12 18v2"/></svg>
              </div>
              <div style="font-size:15px;font-weight:700;color:var(--text);margin-bottom:8px;">Servidor (Computador Principal)</div>
              <div style="font-size:12px;color:var(--text-2);line-height:1.5;">Controle outras máquinas com este teclado/mouse. Ideal para sua estação de trabalho primária.</div>
            </div>

            <div class="mode-card" id="modeClient" data-mode="client" style="background:var(--bg-3);border:1.5px solid var(--border);border-radius:14px;padding:28px 24px;cursor:pointer;transition:all .2s;">
              <div style="width:40px;height:40px;background:var(--bg-5);border-radius:10px;display:flex;align-items:center;justify-content:center;margin-bottom:16px;">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--text-2)" stroke-width="1.5"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
              </div>
              <div style="font-size:15px;font-weight:700;color:var(--text);margin-bottom:8px;">Cliente (Computador Remoto)</div>
              <div style="font-size:12px;color:var(--text-2);line-height:1.5;">Controle esta máquina a partir de outro computador. Use para laptops ou telas auxiliares.</div>
            </div>
          </div>

          <!-- Campos de configuração -->
          <div style="display:grid;grid-template-columns:1fr;gap:12px;margin-bottom:20px;">
            <div id="server-addr-field" style="display:none;">
              <label style="display:block;font-size:12px;font-weight:600;color:var(--text);margin-bottom:6px;">Endereço do Servidor</label>
              <input id="serverAddrInput" type="text" placeholder="Ex: 192.168.1.100"
                style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
              <div style="font-size:10px;color:var(--text-3);margin-top:4px;">IP ou hostname do servidor na rede local</div>
            </div>
            <div>
              <label style="display:block;font-size:12px;font-weight:600;color:var(--text);margin-bottom:6px;">Nome desta máquina</label>
              <input id="hostnameInput" type="text"
                style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'Inter',sans-serif;font-size:13px;color:var(--text);outline:none;" />
            </div>
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px;">
              <div>
                <label style="display:block;font-size:12px;font-weight:600;color:var(--text);margin-bottom:6px;">Porta TCP</label>
                <input id="portInput" type="number" value="${settings.port ?? 24800}"
                  style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
              </div>
              <div>
                <label style="display:block;font-size:12px;font-weight:600;color:var(--text);margin-bottom:6px;">Posição do outro monitor</label>
                <select id="peerPositionInput"
                  style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-size:13px;color:var(--text);outline:none;">
                  <option value="right" ${(settings.peer_position ?? 'right') === 'right' ? 'selected' : ''}>→ À Direita</option>
                  <option value="left"  ${settings.peer_position === 'left'  ? 'selected' : ''}>← À Esquerda</option>
                  <option value="above" ${settings.peer_position === 'above' ? 'selected' : ''}>↑ Acima</option>
                  <option value="below" ${settings.peer_position === 'below' ? 'selected' : ''}>↓ Abaixo</option>
                </select>
              </div>
            </div>
          </div>

          <div style="display:flex;align-items:center;justify-content:space-between;padding-top:4px;">
            <span style="font-size:11px;color:var(--text-3);text-transform:uppercase;letter-spacing:.3px;">ℹ Precisa de ajuda? Ver documentação</span>
            <div style="display:flex;gap:12px;align-items:center;">
              <button id="btnBack1" class="btn-ghost">Voltar</button>
              <button id="btnNext" class="btn-primary">Próximo Passo</button>
            </div>
          </div>
        </div>

        <div id="step-2" style="display:none;">
          <div style="background:var(--bg-2);border:1px solid var(--border-c);border-radius:12px;padding:20px;margin-bottom:20px;display:flex;align-items:center;justify-content:space-between;gap:16px;">
            <code id="pskDisplay" style="font-family:monospace;font-size:18px;letter-spacing:3px;color:var(--cyan);word-break:break-all;">${formatPsk(settings.psk_hex)}</code>
            <button class="btn btn-outline" id="btnCopy">Copiar</button>
          </div>
          <p style="font-size:12px;color:var(--text-3);margin-bottom:28px;">💡 Esta chave é opcional; não é preciso copiá-la para o outro PC. A ligação usa TLS e a aprovação no servidor.</p>
          <div style="display:flex;gap:12px;justify-content:flex-end;">
            <button id="btnBack2" class="btn-ghost">← Voltar</button>
            <button id="btnFinish" class="btn-primary">Concluir Configuração ✓</button>
          </div>
        </div>
      </div>

      <div style="position:fixed;bottom:20px;right:20px;background:var(--bg-3);border:1px solid var(--border);border-radius:10px;padding:10px 14px;font-size:10px;">
        <div style="display:flex;justify-content:space-between;gap:24px;margin-bottom:4px;">
          <span style="color:var(--text-3);text-transform:uppercase;letter-spacing:.5px;">Status do Sistema</span>
          <span style="color:var(--warn);font-weight:600;">Ocioso / Pronto</span>
        </div>
        <div style="display:flex;justify-content:space-between;gap:24px;">
          <span style="color:var(--text-3);text-transform:uppercase;letter-spacing:.5px;">Malha de Rede</span>
          <div style="display:flex;align-items:flex-end;gap:2px;">
            ${[8,12,16,6,10].map((h,i) => `<div style="width:3px;height:${h}px;border-radius:1px;background:${i<3?'var(--cyan)':'var(--bg-5)'};"></div>`).join('')}
          </div>
        </div>
      </div>
    </div>
  `;

  // Preencher hostname via .value para evitar XSS
  const hostnameInput = document.getElementById('hostnameInput') as HTMLInputElement;
  if (hostnameInput) hostnameInput.value = settings.hostname;

  let selectedMode = 'server';

  const selectMode = (mode: string) => {
    selectedMode = mode;
    const server = document.getElementById('modeServer')!;
    const client = document.getElementById('modeClient')!;
    const addrField = document.getElementById('server-addr-field')!;
    if (mode === 'server') {
      server.style.cssText += ';border-color:var(--cyan);background:linear-gradient(135deg,#0f1824,#0b0c10);box-shadow:0 0 24px rgba(0,212,255,.1);';
      client.style.cssText += ';border-color:var(--border);box-shadow:none;background:var(--bg-3);';
      addrField.style.display = 'none';
    } else {
      client.style.cssText += ';border-color:var(--cyan);background:linear-gradient(135deg,#0f1824,#0b0c10);box-shadow:0 0 24px rgba(0,212,255,.1);';
      server.style.cssText += ';border-color:var(--border);box-shadow:none;background:var(--bg-3);';
      addrField.style.display = 'block';
    }
  };

  const showStep = (n: number) => {
    (document.getElementById('step-1') as HTMLElement).style.display = n === 1 ? 'block' : 'none';
    (document.getElementById('step-2') as HTMLElement).style.display = n === 2 ? 'block' : 'none';
  };

  const goToStep2 = async () => {
    const hostname = (document.getElementById('hostnameInput') as HTMLInputElement).value.trim();
    const serverAddr = (document.getElementById('serverAddrInput') as HTMLInputElement)?.value.trim() || null;
    if (!hostname) { alert('Digite o nome desta máquina.'); return; }
    const port = parseInt((document.getElementById('portInput') as HTMLInputElement)?.value) || 24800;
    const peerPosition = (document.getElementById('peerPositionInput') as HTMLSelectElement)?.value || 'right';
    await invoke('save_settings', {
      hostname,
      screenName: hostname,
      expectedClientScreenName: null,
      launchConnectionOnStartup: false,
      role: selectedMode,
      serverAddr,
      port,
      pskHex: settings.psk_hex,
      peerPosition,
      autostart: false,
      theme: 'dark',
    }).catch(console.warn);
    showStep(2);
  };

  const copyPsk = () => {
    navigator.clipboard.writeText(settings.psk_hex);
    const btn = document.getElementById('btnCopy')!;
    btn.textContent = '✓ Copiado!';
    setTimeout(() => { btn.textContent = 'Copiar'; }, 2000);
  };

  const finishSetup = async () => {
    await invoke('complete_setup').catch(console.warn);
    window.location.reload();
  };

  // ── Registrar eventos via addEventListener (CSP do Tauri bloqueia onclick inline) ──
  document.querySelectorAll('[data-mode]').forEach(el => {
    el.addEventListener('click', () => selectMode((el as HTMLElement).dataset.mode!));
  });

  document.getElementById('btnNext')?.addEventListener('click', goToStep2);
  document.getElementById('btnBack1')?.addEventListener('click', () => {});
  document.getElementById('btnBack2')?.addEventListener('click', () => showStep(1));
  document.getElementById('btnCopy')?.addEventListener('click', copyPsk);
  document.getElementById('btnFinish')?.addEventListener('click', finishSetup);
}
