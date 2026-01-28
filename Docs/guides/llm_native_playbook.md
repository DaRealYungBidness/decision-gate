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

## Overview
Decision Gate is LLM-native when you treat evidence as **data**. Agents run
external tools, emit JSON artifacts or inline evidence, and DG evaluates gates
with deterministic comparators. No agent framework is required.

Two primary flows:
1) **Precheck (inline evidence)** – fastest iteration, no run state mutation.
2) **Live run + JSON provider (file evidence)** – audit-grade, runpackable.

---

## Flow A: Precheck (Inline Evidence)
Use precheck when you want LLMs to iterate quickly and avoid filesystem
coordination. Evidence is asserted, schema-validated, and evaluated without
mutating run state.

### 1) Define a scenario
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
          "spec_version": "v1",
          "stages": [
            {
              "stage_id": "main",
              "entry_packets": [],
              "gates": [
                { "gate_id": "quality", "requirement": { "Predicate": "report_ok" } }
              ],
              "advance_to": { "kind": "terminal" },
              "timeout": null,
              "on_timeout": "fail"
            }
          ],
          "predicates": [
            {
              "predicate": "report_ok",
              "query": {
                "provider_id": "json",
                "predicate": "path",
                "params": { "file": "report.json", "jsonpath": "$.summary.failed" }
              },
              "comparator": "equals",
              "expected": 0,
              "policy_tags": []
            }
          ],
          "policies": [],
          "schemas": [],
          "default_tenant_id": null
        }
      }
    }
  }'
```

### 2) Register a payload schema
Precheck requires a data shape so the asserted payload can be validated.

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

### 3) Precheck with inline evidence
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

**Result**: Precheck returns gate outcomes without mutating state. Use this for
LLM iteration loops and unit-style validation of evidence semantics.

**Note**: Precheck evidence is asserted. If your server enforces
`trust.min_lane = verified`, either relax it for precheck or add a predicate
trust override that allows asserted evidence.

---

## Flow B: Live Run + JSON Provider (File Evidence)
Use this for audit-grade runs where evidence must be fetched from deterministic
sources and captured in runpacks.

### 1) Run your tool and write JSON
Example artifact:
```json
{ "summary": { "failed": 0, "passed": 128 }, "tool": "tests" }
```

### 2) Call `evidence_query`
```bash
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "evidence_query",
      "arguments": {
        "query": {
          "provider_id": "json",
          "predicate": "path",
          "params": { "file": "/abs/path/report.json", "jsonpath": "$.summary.failed" }
        },
        "context": {
          "tenant_id": 1,
          "namespace_id": 1,
          "run_id": "run-1",
          "scenario_id": "llm-precheck",
          "stage_id": "main",
          "trigger_id": "trigger-1",
          "trigger_time": { "kind": "logical", "value": 1 },
          "correlation_id": null
        }
      }
    }
  }'
```

### 3) Trigger evaluation
Use `scenario_start` + `scenario_trigger` / `scenario_next` as in the standard
workflow; DG will fetch evidence again during gate evaluation.

---

## Error Metadata and Recovery
Missing or invalid evidence yields **structured error metadata**, enabling
agents to self-repair.

Example response when JSONPath is missing:
```json
{
  "value": null,
  "lane": "verified",
  "error": {
    "code": "jsonpath_not_found",
    "message": "jsonpath not found: $.summary.failed",
    "details": { "jsonpath": "$.summary.failed" }
  },
  "evidence_hash": null,
  "evidence_ref": null,
  "evidence_anchor": null,
  "signature": null,
  "content_type": null
}
```

**Agent loop strategy**:
1) Detect `error.code`.
2) Fix pipeline (adjust JSONPath or tool output).
3) Re-run tool → call `precheck` again.

---

## When to Use Each Flow
- **Precheck**: fast iteration, LLM-native, no audit state.
- **Live run**: audit-grade evidence, runpacks, external verification.

Both flows share the same comparator semantics and determinism guarantees.
