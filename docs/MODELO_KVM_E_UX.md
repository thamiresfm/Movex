# Movex — Modelo KVM (referência Barrier / Deskflow) e checklist de UX

Este documento é **interno**: compara a *lógica de produto* do Movex com software KVM em rede ([Barrier](https://github.com/debauchee/barrier), [Deskflow](https://github.com/deskflow/deskflow)) **sem copiar código** — só alinhamento conceitual e critérios de interface.

---

## 1. Modelo mental partilhado (KVM por rede)

| Conceito (Barrier / Deskflow) | Equivalente no Movex | Onde está na app |
|------------------------------|----------------------|------------------|
| **Servidor** = máquina cujo teclado/rato *controlam* as outras | Papel **Servidor** (`Role::Server`), processo escuta TCP/TLS | Configurações → cartão Servidor; **Conectar** inicia escuta |
| **Cliente** = máquina *controlada* remotamente | Papel **Cliente**, liga ao endereço do servidor | Configurações → Cliente + **Endereço do servidor**; Dispositivos → IP manual ou descoberta |
| **Ligação de rede** | IP:porta (padrão `24800`), opcionalmente descoberta (mDNS) | Dispositivos → **Atualizar** / **Conectar por IP** |
| **Confiança na ligação** | TLS + chave (PSK) + handshake; TOFU do certificado do servidor | Configurações → **Chave de Segurança**; primeiro contacto grava fingerprint |
| **Quem pode entrar** | No Barrier: configuração de ecrãs; no Deskflow: TLS e políticas | Movex: **aprovação explícita** no servidor antes da sessão (modal de pedido); opcionalmente **nome de ecrã** no cliente e filtro **«aceitar só este nome»** no servidor (comparação exata, estilo Barrier) |
| **Nome do ecrã** | Identificador do PC na grelha / handshake | Configurações → **Nomes dos ecrãs**; usado no `Hello` (campo `hostname` do protocolo); mDNS usa o mesmo nome |
| **Início da sessão ao abrir a app** | Barrier: o utilizador inicia o serviço / ligação conforme preferência | Movex: opção **«Ligar sessão KVM ao abrir o app»** (predefinição desligada para novas instalações; migrações antigas mantêm o arranque automático) |
| **Saltar o rato para o outro ecrã** | Borda do ecrã / layout | Painel → **Matriz de Telas** + posição do monitor remoto (↑←→↓) |
| **Clipboard entre máquinas** | Clipboard partilhado | Preferências / toggle clipboard (quando aplicável) |
| **Estabilidade** | Reconexão | Cliente reconecta com backoff; estado **Reconectando…** na UI |

### Diferenças conscientes (não são bugs)

- **Barrier** usa **grelha de vários ecrãs** e nomes de ecrã *case-sensitive* na configuração do servidor. O Movex hoje modela **um par** servidor↔cliente na prática e posição **relativa** (não editor N×M completo).
- **Deskflow** documenta TLS moderno e compatibilidade com vários forks. O Movex usa **TLS + PSK** com protocolo próprio (mensagens `Message` em bincode), não o protocolo Synergy/Barrier.

---

## 2. Fluxo ideal do utilizador (história única)

1. **Definir papéis** nas duas máquinas: uma **Servidor**, outra **Cliente**.
2. **Mesma chave** (PSK) e **mesma porta** em ambas.
3. No **Servidor**: **Conectar** (fica a escutar).
4. No **Cliente**: IP do servidor (Configurações ou Dispositivos) e **Conectar** no painel (comportamento tipo Barrier: arranque automático da sessão só se ativar **Ligar sessão KVM ao abrir o app** em Configurações).
5. No **Servidor**: se aparecer pedido, **Permitir** ou **Recusar**.
6. Ajustar **posição do monitor remoto** na Matriz de telas; mover o rato até à borda para passar o controlo (comportamento tipo KVM).

Se algo falhar, ver **Configurações → Alinhar a rede** (checklist firewall, mesma LAN, `ping`, etc.).

---

## 3. Checklist de UX (validação manual / QA)

Usar antes de release ou ao alterar Painel, Dispositivos ou Configurações.

### Papel e rede

- [ ] Com **Servidor** selecionado, o endereço do servidor **não** é obrigatório (secção cliente oculta ou coerente).
- [ ] Com **Cliente** selecionado, o campo **Endereço do servidor** está visível e persistido após **Salvar**.
- [ ] **Conectar** no painel reflete estado: **Desconectado** → **Aguardando conexão…** (servidor) ou **Conectando…** / **Reconectando…** (cliente), e texto em `#panelConnStatus` (se existir) atualiza.
- [ ] Cliente **sem IP**: ao **Conectar**, a app **não** fica silenciosa — leva a Dispositivos e abre/expande **Conectar por IP** (`details`).

### Dispositivos

- [ ] **Atualizar** executa descoberta sem erro e atualiza subtítulo / grelha.
- [ ] **+ Manual** abre ou expande o bloco **Conectar por IP** (nativo `<details>` ou botão que define `open = true`).
- [ ] **Conectar** (IP manual) chama `connect_to_peer` e há feedback no log em caso de erro.

### Confiança e sessão

- [ ] Pedido de ligação no servidor mostra modal de aprovação; **Permitir** / **Recusar** / timeout estão alinhados com o backend.
- [ ] Após conectar, Matriz de telas e estatísticas (latência, etc.) atualizam sem precisar mudar de aba.

### Regressões conhecidas a evitar

- [ ] Dois handlers no mesmo `id` para ações diferentes (ex.: **Conectar** do painel vs modal manual).
- [ ] Depender só de `element.style.display === 'none'` para toggles no WebView — preferir `<details>` ou estado explícito.

---

## 4. Referências externas (leitura)

- [Barrier — README / uso](https://github.com/debauchee/barrier): servidor com teclado/rato, clientes com IP, nomes de ecrã na configuração.
- [Deskflow — README](https://github.com/deskflow/deskflow): KVM moderno, TLS, clipboard, Wayland; relação com forks (Barrier, Input Leap, etc.).

---

*Documento gerado para alinhamento de produto e QA. Não substitui documentação de API ou de protocolo binário.*
