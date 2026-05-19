#!/usr/bin/env bash
# Configura o ambiente de desenvolvimento local.
# Execute uma vez após clonar: bash scripts/setup-dev.sh
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Apontar git para os hooks do repositório
git config core.hooksPath .githooks
chmod +x "$REPO_ROOT/.githooks/pre-push"

echo "Pronto. Hook pre-push ativado: clippy + testes rodam antes de cada push."
