#!/usr/bin/env bash
# scripts/adapter_tests.sh
# =============================================================================
# Module: Decision Gate Adapter Test Harness
# Description: Opt-in adapter smoke tests against a local MCP server.
# Purpose: Validate framework adapters and examples without publishing.
# Dependencies: bash, python3 (venv + pip), cargo (optional, for server spawn)
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

FRAMEWORKS=()
ENDPOINT=""
VALIDATE="false"
TMP_DIRS=()
SERVER_PID=""

print_usage() {
    cat <<USAGE
Usage: $(basename "$0") [--all] [--frameworks=langchain,crewai,autogen,openai_agents] [--endpoint URL] [--validate]

Options:
  --all                     Run all adapter examples (default).
  --frameworks=LIST         Comma-separated list of adapters to test.
                           Values: langchain, crewai, autogen, openai_agents (openai-agents alias)
  --endpoint URL            Use an existing DG MCP endpoint (skip server spawn).
  --validate                Enable runtime JSON Schema validation (installs jsonschema).
  -h, --help                Show this help message.
USAGE
}

add_framework() {
    local name="$1"
    case "$name" in
        langchain|crewai|autogen)
            FRAMEWORKS+=("$name")
            ;;
        openai_agents|openai-agents)
            FRAMEWORKS+=("openai_agents")
            ;;
        *)
            echo "Unknown framework: $name" >&2
            exit 1
            ;;
    esac
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all)
            FRAMEWORKS=("langchain" "crewai" "autogen" "openai_agents")
            shift
            ;;
        --frameworks=*)
            IFS=',' read -r -a items <<<"${1#*=}"
            FRAMEWORKS=()
            for item in "${items[@]}"; do
                add_framework "$item"
            done
            shift
            ;;
        --endpoint)
            ENDPOINT="$2"
            shift 2
            ;;
        --validate)
            VALIDATE="true"
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

if [[ ${#FRAMEWORKS[@]} -eq 0 ]]; then
    FRAMEWORKS=("langchain" "crewai" "autogen" "openai_agents")
fi

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

wait_for_server_ready() {
    local endpoint="$1"
    local deadline=$((SECONDS + 30))
    while [[ $SECONDS -lt $deadline ]]; do
        if "${PYTHON[@]}" - <<PY >/dev/null 2>&1
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

spawn_server() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "ERROR: cargo is required to spawn a local MCP server." >&2
        exit 1
    fi

    local tmp_root
    tmp_root="$(mktemp -d)"
    TMP_DIRS+=("$tmp_root")

    local port
    port="$(${PYTHON[@]} - <<'PY'
import socket
sock = socket.socket()
sock.bind(("127.0.0.1", 0))
port = sock.getsockname()[1]
sock.close()
print(port)
PY
)"

    local config_path="$tmp_root/decision-gate.toml"
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

[[server.auth.principals]]
subject = "stdio"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[[providers]]
name = "env"
type = "builtin"
[providers.config]
allowlist = ["DEPLOY_ENV"]
DG_CONFIG

    local log_path="$tmp_root/server.log"
    run cargo run -p decision-gate-cli -- serve --config "$config_path" >"$log_path" 2>&1 &
    SERVER_PID=$!

    ENDPOINT="http://127.0.0.1:${port}/rpc"
    if ! wait_for_server_ready "$ENDPOINT"; then
        echo "ERROR: MCP server failed to start." >&2
        tail -n 200 "$log_path" >&2 || true
        exit 1
    fi
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

if ! resolve_python; then
    echo "ERROR: Python 3 interpreter not found for adapter tests." >&2
    exit 1
fi

if [[ -z "$ENDPOINT" ]]; then
    if [[ -n "${DG_ENDPOINT:-}" ]]; then
        ENDPOINT="$DG_ENDPOINT"
    else
        spawn_server
    fi
fi

venv_root="$(mktemp -d)"
TMP_DIRS+=("$venv_root")

run "${PYTHON[@]}" -m venv "$venv_root/venv"
VENV_PY="$venv_root/venv/bin/python"
VENV_PIP="$venv_root/venv/bin/pip"

export PIP_DISABLE_PIP_VERSION_CHECK=1
export PIP_NO_INPUT=1

run "$VENV_PY" -m pip install --upgrade pip

if [[ "$VALIDATE" == "true" ]]; then
    run "$VENV_PIP" install -e "$REPO_ROOT/sdks/python[validation]"
else
    run "$VENV_PIP" install -e "$REPO_ROOT/sdks/python"
fi

for framework in "${FRAMEWORKS[@]}"; do
    case "$framework" in
        langchain)
            run "$VENV_PIP" install -e "$REPO_ROOT/adapters/langchain"
            ;;
        crewai)
            run "$VENV_PIP" install -e "$REPO_ROOT/adapters/crewai"
            ;;
        autogen)
            run "$VENV_PIP" install -e "$REPO_ROOT/adapters/autogen"
            ;;
        openai_agents)
            run "$VENV_PIP" install -e "$REPO_ROOT/adapters/openai_agents"
            ;;
    esac

done

export DG_ENDPOINT="$ENDPOINT"
if [[ "$VALIDATE" == "true" ]]; then
    export DG_VALIDATE="1"
else
    export DG_VALIDATE="0"
fi
export DEPLOY_ENV="production"

for framework in "${FRAMEWORKS[@]}"; do
    case "$framework" in
        langchain)
            run "$VENV_PY" "$REPO_ROOT/examples/frameworks/langchain_tool.py"
            ;;
        crewai)
            run "$VENV_PY" "$REPO_ROOT/examples/frameworks/crewai_tool.py"
            ;;
        autogen)
            run "$VENV_PY" "$REPO_ROOT/examples/frameworks/autogen_tool.py"
            ;;
        openai_agents)
            run "$VENV_PY" "$REPO_ROOT/examples/frameworks/openai_agents_tool.py"
            ;;
    esac

done

echo "Adapter tests complete."
