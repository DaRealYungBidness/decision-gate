#!/usr/bin/env bash
# scripts/verify_all.sh
# =============================================================================
# Module: Decision Gate Verification Orchestrator
# Description: Runs generation checks, unit tests, and optional system tests.
# Purpose: Provide a single entry point for CI or local gating.
# Dependencies: bash, cargo, python3 (optional for system-tests registry runner)
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SYSTEM_TESTS="none"
PACKAGE_DRY_RUN="none"
ADAPTER_TESTS="none"
AGENTIC_HARNESS="none"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [--system-tests[=p0|p1|all|quick]] [--package-dry-run[=python|typescript|all]] [--adapter-tests[=...]]

Options:
  --system-tests           Run P0 system-tests via scripts/test_runner.py.
  --system-tests=p0        Run P0 system-tests (default when flag is present).
  --system-tests=p1        Run P0 and P1 system-tests.
  --system-tests=all       Run all registered system-tests.
  --system-tests=quick     Run quick system-tests (registry quick categories).
  --package-dry-run        Run Python + TypeScript packaging dry-runs.
  --package-dry-run=python Run Python packaging dry-run only.
  --package-dry-run=typescript
                           Run TypeScript packaging dry-run only.
  --package-dry-run=all    Run both packaging dry-runs.
  --adapter-tests          Run all adapter tests via scripts/adapter_tests.sh.
  --adapter-tests=LIST     Run adapter tests for a comma-separated list
                           (langchain,crewai,autogen,openai_agents).
  --agentic-harness        Run the agentic flow harness (deterministic mode).
  -h, --help               Show this help message.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --system-tests)
            SYSTEM_TESTS="p0"
            shift
            ;;
        --system-tests=*)
            SYSTEM_TESTS="${1#*=}"
            shift
            ;;
        --package-dry-run)
            PACKAGE_DRY_RUN="all"
            shift
            ;;
        --package-dry-run=*)
            PACKAGE_DRY_RUN="${1#*=}"
            shift
            ;;
        --adapter-tests)
            ADAPTER_TESTS="all"
            shift
            ;;
        --adapter-tests=*)
            ADAPTER_TESTS="${1#*=}"
            shift
            ;;
        --agentic-harness)
            AGENTIC_HARNESS="deterministic"
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

cd "$REPO_ROOT"

run "$SCRIPT_DIR/generate_all.sh" --check
run cargo test --workspace --exclude system-tests

if [[ "$SYSTEM_TESTS" != "none" ]]; then
    PYTHON=()
    if ! resolve_python; then
        echo "ERROR: Python 3 interpreter not found for system-tests runner." >&2
        exit 1
    fi
    case "$SYSTEM_TESTS" in
        p0)
            run "${PYTHON[@]}" scripts/test_runner.py --priority P0
            ;;
        p1)
            run "${PYTHON[@]}" scripts/test_runner.py --priority P0
            run "${PYTHON[@]}" scripts/test_runner.py --priority P1
            ;;
        quick)
            run "${PYTHON[@]}" scripts/test_runner.py --quick-only
            ;;
        all)
            run "${PYTHON[@]}" scripts/test_runner.py
            ;;
        *)
            echo "Unknown system-tests mode: $SYSTEM_TESTS" >&2
            print_usage
            exit 1
            ;;
    esac
fi

if [[ "$PACKAGE_DRY_RUN" != "none" ]]; then
    case "$PACKAGE_DRY_RUN" in
        python|typescript|all)
            run "$SCRIPT_DIR/package_dry_run.sh" "--$PACKAGE_DRY_RUN"
            ;;
        *)
            echo "Unknown package dry-run mode: $PACKAGE_DRY_RUN" >&2
            print_usage
            exit 1
            ;;
    esac
fi

if [[ "$ADAPTER_TESTS" != "none" ]]; then
    case "$ADAPTER_TESTS" in
        all)
            run "$SCRIPT_DIR/adapter_tests.sh" --all
            ;;
        *)
            run "$SCRIPT_DIR/adapter_tests.sh" --frameworks="$ADAPTER_TESTS"
            ;;
    esac
fi

if [[ "$AGENTIC_HARNESS" != "none" ]]; then
    run "$SCRIPT_DIR/agentic_harness.sh" --mode="$AGENTIC_HARNESS"
fi

echo "Verification complete."
