#!/usr/bin/env bash
# scripts/package_dry_run.sh
# =============================================================================
# Module: Decision Gate Package Dry-Run
# Description: Build and install SDK packages without publishing.
# Purpose: Validate packaging viability for Python and TypeScript SDKs.
# Dependencies: bash, python3 (venv + pip), npm/node (for TypeScript)
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

MODE="all"
TMP_DIRS=()

cleanup() {
    for dir in "${TMP_DIRS[@]}"; do
        rm -rf "$dir"
    done
}

trap cleanup EXIT

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [--python|--typescript|--all]

Options:
  --python       Run Python packaging dry-run only.
  --typescript   Run TypeScript packaging dry-run only.
  --all          Run both Python + TypeScript packaging dry-runs (default).
  -h, --help     Show this help message.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --python)
            MODE="python"
            shift
            ;;
        --typescript)
            MODE="typescript"
            shift
            ;;
        --all)
            MODE="all"
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            print_usage
            exit 1
            ;;
    esac
done

run() {
    echo "+ $*"
    "$@"
}

resolve_python() {
    if command -v python3 >/dev/null 2>&1; then
        PYTHON=(python3)
        return 0
    fi
    if command -v python >/dev/null 2>&1; then
        PYTHON=(python)
        return 0
    fi
    if command -v py >/dev/null 2>&1; then
        PYTHON=(py -3)
        return 0
    fi
    return 1
}

require_node_npm() {
    if ! command -v node >/dev/null 2>&1; then
        echo "ERROR: node is required for TypeScript packaging dry-run." >&2
        exit 1
    fi
    if ! command -v npm >/dev/null 2>&1; then
        echo "ERROR: npm is required for TypeScript packaging dry-run." >&2
        exit 1
    fi
}

python_dry_run() {
    if ! resolve_python; then
        echo "ERROR: Python 3 interpreter not found for Python packaging dry-run." >&2
        exit 1
    fi

    local tmp_root
    tmp_root="$(mktemp -d)"
    TMP_DIRS+=("$tmp_root")

    run "${PYTHON[@]}" -m venv "$tmp_root/venv"
    local venv_py="$tmp_root/venv/bin/python"
    local venv_pip="$tmp_root/venv/bin/pip"

    run "$venv_py" -m pip install --upgrade pip build
    run "$venv_py" -m build --sdist --wheel --outdir "$tmp_root/python/dist" \
        "$REPO_ROOT/sdks/python"

    run "$venv_pip" install "$tmp_root/python/dist/"*.whl
    run "$venv_py" - <<'PY'
import decision_gate
from decision_gate import DecisionGateClient, SchemaValidationError, validate_schema

assert DecisionGateClient is not None
assert SchemaValidationError is not None
assert validate_schema is not None
print("python package import ok")
PY
}

typescript_dry_run() {
    require_node_npm

    local tmp_root
    tmp_root="$(mktemp -d)"
    TMP_DIRS+=("$tmp_root")

    run npm exec --yes --package typescript@5.7.0 -- tsc -p "$REPO_ROOT/sdks/typescript/tsconfig.json"
    run npm pack --pack-destination "$tmp_root/typescript" "$REPO_ROOT/sdks/typescript"

    local tarball
    tarball="$(ls "$tmp_root/typescript"/decision-gate-*.tgz | head -n 1)"
    if [[ -z "$tarball" ]]; then
        echo "ERROR: npm pack did not produce a tarball." >&2
        exit 1
    fi

    local proj="$tmp_root/typescript/proj"
    mkdir -p "$proj"
    cat > "$proj/package.json" <<'EOF'
{
  "name": "dg-packaging-dry-run",
  "private": true,
  "type": "module"
}
EOF
    run npm install "$tarball" --prefix "$proj"
    run node --input-type=module -e "import { DecisionGateClient, validateScenarioDefineRequestWithAjv } from 'decision-gate'; console.log(typeof DecisionGateClient, typeof validateScenarioDefineRequestWithAjv);"
}

cd "$REPO_ROOT"

case "$MODE" in
    python)
        python_dry_run
        ;;
    typescript)
        typescript_dry_run
        ;;
    all)
        python_dry_run
        typescript_dry_run
        ;;
    *)
        echo "Unknown mode: $MODE" >&2
        print_usage
        exit 1
        ;;
esac

echo "Packaging dry-run complete."
