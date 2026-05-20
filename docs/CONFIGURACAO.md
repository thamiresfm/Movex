# Movex v0.1.0 — Guia de Configuração

Referência completa de todas as opções de configuração do Movex.  
Ficheiro de configuração: `~/.movex/config.json` (criado automaticamente na primeira execução).

---

## 1. Configuração rápida (passo a passo)

### Pré-requisitos
- Ambos os PCs na **mesma rede local** (LAN ou Wi-Fi)
- Movex instalado nos dois PCs
- Porta **24800 TCP** aberta no firewall do PC Servidor

### Passo 1 — PC Servidor (Windows recomendado)
1. Abrir Movex → **Configurações**
2. Papel: **Servidor**
3. Copiar a **Chave de Segurança (PSK)** — 8 caracteres visíveis na UI
4. Anotar o **IP do servidor** (mostrado no Painel → Rede)
5. Clicar **Conectar** → estado muda para *Aguardando conexão…*

### Passo 2 — PC Cliente (macOS recomendado)
1. Abrir Movex → **Configurações**
2. Papel: **Cliente**
3. Campo **Endereço do servidor**: inserir o IP anotado no passo anterior
4. Inserir a mesma **Chave de Segurança (PSK)**
5. Clicar **Conectar** → estado muda para *Conectando…* → *● Conectado a [hostname]*

### Passo 3 — Posição do monitor remoto
1. No PC Servidor → Painel → **Matriz de Telas**
2. Clicar na seta que corresponde à posição física do outro PC  
   (ex: Mac está à **esquerda** do Windows → clicar **←**)
3. Mover o cursor até à borda esquerda do ecrã Windows → cursor passa para o Mac

---

## 2. Campos de configuração (`config.json`)

| Campo | Tipo | Padrão | Descrição |
|-------|------|--------|-----------|
| `role` | `"Server"` \| `"Client"` | `"Server"` | Papel desta máquina na sessão KVM |
| `server_addr` | `string \| null` | `null` | IP ou hostname do servidor (só Cliente) |
| `port` | `u16` | `24800` | Porta TCP usada para a ligação |
| `psk_hex` | `string` | gerado | Chave de segurança em hexadecimal (64 chars = 32 bytes) |
| `screen_name` | `string` | hostname | Nome que identifica este PC no handshake |
| `expected_client_screen_name` | `string \| null` | `null` | Servidor: aceitar só clientes com este nome exato (vazio = qualquer) |
| `peer_position` | `"Right"` \| `"Left"` \| `"Above"` \| `"Below"` | `"Right"` | Posição do monitor remoto em relação a este PC |
| `launch_connection_on_startup` | `bool` | `false` | Iniciar sessão KVM automaticamente ao abrir a app |
| `clipboard_sync_enabled` | `bool` | `true` | Sincronizar clipboard de texto entre os PCs |
| `lock_key` | `string` | `"ScrollLock"` | Tecla para bloquear/desbloquear o controlo remoto |
| `lock_mode` | `bool` | `false` | Modo de bloqueio ativo (persistido entre sessões) |
| `notifications_enabled` | `bool` | `true` | Mostrar notificações do sistema (ligação, erros) |
| `autostart` | `bool` | `false` | Iniciar Movex com o sistema operativo |
| `theme` | `"dark"` \| `"light"` | `"dark"` | Tema visual da interface |
| `setup_complete` | `bool` | `false` | Indica se o assistente de configuração inicial foi concluído |

---

## 3. `peer_position` — posição do monitor remoto

Define em que borda do ecrã o cursor "passa" para o outro PC.

```
             Outro PC ACIMA (Above)
                    ↑
Outro PC ←  [ Este PC ]  → Outro PC
ESQUERDA                    DIREITA
(Left)                      (Right)
                    ↓
             Outro PC ABAIXO (Below)
```

**Exemplo:** Mac fisicamente à **esquerda** do Windows  
→ No Windows (Servidor): `peer_position = "Left"`  
→ Cursor chega à borda esquerda do Windows → passa para o Mac

---

## 4. Chave de segurança (PSK)

- Gerada automaticamente na primeira instalação (32 bytes aleatórios = 64 chars hex)
- Deve ser **idêntica** nos dois PCs para que a ligação seja aceite
- A UI mostra apenas os **primeiros 8 caracteres** para confirmação visual
- Para regenerar: **Configurações → Gerar nova chave**
- O valor completo nunca é enviado pela rede em texto plano — é usado apenas no HMAC do handshake

---

## 5. Nomes de ecrã (`screen_name`)

- Identifica o PC no handshake e na Matriz de Telas
- Padrão: hostname do sistema operativo
- Pode conter espaços e caracteres especiais
- O campo `expected_client_screen_name` no Servidor filtra ligações:  
  deixar vazio = aceitar qualquer cliente com a PSK correta  
  preencher = aceitar só o cliente cujo `screen_name` bata exatamente

---

## 6. Porta e firewall

| Sistema | Como abrir a porta |
|---------|-------------------|
| **Windows** | Movex → Configurações → **Aplicar regras no Firewall** (pede UAC) |
| **macOS** | Normalmente não é necessário (macOS não bloqueia ligações de saída). Se houver firewall de terceiros, abrir TCP 24800 |
| **Linux** | `sudo ufw allow 24800/tcp` |

A porta pode ser alterada em **Configurações → Porta** (deve ser a mesma nos dois PCs).

---

## 7. TLS e certificados

O Movex usa **TLS mútuo com TOFU** (Trust On First Use):

1. O Servidor gera um certificado auto-assinado na primeira execução
2. O Cliente valida e guarda o fingerprint na primeira ligação
3. Ligações seguintes verificam o fingerprint guardado

Se reinstalar o Movex no Servidor ou mudar de PC:
→ No Cliente: **Configurações → Esquecer certificado TLS** (ou Resetar)

---

## 8. Clipboard de texto

- Quando ativado, copia automaticamente o clipboard de texto entre os PCs
- Funciona nos dois sentidos (Servidor → Cliente e Cliente → Servidor)
- Limite: **5 MB** de texto por sincronização
- Somente texto (`text/plain`) — imagens e ficheiros não são sincronizados

---

## 9. Tecla de bloqueio (`lock_key`)

Permite travar o controlo remoto com uma tecla:

| Tecla sugerida | Uso |
|---------------|-----|
| `ScrollLock` | Padrão — pouco usado em aplicações modernas |
| `Pause` | Alternativa no Windows |
| `F12` | Alternativa universal |

Quando o bloqueio está ativo:
- O cursor **não passa** para o outro PC mesmo ao atingir a borda
- O ícone na system tray mostra estado bloqueado
- Premir a tecla novamente desbloqueia

---

## 10. Descoberta automática (mDNS)

O Movex anuncia-se na rede local via mDNS (`_movex._tcp.local`):
- No Cliente → **Dispositivos → Atualizar** lista os servidores Movex na LAN
- Clicar num PC da lista preenche o IP automaticamente
- Requer que ambos os PCs estejam na **mesma subnet**

Se a descoberta não funcionar (redes corporativas, VLANs separadas):  
→ Usar **Conectar por IP** com o IP manual do servidor

---

## 11. Arranque automático (`autostart`)

Inicia o Movex quando o utilizador faz login no sistema:

| Sistema | Comportamento |
|---------|--------------|
| **macOS** | Adiciona o app aos itens de login (System Settings → General → Login Items) |
| **Windows** | Adiciona entrada no registo `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |

O `launch_connection_on_startup` é independente: mesmo com `autostart = true`, a sessão KVM só arranca automaticamente se `launch_connection_on_startup = true`.

---

## 12. Configuração manual do `config.json`

Localização:

| Sistema | Caminho |
|---------|---------|
| macOS | `~/.movex/config.json` |
| Windows | `C:\Users\<utilizador>\.movex\config.json` |
| Linux | `~/.movex/config.json` |

Exemplo de configuração completa (Servidor):

```json
{
  "role": "Server",
  "server_addr": null,
  "port": 24800,
  "psk_hex": "a1b2c3d4e5f60718...",
  "screen_name": "Windows-PC",
  "expected_client_screen_name": null,
  "peer_position": "Left",
  "launch_connection_on_startup": true,
  "clipboard_sync_enabled": true,
  "lock_key": "ScrollLock",
  "lock_mode": false,
  "notifications_enabled": true,
  "autostart": false,
  "theme": "dark",
  "setup_complete": true
}
```

Exemplo de configuração completa (Cliente):

```json
{
  "role": "Client",
  "server_addr": "192.168.1.100",
  "port": 24800,
  "psk_hex": "a1b2c3d4e5f60718...",
  "screen_name": "MacBook-Pro",
  "expected_client_screen_name": null,
  "peer_position": "Right",
  "launch_connection_on_startup": true,
  "clipboard_sync_enabled": true,
  "lock_key": "ScrollLock",
  "lock_mode": false,
  "notifications_enabled": true,
  "autostart": false,
  "theme": "dark",
  "setup_complete": true
}
```

---

## 13. Permissões do sistema

O Movex captura e injeta eventos de mouse e teclado ao nível do sistema operativo.
Sem as permissões corretas, o cursor **não passa** entre os computadores mesmo com a conexão estabelecida.

---

### macOS

O macOS exige consentimento explícito do utilizador para cada tipo de acesso privilegiado.

#### Acessibilidade

**Obrigatória** tanto no Servidor (captura de eventos) como no Cliente (injeção de eventos).

| Passo | Ação |
|-------|------|
| 1 | Abrir **Ajustes do Sistema** |
| 2 | Ir a **Privacidade e Segurança → Acessibilidade** |
| 3 | Ativar o interruptor ao lado de **Movex** |
| 4 | Se Movex não aparecer: clicar **+** e adicionar manualmente o app em `/Applications/Movex.app` |
| 5 | **Fechar e reabrir o Movex** |

> **Após atualizar o Movex:** o macOS âncora a permissão ao CD hash do binário. Depois de cada atualização, o toggle pode estar ON mas a permissão já não vale para o novo binário.  
> → Use o botão **«Corrigir»** que aparece no painel do Movex (ou vá a Acessibilidade, desative e reative o Movex).

#### Monitorização de Entrada *(Input Monitoring)*

**Necessária no Servidor** para capturar teclas globalmente (incluindo a tecla de bloqueio `ScrollLock`).

| Passo | Ação |
|-------|------|
| 1 | Abrir **Ajustes do Sistema** |
| 2 | Ir a **Privacidade e Segurança → Monitorização de Entrada** |
| 3 | Ativar o interruptor ao lado de **Movex** |
| 4 | Fechar e reabrir o Movex |

> Se o Movex não aparecer na lista, execute-o primeiro para que o macOS o detete.

#### Resumo rápido — macOS

```
Servidor macOS:  Acessibilidade ✓  +  Monitorização de Entrada ✓
Cliente macOS:   Acessibilidade ✓
```

---

### Windows

#### Regras de Firewall (Servidor)

Na **primeira vez** que clicar em «Conectar» como Servidor, o Movex pede elevação (UAC) para criar regras no Firewall do Windows:

| Regra criada | Detalhe |
|-------------|---------|
| Movex TCP 24800 | Entrada TCP na porta configurada |
| Movex UDP 24800 | Entrada UDP na porta configurada |
| Movex mDNS | Entrada UDP 5353 para descoberta automática |
| Movex App In | Entrada pelo executável do Movex |
| Movex App Out | Saída pelo executável do Movex |

**O que fazer no prompt UAC:**
1. Uma janela do Windows pergunta se permite que o PowerShell faça alterações no dispositivo
2. Clique **Sim**
3. As regras são criadas — nas próximas sessões não é necessário repetir

> Se o prompt UAC não aparecer ou as regras não forem aplicadas:  
> → **Configurações → Permissões → Aplicar regras no Firewall** (dentro do Movex)

#### Execução normal

O Movex **não precisa** ser executado como Administrador no dia a dia.  
A elevação é solicitada apenas uma vez para as regras de Firewall.

#### Verificar se as regras já existem

Execute no Prompt de Comando (sem Admin):
```cmd
netsh advfirewall firewall show rule name="Movex App In"
```
Se a saída mostrar a regra, o Firewall está configurado corretamente.

---

### Sintomas de permissão em falta

| Sintoma | Causa | Solução |
|---------|-------|---------|
| Conexão OK mas cursor não passa | Acessibilidade não concedida (macOS) | Ajustes do Sistema → Acessibilidade → ativar Movex |
| Teclado não funciona no Mac remoto | Input Monitoring não concedida | Ajustes do Sistema → Monitorização de Entrada → ativar Movex |
| Aviso «Permissão de Acessibilidade» no painel | Binário atualizado — TCC stale | Clicar «Corrigir» no painel do Movex |
| Cliente não consegue ligar ao Servidor Windows | Firewall a bloquear porta 24800 | Configurações → Aplicar regras no Firewall (UAC) |
| Prompt UAC não aparece ao clicar Conectar | UAC desativado ou política de grupo | Ir manualmente a Configurações → Aplicar regras no Firewall |

---

## 14. Resolução de problemas

| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| "Conectando…" sem progredir | IP errado ou firewall a bloquear | Verificar IP, porta 24800 aberta, mesma LAN |
| "Handshake falhou" | PSK diferente nos dois PCs | Copiar a PSK de um PC para o outro |
| Cursor não passa para o outro PC | `peer_position` errada | Ajustar em Configurações ou na Matriz de Telas |
| Cursor aparece no lado errado ao voltar | Monitor primário não é o da borda de cruzamento | Atualizado em v0.1.0 (usa bounding-box de todos os monitores) |
| Teclado não funciona no Mac | Permissão de Acessibilidade ou Input Monitoring não concedida | macOS → Privacidade → Acessibilidade → ativar Movex |
| Clipboard não sincroniza | Toggle desativado ou PSK errada | Verificar toggle em Configurações; PSK deve ser igual |
| "TLS handshake falhou" | Certificado do servidor mudou (reinstalação) | Cliente → Configurações → Esquecer certificado TLS |
| App não arranca automaticamente | `autostart` desligado | Configurações → Iniciar com o sistema |
