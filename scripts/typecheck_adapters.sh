#!/usr/bin/env bash
# scripts/typecheck_adapters.sh
# =============================================================================
# Module: Decision Gate Adapter Typecheck Gate
# Description: Run Pyright strict typing checks for adapters + adapter scripts.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PYRIGHT_BIN="$REPO_ROOT/node_modules/.bin/pyright"
if [[ ! -x "$PYRIGHT_BIN" ]]; then
    if command -v pyright >/dev/null 2>&1; then
        PYRIGHT_BIN="$(command -v pyright)"
    else
        echo "ERROR: pyright not installed. Run npm install or npm run typecheck:adapters." >&2
        exit 1
    fi
fi

exec "$PYRIGHT_BIN" --project "$REPO_ROOT/pyrightconfig.json"
