<!--
Docs/guides/getting_started.md
============================================================================
Document: Decision Gate Getting Started
Description: Quick-start guide for running Decision Gate MCP locally.
Purpose: Provide a short walkthrough using built-in evidence providers.
Dependencies:
  - decision-gate-mcp
  - decision-gate-providers
============================================================================
-->

# Getting Started with Decision Gate

## At a Glance

**What:** Run a minimal Decision Gate scenario locally
**Why:** See the full lifecycle: define -> start -> evaluate
**Who:** Developers integrating Decision Gate into CI/CD, agents, or compliance workflows
**Prerequisites:** Familiarity with JSON-RPC 2.0 and curl

## Mental Model: Scenario Lifecycle

Decision Gate evaluates **gates** using **conditions** (evidence checks). The lifecycle is:

```dg-skip dg-reason="output-only" dg-expires=2026-06-30
1. scenario_define  -> registers a ScenarioSpec
2. scenario_start   -> creates a RunState (new run)
3. scenario_next    -> evaluates current stage and returns a DecisionRecord
```

Key API outputs:
- `scenario_define` returns `{ scenario_id, spec_hash }`.
- `scenario_start` returns the full `RunState`.
- `scenario_next` returns `{ decision, packets, status }` (a `NextResult`).

## What is a Scenario?

A **scenario** is a workflow definition composed of:

- **Stages**: Ordered steps in the workflow. Think of stages as the top-level
  sequence you move through as a run progresses.
- **Gates**: Decision points inside a stage. A stage can include one or more
  gates that must pass to move forward.
- **Conditions**: Evidence checks used by gates. Each gate evaluates one or more
  conditions to decide whether it passes.
- **Providers**: Evidence sources (builtin or MCP). Conditions don't fetch data
  themselves; they ask providers for evidence to evaluate.

Providers can be:
- **Built-in**: `time`, `env`, `json`, `http`
- **External MCP**: any tool implementing the `evidence_query` protocol

---

## Quick Start

### Step 0: Get the CLI

If you installed the CLI, use `decision-gate`. If you are running from source,
use `cargo run`:

```bash dg-run dg-level=manual
# Installed binary
decision-gate --help

# From source (repo checkout)
cargo run -p decision-gate-cli -- --help
```

**One-command smoke test:** `scripts/bootstrap/quickstart.sh` (bash/WSL) or
`scripts/bootstrap/quickstart.ps1` (PowerShell) runs the full define → start → next →
runpack → precheck flow with unique IDs.

### Step 1: Choose a Preset

Pick a preset configuration from `configs/presets/` (details in
[preset_configs.md](preset_configs.md)). For this guide, use **Quickstart-Dev**
to keep the first run frictionless.

Other presets:
- **Default-Recommended:** local-only + explicit principal mapping.
- **Hardened:** bearer auth + schema signing required.
- **Container-Prod:** bearer auth + upstream TLS termination (container baseline).

If you want to edit settings, copy the preset first:

```bash dg-run dg-level=manual
cp configs/presets/quickstart-dev.toml decision-gate.toml
```

Notes:
- Default namespace id `1` is **blocked** unless `namespace.allow_default = true` **and** the tenant id is listed in `namespace.default_tenants`.
- For non-loopback HTTP/SSE binds, Decision Gate requires `--allow-non-loopback` plus TLS or `server.tls_termination = "upstream"` and non-local auth. See [security_guide.md](security_guide.md).
- **Windows tip:** PowerShell/CMD do not support bash-style multiline `curl`. Use a single-line command or PowerShell's `@'... '@` here-string.

### Step 2: Start the MCP Server

```bash dg-run dg-level=manual dg-requires=mcp
# Installed binary:
decision-gate serve --config configs/presets/quickstart-dev.toml

# From source (repo checkout):
# cargo run -p decision-gate-cli -- serve --config configs/presets/quickstart-dev.toml
```

### Step 3: Define a Scenario

This scenario gates on a time check: `time.after(timestamp)`.

**If you use the Hardened preset:** add `-H 'Authorization: Bearer <token>'` to every `curl`, and switch to a non-default namespace (e.g., `namespace_id: 2`) because `allow_default = false`.

```bash dg-run dg-level=manual dg-requires=mcp
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
          "scenario_id": "quickstart",
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
                "params": { "timestamp": 1700000000000 }
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
  }'
```

**Time semantics:** `time.after` returns `true` **only if** `trigger_time > timestamp` (strictly greater). Equality returns `false`.

**Response (MCP-wrapped):**

```json dg-parse dg-level=fast
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "json",
        "json": {
          "scenario_id": "quickstart",
          "spec_hash": { "algorithm": "sha256", "value": "<hex>" }
        }
      }
    ]
  }
}
```

### Step 4: Start a Run

```bash dg-run dg-level=manual dg-requires=mcp
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "scenario_start",
      "arguments": {
        "scenario_id": "quickstart",
        "run_config": {
          "tenant_id": 1,
          "namespace_id": 1,
          "run_id": "run-1",
          "scenario_id": "quickstart",
          "dispatch_targets": [],
          "policy_tags": []
        },
        "started_at": { "kind": "unix_millis", "value": 1710000000000 },
        "issue_entry_packets": false
      }
    }
  }'
```

**Note:** `run_id` values must be unique. If you re-run this guide, change
`run_id` (e.g., `run-2`) or delete the local run state store
(`decision-gate.db` by default).

**Response (MCP-wrapped):** `scenario_start` returns the full `RunState` inside `result.content[0].json`.

```json dg-parse dg-level=fast
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "json",
        "json": {
          "tenant_id": 1,
          "namespace_id": 1,
          "run_id": "run-1",
          "scenario_id": "quickstart",
          "spec_hash": { "algorithm": "sha256", "value": "<hex>" },
          "current_stage_id": "main",
          "stage_entered_at": { "kind": "unix_millis", "value": 1710000000000 },
          "status": "active",
          "dispatch_targets": [],
          "triggers": [],
          "gate_evals": [],
          "decisions": [],
          "packets": [],
          "submissions": [],
          "tool_calls": []
        }
      }
    ]
  }
}
```

### Step 5: Trigger Gate Evaluation

```bash dg-run dg-level=manual dg-requires=mcp
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "scenario_next",
      "arguments": {
        "scenario_id": "quickstart",
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

With `time.after` and `timestamp = 1700000000000`, the check returns `true` because `1710000000000 > 1700000000000`.

Optional: add `"feedback": "trace"` inside `arguments` to get gate/condition status when permitted by server feedback policy.

**Response (MCP-wrapped):**

```json dg-parse dg-level=fast
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "json",
        "json": {
          "decision": {
            "decision_id": "decision-1",
            "seq": 1,
            "trigger_id": "trigger-1",
            "stage_id": "main",
            "decided_at": { "kind": "unix_millis", "value": 1710000000000 },
            "outcome": { "kind": "complete", "stage_id": "main" },
            "correlation_id": null
          },
          "packets": [],
          "status": "completed"
        }
      }
    ]
  }
}
```

---

## Troubleshooting

### Gate Outcome is `hold`

If a gate cannot be proven `true` or `false`, the decision outcome will be `hold` and the response will include a `SafeSummary`. For non-local requests, `scenario_next` defaults to summary-only feedback; in local-only mode the default is `trace`. You can always request `feedback: "trace"` (if allowed) or use `precheck` for fast iteration.

```json dg-parse dg-level=fast
{
  "result": {
    "content": [
      {
        "type": "json",
        "json": {
          "decision": {
            "outcome": {
              "kind": "hold",
              "summary": {
                "status": "hold",
                "unmet_gates": ["after-time"],
                "retry_hint": "await_evidence",
                "policy_tags": []
              }
            }
          }
        }
      }
    ]
  }
}
```

**How to debug precisely:**
1. **Export the runpack** with `runpack_export` and inspect `gate_evals` and evidence errors in the manifest.
2. **Use `evidence_query`** (if disclosure policy allows) to reproduce the provider call and see its `EvidenceResult`.

### Auth Required

If you configure `[server.auth]`, include the appropriate auth header:

```bash dg-run dg-level=manual dg-requires=mcp
curl -s http://127.0.0.1:4000/rpc \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer YOUR_TOKEN' \
  -d '{ ... }'
```

---

## Next Steps

- [condition_authoring.md](condition_authoring.md): write precise conditions and comparators
- [json_evidence_playbook.md](json_evidence_playbook.md): JSON evidence recipes
- [llm_native_playbook.md](llm_native_playbook.md): precheck workflows for LLM agents
- [security_guide.md](security_guide.md): production hardening (auth, TLS, signatures, anchors)
