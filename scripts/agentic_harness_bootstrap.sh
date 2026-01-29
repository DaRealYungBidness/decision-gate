#!/usr/bin/env bash
# scripts/agentic_harness_bootstrap.sh
# =============================================================================
# Module: Decision Gate Agentic Harness Bootstrap
# Description: Creates a Python venv + installs agentic harness dependencies.
# Purpose: Provide a deterministic, repeatable setup for the driver matrix.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

VENV_PATH="$REPO_ROOT/.venv/agentic-harness"
RESET="false"

print_usage() {
    cat <<USAGE
Usage: $(basename "$0") [--venv PATH] [--reset]

Options:
  --venv PATH   Location for the virtual environment (default: .venv/agentic-harness).
  --reset       Remove the existing venv before reinstalling.
  -h, --help    Show this help message.
USAGE
}

resolve_python() {
    if [[ -n "${DG_AGENTIC_PYTHON:-}" ]]; then
        if command -v "$DG_AGENTIC_PYTHON" >/dev/null 2>&1; then
            PYTHON=("$DG_AGENTIC_PYTHON")
            return 0
        fi
        echo "ERROR: DG_AGENTIC_PYTHON not found: $DG_AGENTIC_PYTHON" >&2
        exit 1
    fi
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

while [[ $# -gt 0 ]]; do
    case "$1" in
        --venv)
            VENV_PATH="$2"
            shift 2
            ;;
        --reset)
            RESET="true"
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

if ! resolve_python; then
    echo "ERROR: Python 3 interpreter not found for agentic harness bootstrap." >&2
    exit 1
fi

if [[ "$RESET" == "true" && -d "$VENV_PATH" ]]; then
    rm -rf "$VENV_PATH"
fi

if [[ ! -d "$VENV_PATH" ]]; then
    "${PYTHON[@]}" -m venv "$VENV_PATH"
fi

VENV_PY="$VENV_PATH/bin/python"
VENV_PIP="$VENV_PATH/bin/pip"

export PIP_DISABLE_PIP_VERSION_CHECK=1
export PIP_NO_INPUT=1

"$VENV_PY" -m pip install --upgrade pip

REQS_PATH="$REPO_ROOT/system-tests/requirements-agentic.txt"
if [[ -f "$REQS_PATH" ]]; then
    "$VENV_PIP" install -r "$REQS_PATH"
fi

"$VENV_PIP" install -e "$REPO_ROOT/sdks/python"
"$VENV_PIP" install -e "$REPO_ROOT/adapters/langchain"
"$VENV_PIP" install -e "$REPO_ROOT/adapters/crewai"
"$VENV_PIP" install -e "$REPO_ROOT/adapters/autogen"
"$VENV_PIP" install -e "$REPO_ROOT/adapters/openai_agents"

if ! command -v node >/dev/null 2>&1; then
    echo "WARNING: node not found; TypeScript driver will be skipped." >&2
else
    if ! node --experimental-strip-types -e "process.exit(typeof fetch === 'function' ? 0 : 2)" >/dev/null 2>&1; then
        echo "WARNING: node lacks --experimental-strip-types or fetch; Node 18+ required." >&2
    fi
fi

cat <<EOF
Agentic harness dependencies installed.
Venv: $VENV_PATH

Next:
  export PATH="$VENV_PATH/bin:\$PATH"
  export PYTHONNOUSERSITE=1
  scripts/agentic_harness.sh --mode=deterministic
EOF
