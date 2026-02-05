#!/usr/bin/env bash
# scripts/agentic/agentic_harness.sh
# =============================================================================
# Module: Decision Gate Agentic Harness Runner
# Description: Runs the agentic flow harness in deterministic mode.
# Purpose: Local entry point for agentic scenario validation.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

MODE="deterministic"
SCENARIOS=""
DRIVERS=""
UPDATE_EXPECTED="false"
BOOTSTRAP="false"
VENV_PATH=""

print_usage() {
    cat <<USAGE
Usage: $(basename "$0") [--mode=deterministic] [--scenarios=...] [--drivers=...] [--update-expected] [--bootstrap] [--venv=PATH]

Options:
  --mode=deterministic   Run deterministic harness (default).
  --scenarios=LIST       Comma-separated scenario ids to run.
  --drivers=LIST         Comma-separated driver list.
  --update-expected      Update expected runpack hashes.
  --bootstrap            Install Python dependencies for the driver matrix.
  --venv=PATH            Use a specific venv path (default: .venv/agentic-harness).
  -h, --help             Show this help message.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode=*)
            MODE="${1#*=}"
            shift
            ;;
        --scenarios=*)
            SCENARIOS="${1#*=}"
            shift
            ;;
        --drivers=*)
            DRIVERS="${1#*=}"
            shift
            ;;
        --update-expected)
            UPDATE_EXPECTED="true"
            shift
            ;;
        --bootstrap)
            BOOTSTRAP="true"
            shift
            ;;
        --venv=*)
            VENV_PATH="${1#*=}"
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

if [[ "$MODE" != "deterministic" ]]; then
    echo "Unsupported mode: $MODE (only deterministic is implemented)" >&2
    exit 1
fi

export DECISION_GATE_AGENTIC_SCENARIOS="$SCENARIOS"
export DECISION_GATE_AGENTIC_DRIVERS="$DRIVERS"
if [[ "$UPDATE_EXPECTED" == "true" ]]; then
    export UPDATE_AGENTIC_EXPECTED=1
fi

cd "$REPO_ROOT"

if [[ -z "$VENV_PATH" ]]; then
    VENV_PATH="$REPO_ROOT/.venv/agentic-harness"
fi

if [[ "$BOOTSTRAP" == "true" ]]; then
    "$SCRIPT_DIR/agentic_harness_bootstrap.sh" --venv "$VENV_PATH"
fi

if [[ -d "$VENV_PATH" ]]; then
    export VIRTUAL_ENV="$VENV_PATH"
    export PATH="$VENV_PATH/bin:$PATH"
    export PYTHONNOUSERSITE=1
fi

if [[ -z "${DECISION_GATE_SYSTEM_TEST_RUN_ROOT:-}" ]]; then
    timestamp="$(date -u +%Y%m%d_%H%M%S)"
    export DECISION_GATE_SYSTEM_TEST_RUN_ROOT="$REPO_ROOT/.tmp/system-tests/agentic_flow_harness_deterministic/run_${timestamp}_$$"
fi

cargo test -p system-tests --features system-tests --test agentic -- --exact agentic_harness::agentic_flow_harness_deterministic
