#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

CONFIG=${1:-configs/presets/quickstart-dev.toml}
BASE_URL=${DG_URL:-http://127.0.0.1:4000/rpc}
RUNPACK_ROOT=${RUNPACK_ROOT:-/tmp/dg-runpacks}
SUFFIX=${DG_SUFFIX:-$(date +%s)}
NOW_MS=$(( $(date +%s) * 1000 ))
TIMESTAMP_MS=$(( NOW_MS - 60000 ))

if command -v decision-gate >/dev/null 2>&1; then
  DG_CMD=(decision-gate)
else
  DG_CMD=(cargo run -p decision-gate-cli --)
fi

log() {
  printf "%s\n" "$*"
}

log "Starting Decision Gate MCP server..."
"${DG_CMD[@]}" serve --config "$CONFIG" > /tmp/dg-quickstart.log 2>&1 &
DG_PID=$!

cleanup() {
  if kill -0 "$DG_PID" 2>/dev/null; then
    kill "$DG_PID" 2>/dev/null || true
    wait "$DG_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

log "Waiting for server to be ready..."
ready=false
for _ in $(seq 1 240); do
  if curl -s --max-time 1 "$BASE_URL" \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":0,"method":"tools/list"}' | grep -q '"result"'; then
    ready=true
    break
  fi
  sleep 0.5
done

if [ "$ready" != "true" ]; then
  log "Server did not become ready. See /tmp/dg-quickstart.log"
  exit 1
fi

SCENARIO_ID="quickstart-${SUFFIX}"
RUN_ID="run-${SUFFIX}"
PRECHECK_SCENARIO="llm-precheck-${SUFFIX}"
SCHEMA_ID="llm-precheck-${SUFFIX}"
RUNPACK_DIR="${RUNPACK_ROOT}/${RUN_ID}"
mkdir -p "$RUNPACK_DIR"

log "Defining quickstart scenario (${SCENARIO_ID})..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "scenario_define",
    "arguments": {
      "spec": {
        "scenario_id": "${SCENARIO_ID}",
        "namespace_id": 1,
        "spec_version": "v1",
        "stages": [
          {
            "stage_id": "main",
            "entry_packets": [],
            "gates": [
              {
                "gate_id": "after-time",
                "requirement": { "Condition": "after" }
              }
            ],
            "advance_to": { "kind": "terminal" },
            "timeout": null,
            "on_timeout": "fail"
          }
        ],
        "conditions": [
          {
            "condition_id": "after",
            "query": {
              "provider_id": "time",
              "check_id": "after",
              "params": { "timestamp": ${TIMESTAMP_MS} }
            },
            "comparator": "equals",
            "expected": true,
            "policy_tags": []
          }
        ],
        "policies": [],
        "schemas": [],
        "default_tenant_id": 1
      }
    }
  }
}
JSON

echo
log "Starting run (${RUN_ID})..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "scenario_start",
    "arguments": {
      "scenario_id": "${SCENARIO_ID}",
      "run_config": {
        "tenant_id": 1,
        "namespace_id": 1,
        "run_id": "${RUN_ID}",
        "scenario_id": "${SCENARIO_ID}",
        "dispatch_targets": [],
        "policy_tags": []
      },
      "started_at": { "kind": "unix_millis", "value": ${NOW_MS} },
      "issue_entry_packets": false
    }
  }
}
JSON

echo
log "Evaluating gate..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "scenario_next",
    "arguments": {
      "scenario_id": "${SCENARIO_ID}",
      "request": {
        "run_id": "${RUN_ID}",
        "tenant_id": 1,
        "namespace_id": 1,
        "trigger_id": "trigger-1",
        "agent_id": "agent-1",
        "time": { "kind": "unix_millis", "value": ${NOW_MS} },
        "correlation_id": null
      }
    }
  }
}
JSON

echo
log "Exporting runpack (${RUNPACK_DIR})..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "runpack_export",
    "arguments": {
      "tenant_id": 1,
      "namespace_id": 1,
      "scenario_id": "${SCENARIO_ID}",
      "run_id": "${RUN_ID}",
      "generated_at": { "kind": "unix_millis", "value": ${NOW_MS} },
      "include_verification": true,
      "manifest_name": "manifest.json",
      "output_dir": "${RUNPACK_DIR}"
    }
  }
}
JSON

echo
log "Verifying runpack..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "runpack_verify",
    "arguments": {
      "runpack_dir": "${RUNPACK_DIR}",
      "manifest_path": "manifest.json"
    }
  }
}
JSON

echo
log "Defining precheck scenario (${PRECHECK_SCENARIO})..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "name": "scenario_define",
    "arguments": {
      "spec": {
        "scenario_id": "${PRECHECK_SCENARIO}",
        "namespace_id": 1,
        "spec_version": "v1",
        "stages": [
          {
            "stage_id": "main",
            "entry_packets": [],
            "gates": [
              {
                "gate_id": "quality",
                "requirement": { "Condition": "report_ok" }
              }
            ],
            "advance_to": { "kind": "terminal" },
            "timeout": null,
            "on_timeout": "fail"
          }
        ],
        "conditions": [
          {
            "condition_id": "report_ok",
            "query": {
              "provider_id": "json",
              "check_id": "path",
              "params": { "file": "report.json", "jsonpath": "$.summary.failed" }
            },
            "comparator": "equals",
            "expected": 0,
            "policy_tags": []
          }
        ],
        "policies": [],
        "schemas": [],
        "default_tenant_id": 1
      }
    }
  }
}
JSON

echo
log "Registering schema (${SCHEMA_ID})..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "tools/call",
  "params": {
    "name": "schemas_register",
    "arguments": {
      "record": {
        "tenant_id": 1,
        "namespace_id": 1,
        "schema_id": "${SCHEMA_ID}",
        "version": "v1",
        "schema": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "report_ok": { "type": "number" }
          },
          "required": ["report_ok"]
        },
        "description": "LLM precheck payload schema",
        "created_at": { "kind": "logical", "value": 1 },
        "signing": null
      }
    }
  }
}
JSON

echo
log "Running precheck..."
curl -s "$BASE_URL" \
  -H 'Content-Type: application/json' \
  -d @- <<JSON
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "tools/call",
  "params": {
    "name": "precheck",
    "arguments": {
      "tenant_id": 1,
      "namespace_id": 1,
      "scenario_id": "${PRECHECK_SCENARIO}",
      "spec": null,
      "stage_id": "main",
      "data_shape": { "schema_id": "${SCHEMA_ID}", "version": "v1" },
      "payload": { "report_ok": 0 }
    }
  }
}
JSON

echo
log "Quickstart complete. Runpack: ${RUNPACK_DIR}"
