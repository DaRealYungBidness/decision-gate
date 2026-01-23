<!--
Docs/guides/getting_started.md
============================================================================
Document: Decision Gate Getting Started
Description: Quick-start guide for running Decision Gate MCP locally.
Purpose: Provide a 5-minute walkthrough using built-in evidence providers.
Dependencies:
  - decision-gate-mcp
  - decision-gate-providers
============================================================================
-->

# Getting Started (5 Minutes)

## Overview
This guide starts a local Decision Gate MCP server with built-in providers and
walks through a minimal scenario lifecycle.

## 1) Create a Local Config
Create `decision-gate.toml`:

```toml
[server]
transport = "http"
bind = "127.0.0.1:4000"

[trust]
default_policy = "audit"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[run_state_store]
type = "sqlite"
path = "decision-gate.db"
journal_mode = "wal"
sync_mode = "full"
busy_timeout_ms = 5000

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"
```

Use `type = "memory"` for ephemeral local runs, but `sqlite` is the default
for durable, audit-grade runs.

## 2) Start the MCP Server
```bash
decision-gate serve --config decision-gate.toml
```

The server emits a local-only warning because auth/policy is not wired yet.

## 3) Define a Scenario
Send a `scenario_define` request:

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
          "scenario_id": "quickstart",
          "spec_version": "v1",
          "stages": [
            {
              "stage_id": "main",
              "entry_packets": [],
              "gates": [
                {
                  "gate_id": "after-time",
                  "requirement": { "Predicate": "after" }
                }
              ],
              "advance_to": { "kind": "terminal" },
              "timeout": null,
              "on_timeout": "fail"
            }
          ],
          "predicates": [
            {
              "predicate": "after",
              "query": {
                "provider_id": "time",
                "predicate": "after",
                "params": { "timestamp": 1710000000000 }
              },
              "comparator": "equals",
              "expected": true,
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

## 4) Start a Run and Evaluate
```bash
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
          "tenant_id": "tenant-1",
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

```bash
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
          "trigger_id": "trigger-1",
          "agent_id": "agent-1",
          "time": { "kind": "unix_millis", "value": 1710000000000 },
          "correlation_id": null
        }
      }
    }
  }'
```

## Next Steps
- Explore `Docs/guides/integration_patterns.md` for CI and agent-loop patterns.
- Use `decision-gate-cli` to export and verify runpacks.
