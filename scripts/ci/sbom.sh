#!/usr/bin/env bash
# scripts/ci/sbom.sh
# =============================================================================
# Module: Decision Gate SBOM Generator
# Description: Generates a dependency SBOM for the workspace using cargo-sbom.
# Purpose: Provide a minimal, deterministic SBOM for release validation.
# Dependencies: bash, cargo, cargo-sbom
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

OUTPUT_FILE="${REPO_ROOT}/.tmp/ci/sbom/decision-gate.sbom.spdx.json"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [--output=FILE]

Options:
  --output=FILE  Output path for the SBOM (default: $OUTPUT_FILE)
  -h, --help     Show this help message.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output=*)
            OUTPUT_FILE="${1#*=}"
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
    echo "+ $*" >&2
    "$@"
}

cd "$REPO_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo is required to generate SBOMs." >&2
    exit 1
fi

if ! cargo sbom --help >/dev/null 2>&1; then
    cat <<EOF >&2
ERROR: cargo-sbom is not installed.

Install with:
  cargo install cargo-sbom --locked
EOF
    exit 1
fi

OUTPUT_DIR="$(dirname "$OUTPUT_FILE")"
mkdir -p "$OUTPUT_DIR"

run cargo sbom > "$OUTPUT_FILE"
echo "SBOM written to $OUTPUT_FILE"
