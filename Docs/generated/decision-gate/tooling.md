# Decision Gate MCP Tools

This document summarizes the MCP tool surface and expected usage. Full schemas are in `tooling.json`, with supporting schemas under `schemas/` and examples under `examples/`.

## Lifecycle quickstart

- `scenario_define` registers and validates a ScenarioSpec.
- `scenario_start` creates a run and optionally issues entry packets.
- `scenario_next` advances an agent-driven run; `scenario_trigger` advances time/external triggers.
- `scenario_status` polls run state without mutating it.
- `scenario_submit` appends external artifacts for audit and later checks.
- `runpack_export` and `runpack_verify` support offline verification.

## Artifact references

- `examples/scenario.json`: full ScenarioSpec example.
- `examples/run-config.json`: run config example for scenario_start.
- `examples/decision-gate.toml`: MCP config example for providers.

| Tool | Description |
| --- | --- |
| scenario_define | Register a ScenarioSpec, validate it, and return the canonical hash used for integrity checks. |
| scenario_start | Create a new run state for a scenario and optionally emit entry packets. |
| scenario_status | Fetch a read-only run snapshot and safe summary without changing state. |
| scenario_next | Evaluate gates in response to an agent-driven next request. |
| scenario_submit | Submit external artifacts into run state for audit and later evaluation. |
| scenario_trigger | Submit a trigger event (scheduler/external) and evaluate the run. |
| evidence_query | Query an evidence provider with full run context and disclosure policy. |
| runpack_export | Export deterministic runpack artifacts for offline verification. |
| runpack_verify | Verify a runpack manifest and artifacts offline. |

## scenario_define

Register a ScenarioSpec, validate it, and return the canonical hash used for integrity checks.

### Inputs

- `spec` (required): Scenario specification to register.

### Outputs

- `scenario_id` (required): Scenario identifier.
- `spec_hash` (required): Type: object.

### Notes

- Use before starting runs; scenario_id becomes the stable handle for later calls.
- Validates stage/gate/predicate IDs, RET trees, and predicate references.
- Spec hash is deterministic; store it for audit and runpack integrity.
- Fails closed on invalid specs or duplicate scenario IDs.

### Example

Register the example scenario spec.

Input:
```json
{
  "spec": {
    "default_tenant_id": null,
    "policies": [],
    "predicates": [
      {
        "comparator": "equals",
        "expected": "production",
        "policy_tags": [],
        "predicate": "env_is_prod",
        "query": {
          "params": {
            "key": "DEPLOY_ENV"
          },
          "predicate": "get",
          "provider_id": "env"
        }
      },
      {
        "comparator": "equals",
        "expected": true,
        "policy_tags": [],
        "predicate": "after_freeze",
        "query": {
          "params": {
            "timestamp": 1710000000000
          },
          "predicate": "after",
          "provider_id": "time"
        }
      }
    ],
    "scenario_id": "example-scenario",
    "schemas": [],
    "spec_version": "v1",
    "stages": [
      {
        "advance_to": {
          "kind": "terminal"
        },
        "entry_packets": [
          {
            "content_type": "application/json",
            "expiry": null,
            "packet_id": "packet-hello",
            "payload": {
              "kind": "json",
              "value": {
                "message": "hello",
                "purpose": "scenario entry packet"
              }
            },
            "policy_tags": [],
            "schema_id": "schema-hello",
            "visibility_labels": [
              "public"
            ]
          }
        ],
        "gates": [
          {
            "gate_id": "env_gate",
            "requirement": {
              "Predicate": "env_is_prod"
            }
          },
          {
            "gate_id": "time_gate",
            "requirement": {
              "Predicate": "after_freeze"
            }
          }
        ],
        "on_timeout": "fail",
        "stage_id": "main",
        "timeout": null
      }
    ]
  }
}
```
Output:
```json
{
  "scenario_id": "example-scenario",
  "spec_hash": {
    "algorithm": "sha256",
    "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
  }
}
```
## scenario_start

Create a new run state for a scenario and optionally emit entry packets.

### Inputs

- `issue_entry_packets` (required): Issue entry packets immediately.
- `run_config` (required): Run configuration and dispatch targets.
- `scenario_id` (required): Scenario identifier.
- `started_at` (required): Caller-supplied run start timestamp.

### Outputs

- `current_stage_id` (required): Current stage identifier.
- `decisions` (required): Type: array.
- `dispatch_targets` (required): Type: array.
- `gate_evals` (required): Type: array.
- `packets` (required): Type: array.
- `run_id` (required): Run identifier.
- `scenario_id` (required): Scenario identifier.
- `spec_hash` (required): Type: object.
- `status` (required): Type: string.
- `submissions` (required): Type: array.
- `tenant_id` (required): Tenant identifier.
- `tool_calls` (required): Type: array.
- `triggers` (required): Type: array.

### Notes

- Requires RunConfig (tenant_id, run_id, scenario_id, dispatch_targets).
- Use started_at to record the caller-supplied start timestamp.
- If issue_entry_packets is true, entry packets are disclosed immediately.
- Fails closed if run_id already exists or scenario_id is unknown.

### Example

Start a run for the example scenario and issue entry packets.

Input:
```json
{
  "issue_entry_packets": true,
  "run_config": {
    "dispatch_targets": [
      {
        "agent_id": "agent-alpha",
        "kind": "agent"
      }
    ],
    "policy_tags": [],
    "run_id": "run-0001",
    "scenario_id": "example-scenario",
    "tenant_id": "tenant-001"
  },
  "scenario_id": "example-scenario",
  "started_at": {
    "kind": "unix_millis",
    "value": 1710000000000
  }
}
```
Output:
```json
{
  "current_stage_id": "main",
  "decisions": [],
  "dispatch_targets": [
    {
      "agent_id": "agent-alpha",
      "kind": "agent"
    }
  ],
  "gate_evals": [],
  "packets": [],
  "run_id": "run-0001",
  "scenario_id": "example-scenario",
  "spec_hash": {
    "algorithm": "sha256",
    "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
  },
  "status": "active",
  "submissions": [],
  "tenant_id": "tenant-001",
  "tool_calls": [],
  "triggers": []
}
```
## scenario_status

Fetch a read-only run snapshot and safe summary without changing state.

### Inputs

- `request` (required): Status request payload.
- `scenario_id` (required): Scenario identifier.

### Outputs

- `current_stage_id` (required): Current stage identifier.
- `issued_packet_ids` (required): Type: array.
- `last_decision` (required, nullable): One of: null, object.
- `run_id` (required): Run identifier.
- `safe_summary` (required, nullable): One of: null, object.
- `scenario_id` (required): Scenario identifier.
- `status` (required): Type: string.

### Notes

- Use for polling or UI state; does not evaluate gates.
- Safe summaries omit evidence values and may include retry hints.
- Returns issued packet IDs to help track disclosures.

### Example

Poll run status without advancing the run.

Input:
```json
{
  "request": {
    "correlation_id": null,
    "requested_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "run_id": "run-0001"
  },
  "scenario_id": "example-scenario"
}
```
Output:
```json
{
  "current_stage_id": "main",
  "issued_packet_ids": [],
  "last_decision": null,
  "run_id": "run-0001",
  "safe_summary": null,
  "scenario_id": "example-scenario",
  "status": "active"
}
```
## scenario_next

Evaluate gates in response to an agent-driven next request.

### Inputs

- `request` (required): Next request payload from an agent.
- `scenario_id` (required): Scenario identifier.

### Outputs

- `decision` (required): Type: object.
- `packets` (required): Type: array.
- `status` (required): Type: string.

### Notes

- Idempotent by trigger_id; repeated calls return the same decision.
- Records decision, evidence, and packet disclosures in run state.
- Requires an active run; completed or failed runs do not advance.

### Example

Evaluate the next agent-driven step for a run.

Input:
```json
{
  "request": {
    "agent_id": "agent-alpha",
    "correlation_id": null,
    "run_id": "run-0001",
    "time": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "trigger_id": "trigger-0001"
  },
  "scenario_id": "example-scenario"
}
```
Output:
```json
{
  "decision": {
    "correlation_id": null,
    "decided_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "decision_id": "decision-0001",
    "outcome": {
      "kind": "complete",
      "stage_id": "main"
    },
    "seq": 0,
    "stage_id": "main",
    "trigger_id": "trigger-0001"
  },
  "packets": [],
  "status": "completed"
}
```
## scenario_submit

Submit external artifacts into run state for audit and later evaluation.

### Inputs

- `request` (required): Submission payload and metadata.
- `scenario_id` (required): Scenario identifier.

### Outputs

- `record` (required): Type: object.

### Notes

- Payload is hashed and stored as a submission record.
- Does not advance the run by itself.
- Use for artifacts the model or operator supplies.

### Example

Submit an external artifact for audit and later evaluation.

Input:
```json
{
  "request": {
    "content_type": "application/json",
    "correlation_id": null,
    "payload": {
      "kind": "json",
      "value": {
        "artifact": "attestation",
        "status": "approved"
      }
    },
    "run_id": "run-0001",
    "submission_id": "submission-0001",
    "submitted_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    }
  },
  "scenario_id": "example-scenario"
}
```
Output:
```json
{
  "record": {
    "content_hash": {
      "algorithm": "sha256",
      "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
    },
    "content_type": "application/json",
    "correlation_id": null,
    "payload": {
      "kind": "json",
      "value": {
        "artifact": "attestation",
        "status": "approved"
      }
    },
    "run_id": "run-0001",
    "submission_id": "submission-0001",
    "submitted_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    }
  }
}
```
## scenario_trigger

Submit a trigger event (scheduler/external) and evaluate the run.

### Inputs

- `scenario_id` (required): Scenario identifier.
- `trigger` (required): Trigger event payload.

### Outputs

- `decision` (required): Type: object.
- `packets` (required): Type: array.
- `status` (required): Type: string.

### Notes

- Trigger time is supplied by the caller; no wall-clock reads.
- Records the trigger event and resulting decision.
- Use for time-based or external system triggers.

### Example

Advance a run from a scheduler or external trigger.

Input:
```json
{
  "scenario_id": "example-scenario",
  "trigger": {
    "correlation_id": null,
    "kind": "tick",
    "payload_ref": null,
    "run_id": "run-0001",
    "source_id": "scheduler-01",
    "time": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "trigger_id": "trigger-0001"
  }
}
```
Output:
```json
{
  "decision": {
    "correlation_id": null,
    "decided_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "decision_id": "decision-0001",
    "outcome": {
      "kind": "complete",
      "stage_id": "main"
    },
    "seq": 0,
    "stage_id": "main",
    "trigger_id": "trigger-0001"
  },
  "packets": [],
  "status": "completed"
}
```
## evidence_query

Query an evidence provider with full run context and disclosure policy.

### Inputs

- `context` (required): Evidence context used for evaluation.
- `query` (required): Evidence query payload.

### Outputs

- `result` (required): Type: object.

### Notes

- Disclosure policy may redact raw values; hashes/anchors still returned.
- Use for diagnostics or preflight checks; runtime uses the same provider logic.
- Requires provider_id, predicate, and full EvidenceContext.

### Example

Query an evidence provider using the run context.

Input:
```json
{
  "context": {
    "correlation_id": null,
    "run_id": "run-0001",
    "scenario_id": "example-scenario",
    "stage_id": "main",
    "tenant_id": "tenant-001",
    "trigger_id": "trigger-0001",
    "trigger_time": {
      "kind": "unix_millis",
      "value": 1710000000000
    }
  },
  "query": {
    "params": {
      "key": "DEPLOY_ENV"
    },
    "predicate": "get",
    "provider_id": "env"
  }
}
```
Output:
```json
{
  "result": {
    "content_type": "text/plain",
    "evidence_anchor": {
      "anchor_type": "env",
      "anchor_value": "DEPLOY_ENV"
    },
    "evidence_hash": {
      "algorithm": "sha256",
      "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
    },
    "evidence_ref": null,
    "signature": null,
    "value": {
      "kind": "json",
      "value": "production"
    }
  }
}
```
## runpack_export

Export deterministic runpack artifacts for offline verification.

### Inputs

- `generated_at` (required): Timestamp recorded in the manifest.
- `include_verification` (required): Generate a verification report artifact.
- `manifest_name` (optional, nullable): Optional override for the manifest file name.
- `output_dir` (required): Output directory path.
- `run_id` (required): Run identifier.
- `scenario_id` (required): Scenario identifier.

### Outputs

- `manifest` (required): Type: object.
- `report` (required, nullable): One of: null, object.

### Notes

- Writes manifest and logs to output_dir; generated_at is recorded in the manifest.
- include_verification adds a verification report artifact.
- Use after runs complete or for audit snapshots.

### Example

Export a runpack with manifest metadata.

Input:
```json
{
  "generated_at": {
    "kind": "unix_millis",
    "value": 1710000000000
  },
  "include_verification": false,
  "manifest_name": "manifest.json",
  "output_dir": "/var/lib/decision-gate/runpacks/run-0001",
  "run_id": "run-0001",
  "scenario_id": "example-scenario"
}
```
Output:
```json
{
  "manifest": {
    "artifacts": [
      {
        "artifact_id": "decision_log",
        "content_type": "application/json",
        "hash": {
          "algorithm": "sha256",
          "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
        },
        "kind": "decision_log",
        "path": "decision_log.json",
        "required": true
      }
    ],
    "generated_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "hash_algorithm": "sha256",
    "integrity": {
      "file_hashes": [
        {
          "hash": {
            "algorithm": "sha256",
            "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
          },
          "path": "decision_log.json"
        }
      ],
      "root_hash": {
        "algorithm": "sha256",
        "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
      }
    },
    "manifest_version": "v1",
    "run_id": "run-0001",
    "scenario_id": "example-scenario",
    "spec_hash": {
      "algorithm": "sha256",
      "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
    },
    "verifier_mode": "offline_strict"
  },
  "report": null
}
```
## runpack_verify

Verify a runpack manifest and artifacts offline.

### Inputs

- `manifest_path` (required): Manifest path relative to runpack root.
- `runpack_dir` (required): Runpack root directory.

### Outputs

- `report` (required): Type: object.
- `status` (required): Runpack verification status.

### Notes

- Validates hashes, integrity root, and decision log structure.
- Fails closed on missing or tampered files.
- Use in CI or offline audit pipelines.

### Example

Verify a runpack manifest and artifacts offline.

Input:
```json
{
  "manifest_path": "manifest.json",
  "runpack_dir": "/var/lib/decision-gate/runpacks/run-0001"
}
```
Output:
```json
{
  "report": {
    "checked_files": 12,
    "errors": [],
    "status": "pass"
  },
  "status": "pass"
}
```
