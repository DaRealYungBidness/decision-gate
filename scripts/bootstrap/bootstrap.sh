#!/usr/bin/env bash
# scripts/bootstrap/bootstrap.sh
# =============================================================================
# Module: Decision Gate Clone-and-go Bootstrap
# Description: Create a local venv, install SDK/adapters, and run a hello flow.
# Purpose: Provide a one-command onboarding path for source users.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

VENV_PATH="$REPO_ROOT/.venv/onboarding"
ADAPTERS="none"
VALIDATE="false"
SMOKE="true"
TMP_DIRS=()
SERVER_PID=""

print_usage() {
    cat <<USAGE
Usage: $(basename "$0") [--venv PATH] [--adapters=none|all|LIST] [--validate] [--no-smoke]

Options:
  --venv PATH          Location for the virtual environment (default: .venv/onboarding).
  --adapters=LIST      Comma-separated list of adapters to install
                       (langchain,crewai,autogen,openai_agents) or "all" or "none".
  --validate           Install decision-gate[validation] for schema validation.
  --no-smoke           Skip the hello-flow smoke run.
  -h, --help           Show this help message.
USAGE
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

wait_for_server_ready() {
    local endpoint="$1"
    local deadline=$((SECONDS + 30))
    while [[ $SECONDS -lt $deadline ]]; do
        if "$VENV_PY" - <<PY >/dev/null 2>&1
import json
import urllib.request

payload = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}).encode("utf-8")
req = urllib.request.Request("$endpoint", data=payload, headers={"Content-Type": "application/json"}, method="POST")
with urllib.request.urlopen(req, timeout=2) as resp:
    resp.read()
PY
        then
            return 0
        fi
        sleep 0.5
    done
    return 1
}

cleanup() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" >/dev/null 2>&1 || true
        wait "$SERVER_PID" >/dev/null 2>&1 || true
    fi
    for dir in "${TMP_DIRS[@]}"; do
        rm -rf "$dir"
    done
}

trap cleanup EXIT

while [[ $# -gt 0 ]]; do
    case "$1" in
        --venv)
            VENV_PATH="$2"
            shift 2
            ;;
        --adapters=*)
            ADAPTERS="${1#*=}"
            shift
            ;;
        --validate)
            VALIDATE="true"
            shift
            ;;
        --no-smoke)
            SMOKE="false"
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
    echo "ERROR: Python 3 interpreter not found." >&2
    exit 1
fi

if [[ ! -d "$VENV_PATH" ]]; then
    "${PYTHON[@]}" -m venv "$VENV_PATH"
fi

VENV_PY="$VENV_PATH/bin/python"
VENV_PIP="$VENV_PATH/bin/pip"

export PIP_DISABLE_PIP_VERSION_CHECK=1
export PIP_NO_INPUT=1

"$VENV_PY" -m pip install --upgrade pip

if [[ "$VALIDATE" == "true" ]]; then
    "$VENV_PIP" install -e "$REPO_ROOT/sdks/python[validation]"
else
    "$VENV_PIP" install -e "$REPO_ROOT/sdks/python"
fi

if [[ "$ADAPTERS" != "none" ]]; then
    if [[ "$ADAPTERS" == "all" ]]; then
        ADAPTER_LIST=("langchain" "crewai" "autogen" "openai_agents")
    else
        IFS=',' read -r -a ADAPTER_LIST <<<"$ADAPTERS"
    fi
    for adapter in "${ADAPTER_LIST[@]}"; do
        case "$adapter" in
            langchain)
                "$VENV_PIP" install -e "$REPO_ROOT/adapters/langchain"
                ;;
            crewai)
                "$VENV_PIP" install -e "$REPO_ROOT/adapters/crewai"
                ;;
            autogen)
                "$VENV_PIP" install -e "$REPO_ROOT/adapters/autogen"
                ;;
            openai_agents|openai-agents)
                "$VENV_PIP" install -e "$REPO_ROOT/adapters/openai_agents"
                ;;
            *)
                echo "Unknown adapter: $adapter" >&2
                exit 1
                ;;
        esac
    done
fi

if [[ "$SMOKE" == "true" ]]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "WARNING: cargo not found; skipping smoke run." >&2
    else
        tmp_root="$(mktemp -d)"
        TMP_DIRS+=("$tmp_root")
        port="$("$VENV_PY" - <<'PY'
import socket
sock = socket.socket()
sock.bind(("127.0.0.1", 0))
port = sock.getsockname()[1]
sock.close()
print(port)
PY
)"
        config_path="$tmp_root/decision-gate.toml"
        cat > "$config_path" <<DG_CONFIG
[server]
transport = "http"
mode = "strict"
bind = "127.0.0.1:${port}"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[schema_registry.acl]
allow_local_only = true

[[providers]]
name = "env"
type = "builtin"
[providers.config]
allowlist = ["DEPLOY_ENV"]
denylist = []
max_key_bytes = 255
max_value_bytes = 65536
DG_CONFIG

        log_path="$tmp_root/server.log"
        cargo run -p decision-gate-cli -- serve --config "$config_path" >"$log_path" 2>&1 &
        SERVER_PID=$!
        endpoint="http://127.0.0.1:${port}/rpc"
        if ! wait_for_server_ready "$endpoint"; then
            echo "ERROR: MCP server failed to start." >&2
            tail -n 200 "$log_path" >&2 || true
            exit 1
        fi
        export DG_ENDPOINT="$endpoint"
        export DEPLOY_ENV="production"
        "$VENV_PY" "$REPO_ROOT/examples/python/precheck.py"
    fi
fi

cat <<EOF
Bootstrap complete.
Venv: $VENV_PATH

Next:
  source "$VENV_PATH/bin/activate"
  scripts/adapters/adapter_tests.sh --all
EOF
