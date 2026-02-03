#!/usr/bin/env bash
# scripts/ci_release_gate.sh
# =============================================================================
# Module: Decision Gate CI Release Eligibility
# Description: Runs a local MCP server and evaluates a release gate scenario
#              against a CI evidence bundle, exporting a runpack.
# Dependencies: bash, curl, python3, cargo (for decision-gate-cli)
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

EVIDENCE_FILE=""
OUTPUT_DIR=""
CONFIG_PATH="$REPO_ROOT/configs/presets/ci-release-gate.toml"
SCENARIO_TEMPLATE="$REPO_ROOT/configs/ci/release_gate_scenario.json"
BASE_URL="http://127.0.0.1:4010/rpc"

print_usage() {
    cat <<EOF
Usage: $(basename "$0") --evidence-file PATH --output-dir DIR [--config PATH] [--base-url URL]

Options:
  --evidence-file PATH  Path to JSON evidence bundle.
  --output-dir DIR      Output directory for runpack artifacts.
  --config PATH         MCP server config file (default: configs/presets/ci-release-gate.toml).
  --base-url URL        MCP server base URL (default: http://127.0.0.1:4010).
  -h, --help            Show this help message.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --evidence-file)
            EVIDENCE_FILE="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --config)
            CONFIG_PATH="$2"
            shift 2
            ;;
        --base-url)
            BASE_URL="$2"
            shift 2
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

if [[ -z "$EVIDENCE_FILE" || -z "$OUTPUT_DIR" ]]; then
    echo "ERROR: --evidence-file and --output-dir are required." >&2
    print_usage
    exit 1
fi

if [[ "$BASE_URL" != */rpc ]]; then
    BASE_URL="${BASE_URL%/}/rpc"
fi

if [[ ! -f "$EVIDENCE_FILE" ]]; then
    echo "ERROR: evidence file not found: $EVIDENCE_FILE" >&2
    exit 1
fi

if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "ERROR: config file not found: $CONFIG_PATH" >&2
    exit 1
fi

if [[ ! -f "$SCENARIO_TEMPLATE" ]]; then
    echo "ERROR: scenario template not found: $SCENARIO_TEMPLATE" >&2
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "ERROR: python3 is required for release gate evaluation." >&2
    exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
    echo "ERROR: curl is required for release gate evaluation." >&2
    exit 1
fi

ABS_EVIDENCE_FILE="$(python3 - <<PY
import pathlib
print(pathlib.Path("$EVIDENCE_FILE").resolve())
PY
)"

mkdir -p "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/runpack"
LOG_PATH="$OUTPUT_DIR/dg-release-gate.log"

DG_BIN="${DG_BIN:-}"
if [[ -z "$DG_BIN" ]]; then
    TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
    BIN_CANDIDATE="$TARGET_DIR/debug/decision-gate"
    if [[ -x "$BIN_CANDIDATE.exe" ]]; then
        BIN_CANDIDATE="$BIN_CANDIDATE.exe"
    fi
    if [[ -x "$BIN_CANDIDATE" ]]; then
        DG_BIN="$BIN_CANDIDATE"
    else
        (cd "$REPO_ROOT" && cargo build -p decision-gate-cli --locked)
        BIN_CANDIDATE="$TARGET_DIR/debug/decision-gate"
        if [[ -x "$BIN_CANDIDATE.exe" ]]; then
            BIN_CANDIDATE="$BIN_CANDIDATE.exe"
        fi
        DG_BIN="$BIN_CANDIDATE"
    fi
fi

SCENARIO_ID="ci-release-gate-$(date -u +%Y%m%d%H%M%S)-$$"
RUN_ID="release-run-$(date -u +%Y%m%d%H%M%S)-$$"
NOW_MS="$(python3 - <<PY
import time
print(int(time.time() * 1000))
PY
)"

SERVER_PID=""
cleanup() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" >/dev/null 2>&1 || true
        wait "$SERVER_PID" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

mkdir -p "$REPO_ROOT/.tmp/ci-release-gate"

"$DG_BIN" serve --config "$CONFIG_PATH" >"$LOG_PATH" 2>&1 &
SERVER_PID=$!

ready="false"
for _ in $(seq 1 60); do
    if curl -s "$BASE_URL" -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":0,"method":"tools/list"}' | grep -q '"result"'; then
        ready="true"
        break
    fi
    sleep 0.5
done

if [[ "$ready" != "true" ]]; then
    echo "ERROR: MCP server did not become ready. Log: $LOG_PATH" >&2
    exit 1
fi

SPEC_FILE="$(mktemp)"
python3 - <<PY
import json
from pathlib import Path
text = Path("$SCENARIO_TEMPLATE").read_text()
text = text.replace("{{EVIDENCE_FILE}}", "$ABS_EVIDENCE_FILE")
text = text.replace("{{SCENARIO_ID}}", "$SCENARIO_ID")
json.loads(text)
Path("$SPEC_FILE").write_text(text)
PY

DEFINE_REQ="$(mktemp)"
python3 - <<PY
import json
from pathlib import Path
spec = json.loads(Path("$SPEC_FILE").read_text())
req = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "scenario_define",
        "arguments": {"spec": spec},
    },
}
Path("$DEFINE_REQ").write_text(json.dumps(req))
PY

curl -s "$BASE_URL" -H 'Content-Type: application/json' -d @"$DEFINE_REQ" >/dev/null

START_REQ="$(mktemp)"
python3 - <<PY
import json
from pathlib import Path
req = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "scenario_start",
        "arguments": {
            "scenario_id": "$SCENARIO_ID",
            "run_config": {
                "tenant_id": 1,
                "namespace_id": 1,
                "run_id": "$RUN_ID",
                "scenario_id": "$SCENARIO_ID",
                "dispatch_targets": [],
                "policy_tags": [],
            },
            "started_at": {"kind": "unix_millis", "value": $NOW_MS},
            "issue_entry_packets": False,
        },
    },
}
Path("$START_REQ").write_text(json.dumps(req))
PY

curl -s "$BASE_URL" -H 'Content-Type: application/json' -d @"$START_REQ" >/dev/null

NEXT_REQ="$(mktemp)"
python3 - <<PY
import json
from pathlib import Path
req = {
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
        "name": "scenario_next",
        "arguments": {
            "scenario_id": "$SCENARIO_ID",
            "request": {
                "run_id": "$RUN_ID",
                "tenant_id": 1,
                "namespace_id": 1,
                "trigger_id": "release-gate",
                "agent_id": "ci",
                "time": {"kind": "unix_millis", "value": $NOW_MS},
                "correlation_id": None,
            },
        },
    },
}
Path("$NEXT_REQ").write_text(json.dumps(req))
PY

NEXT_RESP_PATH="$OUTPUT_DIR/decision.json"
curl -s "$BASE_URL" -H 'Content-Type: application/json' -d @"$NEXT_REQ" >"$NEXT_RESP_PATH"

python3 - <<PY
import json
from pathlib import Path
resp = json.loads(Path("$NEXT_RESP_PATH").read_text())
result = resp.get("result") or {}
content = result.get("content") or []
if not content:
    raise SystemExit("ERROR: scenario_next returned empty content")
payload = content[0].get("json") or {}
Path("$OUTPUT_DIR/decision_payload.json").write_text(json.dumps(payload, indent=2))
decision = payload.get("decision") or {}
kind = decision.get("kind")
if kind is None:
    outcome = decision.get("outcome") or {}
    kind = outcome.get("kind")
allowed = kind in {"advance", "complete"}
Path("$OUTPUT_DIR/decision_summary.json").write_text(
    json.dumps({"kind": kind, "allowed": allowed}, indent=2)
)
if kind is None:
    raise SystemExit("ERROR: decision kind missing from scenario_next response")
PY

RUNPACK_REQ="$(mktemp)"
python3 - <<PY
import json
from pathlib import Path
req = {
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/call",
    "params": {
        "name": "runpack_export",
        "arguments": {
            "tenant_id": 1,
            "namespace_id": 1,
            "scenario_id": "$SCENARIO_ID",
            "run_id": "$RUN_ID",
            "generated_at": {"kind": "unix_millis", "value": $NOW_MS},
            "include_verification": True,
            "manifest_name": "manifest.json",
            "output_dir": "$OUTPUT_DIR/runpack",
        },
    },
}
Path("$RUNPACK_REQ").write_text(json.dumps(req))
PY

curl -s "$BASE_URL" -H 'Content-Type: application/json' -d @"$RUNPACK_REQ" >/dev/null

VERIFY_REQ="$(mktemp)"
python3 - <<PY
import json
from pathlib import Path
req = {
    "jsonrpc": "2.0",
    "id": 5,
    "method": "tools/call",
    "params": {
        "name": "runpack_verify",
        "arguments": {
            "runpack_dir": "$OUTPUT_DIR/runpack",
            "manifest_path": "manifest.json",
        },
    },
}
Path("$VERIFY_REQ").write_text(json.dumps(req))
PY

curl -s "$BASE_URL" -H 'Content-Type: application/json' -d @"$VERIFY_REQ" >"$OUTPUT_DIR/runpack_verify.json"

python3 - <<PY
import json
from pathlib import Path
resp = json.loads(Path("$OUTPUT_DIR/runpack_verify.json").read_text())
if resp.get("error"):
    raise SystemExit(f"ERROR: runpack_verify failed: {resp['error'].get('message')}")
PY

python3 - <<PY
import json
from pathlib import Path
summary = json.loads(Path("$OUTPUT_DIR/decision_summary.json").read_text())
if not summary.get("allowed"):
    raise SystemExit(f"ERROR: release gate denied (decision kind: {summary.get('kind')})")
PY

echo "Release gate runpack written to: $OUTPUT_DIR/runpack"
