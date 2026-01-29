#!/usr/bin/env bash
# scripts/generate_all.sh
# =============================================================================
# Module: Decision Gate Generation Orchestrator
# Description: Runs the contract + SDK generation pipeline for OSS artifacts.
# Purpose: Provide one-command regeneration of generated artifacts with a check mode.
# Dependencies: bash, cargo
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CHECK_MODE="false"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [--check]

Options:
  --check    Verify generated artifacts match committed outputs.
  -h, --help Show this help message.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_MODE="true"
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

cd "$REPO_ROOT"

if [[ "$CHECK_MODE" == "true" ]]; then
    run cargo run -p decision-gate-contract -- check
    run cargo run -p decision-gate-sdk-gen -- check
    echo "Generation verification complete."
else
    run cargo run -p decision-gate-contract -- generate
    run cargo run -p decision-gate-sdk-gen -- generate
    echo "Generated artifacts refreshed."
fi
