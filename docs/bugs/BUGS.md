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

## Como adicionar um bug

1. Copie `docs/bugs/template.md` para `docs/bugs/BUG-<versão>-<número>.md`
2. Preencha todos os campos
3. Adicione uma linha na tabela da versão correspondente acima
4. Faça commit: `fix(bugs): registar BUG-XXX-YYY`
