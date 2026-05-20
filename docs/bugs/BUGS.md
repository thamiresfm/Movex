# Registro de Bugs — Movex

Cada bug tem um ID único no formato `BUG-<versão>-<número>`.
Para reportar um novo bug, copie `template.md`, preencha e adicione uma entrada aqui.

---

## v0.1.0

| ID | Título | Componente | Status |
|----|--------|-----------|--------|
| BUG-010-001 | Cursor aparece no lado invertido ao voltar do Mac para Windows | Windows · `unlock_cursor` | ✅ Corrigido em v0.1.0 |
| BUG-010-002 | Teclado não funciona no Mac após cursor cruzar do Windows | macOS · `inject` keyboard | ✅ Corrigido em v0.1.0 |
| BUG-010-003 | `active_screen_remote` fica `true` se conexão cair com cursor no remoto | Servidor · `server.rs` | ✅ Corrigido em v0.1.0 |
| BUG-010-004 | Clipboard compartilhava imagens além de texto | `clipboard/sync.rs` | ✅ Corrigido em v0.1.0 |
| BUG-010-005 | Status "Conectando…" persiste mesmo após conexão estabelecida | Frontend · `Dashboard.ts` | ✅ Corrigido em v0.1.0 |

---

## v0.1.1

| ID | Título | Componente | Status |
|----|--------|-----------|--------|
| BUG-011-001 | Servidor ignorava HMAC — qualquer PSK era aceita | `core/server.rs` · handshake | ✅ Corrigido em v0.1.1 |
| BUG-011-002 | XSS em mensagens de log via `innerHTML` sem escape | Frontend · `Logs.ts` | ✅ Corrigido em v0.1.1 |
| BUG-011-003 | `verify_hmac` com PSK hex inválida usava fallback silencioso | `core/auth.rs` | ✅ Corrigido em v0.1.1 |
| BUG-011-004 | `peer_hostname` sem limite de tamanho em notificações | `core/server.rs` | ✅ Corrigido em v0.1.1 |
| BUG-011-005 | Script PS1 de firewall com nome fixo sujeito a TOCTOU | `permissions.rs` · Windows | ✅ Corrigido em v0.1.1 |

---

## v0.1.2

| ID | Título | Componente | Status |
|----|--------|-----------|--------|
| BUG-012-001 | Teclas modificadoras (Shift/Ctrl/Alt/Cmd) não encaminhadas quando macOS é servidor | macOS · `FlagsChanged` ausente do CGEventTap | ✅ Corrigido em v0.1.2 |
| BUG-012-002 | Teclas do numpad e ABNT2 (VK_NUMPAD0-9, VK_ABNT_C1) descartadas | `input/keycodes.rs` · `vk_to_hid` incompleto | ✅ Corrigido em v0.1.2 |
| BUG-012-003 | Modificador prematuramente solto no destino ao largar qualquer tecla | Windows · `WindowsInjector` · modifier sandwich indevido | ✅ Corrigido em v0.1.2 |

---

## Como adicionar um bug

1. Copie `docs/bugs/template.md` para `docs/bugs/BUG-<versão>-<número>.md`
2. Preencha todos os campos
3. Adicione uma linha na tabela da versão correspondente acima
4. Faça commit: `fix(bugs): registar BUG-XXX-YYY`
