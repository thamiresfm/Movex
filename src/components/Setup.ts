import { invoke } from "@tauri-apps/api/core";

export async function renderSetup(): Promise<void> {
  const app = document.getElementById("app")!;

  const settings = await invoke<any>("get_settings").catch(() => ({
    hostname: "Meu Computador",
    psk_hex: "0000000000000000000000000000000000000000000000000000000000000000",
    port: 24800,
    peer_position: "right",
  }));

  // Guarda contra psk_hex undefined/curto: usa fallback de zeros antes de fatiar.
  const formatPsk = (hex: string | undefined | null) => {
    const safe = (hex ?? "").padEnd(32, "0");
    return [safe.slice(0,8), safe.slice(8,16), safe.slice(16,24), safe.slice(24,32)]
      .map(s => s.toUpperCase())
      .join("-");
  };

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
          <div id="progress-1" style="width:48px;height:3px;border-radius:2px;background:var(--cyan);"></div>
          <div id="progress-2" style="width:48px;height:3px;border-radius:2px;background:var(--bg-5);"></div>
          <div id="progress-3" style="width:48px;height:3px;border-radius:2px;background:var(--bg-5);"></div>
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
            <div>
              <label style="display:block;font-size:12px;font-weight:600;color:var(--text);margin-bottom:6px;">Porta TCP</label>
              <input id="portInput" type="number" value="${settings.port ?? 24800}"
                style="width:100%;background:var(--bg-2);border:1px solid var(--border);border-radius:8px;padding:9px 13px;font-family:'JetBrains Mono',monospace;font-size:13px;color:var(--text);outline:none;" />
            </div>
          </div>

          <!-- Erro inline do passo 1 (substitui alert) -->
          <div id="step1-error" style="display:none;font-size:12px;font-weight:600;color:var(--danger);margin-bottom:14px;"></div>

          <div style="display:flex;align-items:center;justify-content:space-between;padding-top:4px;">
            <span style="font-size:11px;color:var(--text-3);text-transform:uppercase;letter-spacing:.3px;">ℹ Precisa de ajuda? Ver documentação</span>
            <div style="display:flex;gap:12px;align-items:center;">
              <button id="btnBack1" class="btn-ghost">Voltar</button>
              <button id="btnNext" class="btn-primary">Próximo Passo</button>
            </div>
          </div>
        </div>

        <!-- Passo 2: Posição das telas (matriz de setas) -->
        <div id="step-2" style="display:none;">
          <p style="font-size:13px;color:var(--text-2);text-align:center;margin-bottom:20px;line-height:1.6;">
            Clique numa seta para definir onde o outro monitor fica em relação a este. O cursor passará para o outro PC ao tocar nessa borda.
          </p>
          <div style="display:flex;justify-content:center;margin-bottom:18px;">
            <div style="padding:18px;background:var(--bg-2);border:1px solid var(--border);border-radius:12px;">
              <div style="display:grid;grid-template-columns:1fr 1fr 1fr;grid-template-rows:1fr 1fr 1fr;gap:6px;width:168px;">
                <div></div>
                <button id="pos-above" class="btn btn-outline" style="padding:10px;font-size:16px;">↑</button>
                <div></div>
                <button id="pos-left"  class="btn btn-outline" style="padding:10px;font-size:16px;">←</button>
                <div style="background:var(--cyan-dim);border:1px solid var(--border-c);border-radius:8px;display:flex;align-items:center;justify-content:center;font-size:14px;color:var(--cyan);">📍</div>
                <button id="pos-right" class="btn btn-cyan"    style="padding:10px;font-size:16px;">→</button>
                <div></div>
                <button id="pos-below" class="btn btn-outline" style="padding:10px;font-size:16px;">↓</button>
                <div></div>
              </div>
            </div>
          </div>
          <p id="posLabel" style="font-size:12px;color:var(--text-3);text-align:center;margin-bottom:28px;">Posição selecionada: <strong style="color:var(--cyan);">À Direita</strong></p>
          <div style="display:flex;gap:12px;justify-content:flex-end;">
            <button id="btnBack2" class="btn-ghost">← Voltar</button>
            <button id="btnNext2" class="btn-primary">Próximo Passo</button>
          </div>
        </div>

        <!-- Passo 3: Chave PSK e conclusão -->
        <div id="step-3" style="display:none;">
          <div style="background:var(--bg-2);border:1px solid var(--border-c);border-radius:12px;padding:20px;margin-bottom:20px;display:flex;align-items:center;justify-content:space-between;gap:16px;">
            <code id="pskDisplay" style="font-family:monospace;font-size:18px;letter-spacing:3px;color:var(--cyan);word-break:break-all;">${formatPsk(settings.psk_hex)}</code>
            <button class="btn btn-outline" id="btnCopy">Copiar</button>
          </div>
          <p style="font-size:12px;color:var(--text-3);margin-bottom:18px;">💡 Esta chave é opcional; não é preciso copiá-la para o outro PC. A ligação usa TLS.</p>
          <!-- Erro inline do passo 3 (substitui reload silencioso) -->
          <div id="step3-error" style="display:none;font-size:12px;font-weight:600;color:var(--danger);margin-bottom:18px;"></div>
          <div style="display:flex;gap:12px;justify-content:flex-end;">
            <button id="btnBack3" class="btn-ghost">← Voltar</button>
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
  // Posição do outro monitor escolhida na matriz de setas (passo 2).
  let peerPosition = (settings.peer_position as string) || 'right';

  // ── Helpers de erro inline (substituem os alert/reload silenciosos) ──
  const setError = (id: string, msg: string) => {
    const el = document.getElementById(id);
    if (!el) return;
    el.textContent = msg;
    el.style.display = msg ? 'block' : 'none';
  };
  const clearError = (id: string) => setError(id, '');

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
    // Trocar de modo limpa qualquer erro de validação anterior.
    clearError('step1-error');
  };

  // ── Matriz de setas: define peer_position (passo 2) ──
  const POSITIONS = ['above', 'below', 'left', 'right'] as const;
  const POSITION_LABELS: Record<string, string> = {
    above: 'Acima', below: 'Abaixo', left: 'À Esquerda', right: 'À Direita',
  };
  const renderPositionMatrix = () => {
    POSITIONS.forEach(p => {
      const btn = document.getElementById(`pos-${p}`);
      if (btn) {
        btn.className = p === peerPosition ? 'btn btn-cyan' : 'btn btn-outline';
        (btn as HTMLButtonElement).style.cssText = 'padding:10px;font-size:16px;';
      }
    });
    const label = document.getElementById('posLabel');
    if (label) {
      label.innerHTML = `Posição selecionada: <strong style="color:var(--cyan);">${POSITION_LABELS[peerPosition] ?? peerPosition}</strong>`;
    }
  };
  const selectPosition = (p: string) => {
    peerPosition = p;
    renderPositionMatrix();
  };

  const showStep = (n: number) => {
    (document.getElementById('step-1') as HTMLElement).style.display = n === 1 ? 'block' : 'none';
    (document.getElementById('step-2') as HTMLElement).style.display = n === 2 ? 'block' : 'none';
    (document.getElementById('step-3') as HTMLElement).style.display = n === 3 ? 'block' : 'none';
    // Barra de progresso reflete os 3 passos reais.
    for (let i = 1; i <= 3; i++) {
      const bar = document.getElementById(`progress-${i}`);
      if (bar) bar.style.background = i <= n ? 'var(--cyan)' : 'var(--bg-5)';
    }
    if (n === 2) renderPositionMatrix();
  };

  // Persiste as configurações com a posição atual; usado ao avançar e ao concluir.
  const persistSettings = (hostname: string, serverAddr: string | null, port: number) =>
    invoke('save_settings', {
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
    });

  // Lê e valida os campos do passo 1.
  const readStep1 = (): { hostname: string; serverAddr: string | null; port: number } | null => {
    clearError('step1-error');
    const hostname = (document.getElementById('hostnameInput') as HTMLInputElement).value.trim();
    const serverAddr = (document.getElementById('serverAddrInput') as HTMLInputElement)?.value.trim() || null;
    if (!hostname) { setError('step1-error', 'Digite o nome desta máquina.'); return null; }
    // Cliente exige endereço do servidor: não deixar salvar silenciosamente sem ele.
    if (selectedMode === 'client' && !serverAddr) {
      setError('step1-error', 'Informe o endereço do servidor para o modo Cliente.');
      return null;
    }
    const port = parseInt((document.getElementById('portInput') as HTMLInputElement)?.value) || 24800;
    return { hostname, serverAddr, port };
  };

  const goToStep2 = async () => {
    const fields = readStep1();
    if (!fields) return;
    await persistSettings(fields.hostname, fields.serverAddr, fields.port).catch(console.warn);
    showStep(2);
  };

  const goToStep3 = async () => {
    // Revalida os campos do passo 1 (cliente sem endereço continua bloqueado).
    const fields = readStep1();
    if (!fields) { showStep(1); return; }
    await persistSettings(fields.hostname, fields.serverAddr, fields.port).catch(console.warn);
    showStep(3);
  };

  const copyPsk = async () => {
    const btn = document.getElementById('btnCopy')!;
    try {
      await navigator.clipboard.writeText(settings.psk_hex ?? '');
      btn.textContent = '✓ Copiado!';
    } catch (e) {
      console.warn('Falha ao copiar PSK', e);
      btn.textContent = '✗ Falhou';
    }
    setTimeout(() => { btn.textContent = 'Copiar'; }, 2000);
  };

  const finishSetup = async () => {
    clearError('step3-error');
    try {
      await invoke('complete_setup');
      // Só recarrega em sucesso; em falha mostramos erro e permanecemos no Setup.
      window.location.reload();
    } catch (e) {
      console.warn('Falha ao concluir configuração', e);
      setError('step3-error', 'Não foi possível concluir a configuração. Tente novamente.');
    }
  };

  // ── Registrar eventos via addEventListener (CSP do Tauri bloqueia onclick inline) ──
  document.querySelectorAll('[data-mode]').forEach(el => {
    el.addEventListener('click', () => selectMode((el as HTMLElement).dataset.mode!));
  });

  POSITIONS.forEach(p => {
    document.getElementById(`pos-${p}`)?.addEventListener('click', () => selectPosition(p));
  });

  document.getElementById('btnNext')?.addEventListener('click', goToStep2);
  document.getElementById('btnBack1')?.addEventListener('click', () => {});
  document.getElementById('btnBack2')?.addEventListener('click', () => showStep(1));
  document.getElementById('btnNext2')?.addEventListener('click', goToStep3);
  document.getElementById('btnBack3')?.addEventListener('click', () => showStep(2));
  document.getElementById('btnCopy')?.addEventListener('click', copyPsk);
  document.getElementById('btnFinish')?.addEventListener('click', finishSetup);
}
