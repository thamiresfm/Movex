# Movex

> Controle múltiplos computadores com um único teclado e mouse pela rede local.

[![CI](https://github.com/thamiresfm/Movex/actions/workflows/ci.yml/badge.svg)](https://github.com/thamiresfm/Movex/actions/workflows/ci.yml)
[![Release](https://github.com/thamiresfm/Movex/actions/workflows/release.yml/badge.svg)](https://github.com/thamiresfm/Movex/releases/latest)

---

## Documentação

| Documento | Descrição |
|-----------|-----------|
| [Guia de Configuração](docs/CONFIGURACAO.md) | Todos os campos do `config.json`, passo a passo, permissões macOS/Windows, firewall, TLS, clipboard e resolução de problemas |
| [Registro de Bugs](docs/bugs/BUGS.md) | Bugs conhecidos por versão e respetivo status |
| [Modelo KVM & UX](docs/MODELO_KVM_E_UX.md) | Referência interna: comparação com Barrier/Deskflow e checklist de QA |

---

## O que é o Movex?

O **Movex** é um aplicativo de KVM por software — semelhante ao Barrier e Deskflow — que permite usar **um único teclado e mouse físicos para controlar múltiplos computadores** na mesma rede local. Basta mover o cursor para a borda da tela e ele "flui" para o próximo computador.


---

## Funcionalidades

| Feature | Status |
|---|---|
| Controle de mouse e teclado entre máquinas | ✅ |
| Sincronização de clipboard (texto) | ✅ |
| Descoberta automática via mDNS | ✅ |
| Conexão segura com TLS 1.3 | ✅ |
| Reconexão automática com backoff exponencial | ✅ |
| Interface com mapa de telas e logs em tempo real | ✅ |
| Build para macOS (Universal Binary) | ✅ |
| Build para Windows (MSI + NSIS) | ✅ |

---

## Plataformas

| SO | Versão mínima | Arquitetura |
|---|---|---|
| macOS | 12 (Monterey) | Apple Silicon + Intel (Universal) |
| Windows | 10 build 1903+ | x64 |

---

## Primeiros passos

### 1. Instale o Movex nos dois computadores

Baixe o instalador na página de [Releases](https://github.com/thamiresfm/Movex/releases):
- **macOS** → `.dmg`
- **Windows** → `.msi` ou `.exe`

### 2. Configure o Servidor (computador com o teclado/mouse físico)

1. Abra o Movex
2. Selecione **Servidor (Computador Principal)**
3. Anote a **Chave de Segurança** gerada

### 3. Configure o Cliente (computador que vai receber o controle)

1. Abra o Movex
2. Selecione **Cliente (Computador Remoto)**
3. Digite o mesmo servidor ou deixe descobrir automaticamente
4. Use a mesma **Chave de Segurança**

### 4. Conceda as permissões necessárias

O Movex precisa de permissão do sistema para capturar e injetar mouse/teclado.  
**Sem essas permissões o cursor não passa entre os computadores.**

#### macOS

| Permissão | Onde conceder | Necessária para |
|-----------|--------------|----------------|
| **Acessibilidade** | Ajustes do Sistema → Privacidade e Segurança → Acessibilidade → ativar Movex | Capturar mouse/teclado no Servidor e injetar eventos no Cliente |
| **Monitorização de Entrada** | Ajustes do Sistema → Privacidade e Segurança → Monitorização de Entrada → ativar Movex | Capturar teclas globalmente (atalhos, tecla de bloqueio) |

> **Depois de conceder:** feche e reabra o Movex para que as permissões entrem em vigor.  
> **Após atualizar o app:** as permissões podem precisar ser reativadas (o macOS âncora a permissão ao binário).  
> Se o botão «Corrigir» aparecer no painel, clique nele — ele redefine automaticamente a entrada TCC stale.

#### Windows

| Permissão | Como funciona |
|-----------|--------------|
| **Regras de Firewall** | Na 1.ª ligação como Servidor, o Movex solicita UAC para abrir a porta 24800. Aceite o prompt de Controlo de Conta de Utilizador. |
| **Execução normal** | Não é necessário executar como Administrador no dia a dia — apenas o primeiro UAC do Firewall requer elevação. |

> Se o prompt UAC não aparecer ou as regras não forem aplicadas: vá a **Configurações → Permissões → Aplicar regras no Firewall** dentro do Movex.

Veja mais detalhes em [Guia de Configuração — Permissões do sistema](docs/CONFIGURACAO.md#14-permissões-do-sistema).

---

### 5. Use

Mova o cursor para a **borda da tela** na direção do outro computador — o controle passa automaticamente.

---

## Desenvolvimento

### Pré-requisitos

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js 20+
# macOS: brew install node
# Windows: nodejs.org

# Tauri CLI
cargo install tauri-cli --version "^2"
```

### Rodar em modo desenvolvimento

```bash
git clone https://github.com/thamiresfm/Movex.git
cd Movex
npm install
npm run tauri dev
```

### Executar testes

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

### Build para produção

```bash
# macOS (Universal Binary: Apple Silicon + Intel)
npm run tauri build -- --target universal-apple-darwin

# Windows
npm run tauri build
```

---

## Arquitetura

```
Movex
├── src-tauri/          # Core Rust (backend)
│   └── src/
│       ├── core/       # Servidor e Cliente TCP/TLS
│       ├── input/      # Captura e injeção de mouse/teclado
│       ├── network/    # Protocolo, transporte TLS, mDNS
│       ├── screen/     # Detecção de borda, layout de telas
│       ├── transfer/   # Transferência de arquivos SHA-256
│       ├── clipboard/  # Sincronização de clipboard
│       └── config/     # Configurações e autostart
└── src/                # UI (TypeScript)
    └── components/
        ├── Setup.ts    # Assistente de configuração inicial
        └── Dashboard.ts # Painel principal
```

**Stack:** Rust 1.78+ · Tauri 2 · TLS 1.3 (rustls) · bincode · mDNS · TypeScript · Vite

---

## CI/CD

O repositório usa **GitHub Actions** para build automático:

- **Push na `main`** → executa testes + gera builds para macOS e Windows
- **Tag `v*`** → publica automaticamente um [Release](https://github.com/thamiresfm/Movex/releases) com os instaladores

---

## Licença

MIT — veja [LICENSE](LICENSE) para detalhes.
