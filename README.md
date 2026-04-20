# Movex

> Controle múltiplos computadores com um único teclado e mouse pela rede local.

[![Build Movex](https://github.com/thamiresfm/Movex/actions/workflows/build.yml/badge.svg)](https://github.com/thamiresfm/Movex/actions/workflows/build.yml)

---

## O que é o Movex?

O **Movex** é um aplicativo de KVM por software — semelhante ao Barrier e Deskflow — que permite usar **um único teclado e mouse físicos para controlar múltiplos computadores** na mesma rede local. Basta mover o cursor para a borda da tela e ele "flui" para o próximo computador.


---

## Funcionalidades

| Feature | Status |
|---|---|
| Controle de mouse e teclado entre máquinas | ✅ |
| Transferência de arquivos com verificação SHA-256 | ✅ |
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

### 4. Use

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
