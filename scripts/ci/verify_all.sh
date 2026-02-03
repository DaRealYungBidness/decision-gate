#!/usr/bin/env bash
# scripts/ci/verify_all.sh
# =============================================================================
# Module: Decision Gate Verification Orchestrator
# Description: Runs generation checks, unit tests, and optional system tests.
# Purpose: Provide a single entry point for CI or local gating.
# Dependencies: bash, cargo, python3 (optional for system-tests registry runner)
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

SYSTEM_TESTS="none"
PACKAGE_DRY_RUN="none"
ADAPTER_TESTS="none"
AGENTIC_HARNESS="none"
DOCS_RUN="none"
DOCS_LINT="none"
DOCS_LINKS="none"
SBOM="none"
PYTHON_FORMAT="none"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [--system-tests[=p0|p1|all|quick]] [--package-dry-run[=python|typescript|all]] [--adapter-tests[=...]]

Options:
  --system-tests           Run P0 system-tests via scripts/system_tests/test_runner.py.
  --system-tests=p0        Run P0 system-tests (default when flag is present).
  --system-tests=p1        Run P0 and P1 system-tests.
  --system-tests=all       Run all registered system-tests.
  --system-tests=quick     Run quick system-tests (registry quick categories).
  --package-dry-run        Run Python + TypeScript packaging dry-runs.
  --package-dry-run=python Run Python packaging dry-run only.
  --package-dry-run=typescript
                           Run TypeScript packaging dry-run only.
  --package-dry-run=all    Run both packaging dry-runs.
  --adapter-tests          Run all adapter tests via scripts/adapters/adapter_tests.sh.
  --adapter-tests=LIST     Run adapter tests for a comma-separated list
                           (langchain,crewai,autogen,openai_agents).
  --agentic-harness        Run the agentic flow harness (deterministic mode).
  --docs-run               Execute runnable documentation blocks (fast level).
  --docs-run=all            Execute all runnable documentation blocks (fast + slow).
  --docs-lint              Run Markdown linting for Docs/ and README.md.
  --docs-links             Ensure [F:...] refs are linkified in Docs/ and README.md.
  --python-format          Run Black in check mode on Python sources.
  --python-format=check    Same as --python-format.
  --python-format=fix      Run Black to rewrite Python sources in place.
  --sbom                   Generate a dependency SBOM (cargo-sbom).
  --sbom=FILE              Generate SBOM to a specific output path.
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
        --docs-run)
            DOCS_RUN="fast"
            shift
            ;;
        --docs-run=*)
            DOCS_RUN="${1#*=}"
            shift
            ;;
        --docs-lint)
            DOCS_LINT="enabled"
            shift
            ;;
        --docs-links)
            DOCS_LINKS="enabled"
            shift
            ;;
        --python-format)
            PYTHON_FORMAT="check"
            shift
            ;;
        --python-format=*)
            PYTHON_FORMAT="${1#*=}"
            shift
            ;;
        --sbom)
            SBOM="default"
            shift
            ;;
        --sbom=*)
            SBOM="${1#*=}"
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

PYTHON=()
if ! resolve_python; then
    echo "ERROR: Python 3 interpreter not found for docs verification." >&2
    exit 1
fi

run "$SCRIPT_DIR/generate_all.sh" --check
run cargo test --workspace --exclude system-tests
run "${PYTHON[@]}" "$REPO_ROOT/scripts/docs/docs_verify.py"

if [[ "$PYTHON_FORMAT" != "none" ]]; then
    if ! "${PYTHON[@]}" -m black --version >/dev/null 2>&1; then
        echo "ERROR: black is not installed. Install with: python -m pip install black" >&2
        exit 1
    fi
    case "$PYTHON_FORMAT" in
        check)
            run "${PYTHON[@]}" -m black --check .
            ;;
        fix)
            run "${PYTHON[@]}" -m black .
            ;;
        *)
            echo "Unknown python-format mode: $PYTHON_FORMAT" >&2
            print_usage
            exit 1
            ;;
    esac
fi

if [[ "$SYSTEM_TESTS" != "none" ]]; then
    case "$SYSTEM_TESTS" in
        p0)
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/system_tests/test_runner.py" --priority P0
            ;;
        p1)
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/system_tests/test_runner.py" --priority P0
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/system_tests/test_runner.py" --priority P1
            ;;
        quick)
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/system_tests/test_runner.py" --quick-only
            ;;
        all)
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/system_tests/test_runner.py"
            ;;
        *)
            echo "Unknown system-tests mode: $SYSTEM_TESTS" >&2
            print_usage
            exit 1
            ;;
    esac
fi

if [[ "$DOCS_RUN" != "none" ]]; then
    case "$DOCS_RUN" in
        fast)
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/docs/docs_verify.py" --run --level=fast
            ;;
        all)
            run "${PYTHON[@]}" "$REPO_ROOT/scripts/docs/docs_verify.py" --run --level=all
            ;;
        *)
            echo "Unknown docs run mode: $DOCS_RUN" >&2
            print_usage
            exit 1
            ;;
    esac
fi

if [[ "$DOCS_LINT" != "none" ]]; then
    run npm run docs:lint
fi

if [[ "$DOCS_LINKS" != "none" ]]; then
    run npm run docs:linkify:check
fi

if [[ "$SBOM" != "none" ]]; then
    if [[ "$SBOM" == "default" ]]; then
        run "$SCRIPT_DIR/sbom.sh"
    else
        run "$SCRIPT_DIR/sbom.sh" --output="$SBOM"
    fi
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
            run "$REPO_ROOT/scripts/adapters/adapter_tests.sh" --all
            ;;
        *)
            run "$REPO_ROOT/scripts/adapters/adapter_tests.sh" --frameworks="$ADAPTER_TESTS"
            ;;
    esac
fi

if [[ "$AGENTIC_HARNESS" != "none" ]]; then
    run "$REPO_ROOT/scripts/agentic/agentic_harness.sh" --mode="$AGENTIC_HARNESS"
fi

echo "Verification complete."
