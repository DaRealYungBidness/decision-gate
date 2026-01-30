<!--
Docs/guides/llm_native_playbook.md
============================================================================
Document: LLM-Native Playbook
Description: LLM-first workflows for Decision Gate using precheck and JSON evidence.
Purpose: Provide concrete, copy-pasteable MCP flows for agents and toolchains.
Dependencies:
  - Docs/guides/evidence_flow_and_execution_model.md
  - Docs/generated/decision-gate/tooling.md
============================================================================
-->

# LLM-Native Playbook

## At a Glance

**What:** LLM-optimized workflows for fast iteration (precheck) and auditable runs (live)
**Why:** Agents can iterate toward gate satisfaction deterministically
**Who:** LLM agent developers, automation engineers
**Prerequisites:** [evidence_flow_and_execution_model.md](evidence_flow_and_execution_model.md)

---

## Mental Model: Two Paths

```
Path A: Precheck (fast, asserted)
  - client supplies payload
  - data shape validates payload
  - gates evaluated, no run state mutation

Path B: Live Run (audited, verified)
  - providers fetch evidence
  - run state mutated, runpack stored
```

---

## Quick Start: Precheck

### Step 1: Define a Scenario

```bash
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "scenario_define",
      "arguments": {
        "spec": {
          "scenario_id": "llm-precheck",
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
  }'
```

### Step 2: Register a Data Shape (Schema)

```bash
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "schemas_register",
      "arguments": {
        "record": {
          "tenant_id": 1,
          "namespace_id": 1,
          "schema_id": "llm-precheck",
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
  }'
```

### Step 3: Precheck with Inline Payload

```bash
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "precheck",
      "arguments": {
        "tenant_id": 1,
        "namespace_id": 1,
        "scenario_id": "llm-precheck",
        "spec": null,
        "stage_id": "main",
        "data_shape": { "schema_id": "llm-precheck", "version": "v1" },
        "payload": { "report_ok": 0 }
      }
    }
  }'
```

**Precheck response (exact shape):**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "decision": {
      "kind": "complete",
      "stage_id": "main"
    },
    "gate_evaluations": [
      {
        "gate_id": "quality",
        "status": "true",
        "trace": [
          { "condition_id": "report_ok", "status": "true" }
        ]
      }
    ]
  }
}
```

**Important:** `precheck` does **not** return evidence values or provider errors.

---

## Live Run Flow (Audited)

```bash
# scenario_start
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/call",
    "params": {
      "name": "scenario_start",
      "arguments": {
        "scenario_id": "llm-precheck",
        "run_config": {
          "tenant_id": 1,
          "namespace_id": 1,
          "run_id": "run-1",
          "scenario_id": "llm-precheck",
          "dispatch_targets": [],
          "policy_tags": []
        },
        "started_at": { "kind": "unix_millis", "value": 1710000000000 },
        "issue_entry_packets": false
      }
    }
  }'

# scenario_next
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 5,
    "method": "tools/call",
    "params": {
      "name": "scenario_next",
      "arguments": {
        "scenario_id": "llm-precheck",
        "request": {
          "run_id": "run-1",
          "tenant_id": 1,
          "namespace_id": 1,
          "trigger_id": "trigger-1",
          "agent_id": "agent-1",
          "time": { "kind": "unix_millis", "value": 1710000000000 },
          "correlation_id": null
        }
      }
    }
  }'
```

**Live result:** `NextResult { decision, packets, status }`.

To inspect evidence and errors, call `runpack_export` or `evidence_query` (if disclosure policy allows).

---

## Error Recovery

Precheck failures are either:
- **Tool errors** (schema not found, payload invalid), or
- **Gate holds** (decision `hold`), with trace showing which conditions are unknown/false.

Since precheck does not return evidence errors:
1. Use `evidence_query` for provider debugging (subject to disclosure policy).
2. Run a live evaluation and export the runpack to inspect evidence errors.

---

## Schema Design Tips

- Use an **object** payload keyed by condition IDs.
- Set `additionalProperties: false` to catch typos.
- If your scenario has exactly one condition, you may pass a non-object payload.

---

## Glossary

**Precheck:** Asserted evidence evaluation; no state mutation.
**Live Run:** Provider-fetched evaluation; runpack stored.
**Data Shape:** JSON Schema used to validate precheck payloads.
