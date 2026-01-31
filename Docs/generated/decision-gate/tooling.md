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

- `authoring.md`: authoring formats and normalization guidance.
- `examples/scenario.json`: full ScenarioSpec example.
- `examples/scenario.ron`: authoring-friendly ScenarioSpec example.
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
| providers_list | List registered evidence providers and capabilities summary. |
| provider_contract_get | Fetch the canonical provider contract JSON and hash for a provider. |
| provider_check_schema_get | Fetch check schema details (params/result/comparators) for a provider. |
| schemas_register | Register a data shape schema for a tenant and namespace. |
| schemas_list | List registered data shapes for a tenant and namespace. |
| schemas_get | Fetch a specific data shape by identifier and version. |
| scenarios_list | List registered scenarios for a tenant and namespace. |
| precheck | Evaluate a scenario against asserted data without mutating state. |

## scenario_define

Register a ScenarioSpec, validate it, and return the canonical hash used for integrity checks.

### Inputs

- `spec` (required): Scenario specification to register.

### Outputs

- `scenario_id` (required): Scenario identifier.
- `spec_hash` (required): Type: object.

### Notes

- Use before starting runs; scenario_id becomes the stable handle for later calls.
- Validates stage/gate/condition IDs, RET trees, and condition references.
- Spec hash is deterministic; store it for audit and runpack integrity.
- Fails closed on invalid specs or duplicate scenario IDs.

### Example

Register the example scenario spec.

Input:
```json
{
  "spec": {
    "conditions": [
      {
        "comparator": "equals",
        "condition_id": "env_is_prod",
        "expected": "production",
        "policy_tags": [],
        "query": {
          "check_id": "get",
          "params": {
            "key": "DEPLOY_ENV"
          },
          "provider_id": "env"
        }
      },
      {
        "comparator": "equals",
        "condition_id": "after_freeze",
        "expected": true,
        "policy_tags": [],
        "query": {
          "check_id": "after",
          "params": {
            "timestamp": 1710000000000
          },
          "provider_id": "time"
        }
      }
    ],
    "default_tenant_id": null,
    "namespace_id": 1,
    "policies": [],
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
              "Condition": "env_is_prod"
            }
          },
          {
            "gate_id": "time_gate",
            "requirement": {
              "Condition": "after_freeze"
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
- `namespace_id` (required): Namespace identifier.
- `packets` (required): Type: array.
- `run_id` (required): Run identifier.
- `scenario_id` (required): Scenario identifier.
- `spec_hash` (required): Type: object.
- `stage_entered_at` (required): One of: object, object.
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
    "namespace_id": 1,
    "policy_tags": [],
    "run_id": "run-0001",
    "scenario_id": "example-scenario",
    "tenant_id": 1
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
  "namespace_id": 1,
  "packets": [],
  "run_id": "run-0001",
  "scenario_id": "example-scenario",
  "spec_hash": {
    "algorithm": "sha256",
    "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
  },
  "stage_entered_at": {
    "kind": "unix_millis",
    "value": 1710000000000
  },
  "status": "active",
  "submissions": [],
  "tenant_id": 1,
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
- `namespace_id` (optional): Namespace identifier.
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
    "namespace_id": 1,
    "requested_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "run_id": "run-0001",
    "tenant_id": 1
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

- `feedback` (optional, nullable): Optional feedback level override for scenario_next.
- `request` (required): Next request payload from an agent.
- `scenario_id` (required): Scenario identifier.

### Outputs

- `decision` (required): Type: object.
- `feedback` (optional, nullable): One of: null, object.
- `packets` (required): Type: array.
- `status` (required): Type: string.

### Notes

- Idempotent by trigger_id; repeated calls return the same decision.
- Records decision, evidence, and packet disclosures in run state.
- Requires an active run; completed or failed runs do not advance.
- Optional feedback can include gate trace or evidence when permitted by server feedback policy.

### Example

Example 1: Evaluate the next agent-driven step for a run.

Input:
```json
{
  "request": {
    "agent_id": "agent-alpha",
    "correlation_id": null,
    "namespace_id": 1,
    "run_id": "run-0001",
    "tenant_id": 1,
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
Example 2: Evaluate a run and request trace feedback.

Input:
```json
{
  "feedback": "trace",
  "request": {
    "agent_id": "agent-alpha",
    "correlation_id": null,
    "namespace_id": 1,
    "run_id": "run-0001",
    "tenant_id": 1,
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
  "feedback": {
    "gate_evaluations": [],
    "level": "trace"
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
    "namespace_id": 1,
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
    },
    "tenant_id": 1
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
    "namespace_id": 1,
    "payload": null,
    "run_id": "run-0001",
    "source_id": "scheduler-01",
    "tenant_id": 1,
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
- Requires provider_id, check_id, and full EvidenceContext.

### Example

Query an evidence provider using the run context.

Input:
```json
{
  "context": {
    "correlation_id": null,
    "namespace_id": 1,
    "run_id": "run-0001",
    "scenario_id": "example-scenario",
    "stage_id": "main",
    "tenant_id": 1,
    "trigger_id": "trigger-0001",
    "trigger_time": {
      "kind": "unix_millis",
      "value": 1710000000000
    }
  },
  "query": {
    "check_id": "get",
    "params": {
      "key": "DEPLOY_ENV"
    },
    "provider_id": "env"
  }
}
```
Output:
```json
{
  "result": {
    "content_type": "text/plain",
    "error": null,
    "evidence_anchor": {
      "anchor_type": "env",
      "anchor_value": "DEPLOY_ENV"
    },
    "evidence_hash": {
      "algorithm": "sha256",
      "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
    },
    "evidence_ref": null,
    "lane": "verified",
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
- `namespace_id` (required): Namespace identifier.
- `output_dir` (optional, nullable): Optional output directory (required for filesystem export).
- `run_id` (required): Run identifier.
- `scenario_id` (required): Scenario identifier.
- `tenant_id` (required): Tenant identifier.

### Outputs

- `manifest` (required): Type: object.
- `report` (required, nullable): One of: null, object.
- `storage_uri` (optional, nullable): Optional storage URI for managed runpack storage backends.

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
  "namespace_id": 1,
  "output_dir": "/var/lib/decision-gate/runpacks/run-0001",
  "run_id": "run-0001",
  "scenario_id": "example-scenario",
  "tenant_id": 1
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
    "namespace_id": 1,
    "run_id": "run-0001",
    "scenario_id": "example-scenario",
    "spec_hash": {
      "algorithm": "sha256",
      "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
    },
    "tenant_id": 1,
    "verifier_mode": "offline_strict"
  },
  "report": null,
  "storage_uri": null
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
## providers_list

List registered evidence providers and capabilities summary.

### Inputs


### Outputs

- `providers` (required): Type: array.

### Notes

- Returns provider identifiers and transport metadata.
- Results are scoped by auth policy.

### Example

List registered evidence providers.

Input:
```json
{}
```
Output:
```json
{
  "providers": [
    {
      "checks": [
        "get"
      ],
      "provider_id": "env",
      "transport": "builtin"
    }
  ]
}
```
## provider_contract_get

Fetch the canonical provider contract JSON and hash for a provider.

### Inputs

- `provider_id` (required): Provider identifier.

### Outputs

- `contract` (required): Type: object.
- `contract_hash` (required): Type: object.
- `provider_id` (required): Provider identifier.
- `source` (required): Contract source origin.
- `version` (required, nullable): Optional contract version label.

### Notes

- Returns the provider contract as loaded by the MCP server.
- Includes a canonical hash for audit and reproducibility.
- Subject to provider disclosure policy and authz.

### Example

Fetch the contract JSON for a provider.

Input:
```json
{
  "provider_id": "json"
}
```
Output:
```json
{
  "contract": {
    "checks": [],
    "config_schema": {
      "additionalProperties": false,
      "type": "object"
    },
    "description": "Reads JSON or YAML files and evaluates JSONPath.",
    "name": "JSON Provider",
    "notes": [],
    "provider_id": "json",
    "transport": "builtin"
  },
  "contract_hash": {
    "algorithm": "sha256",
    "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
  },
  "provider_id": "json",
  "source": "builtin",
  "version": null
}
```
## provider_check_schema_get

Fetch check schema details (params/result/comparators) for a provider.

### Inputs

- `check_id` (required): Provider check identifier.
- `provider_id` (required): Provider identifier.

### Outputs

- `allowed_comparators` (required): Comparator allow-list for this check.
- `anchor_types` (required): Anchor types emitted by this check.
- `check_id` (required): Check identifier.
- `content_types` (required): Content types for check output.
- `contract_hash` (required): Type: object.
- `determinism` (required): Determinism classification for provider checks.
- `examples` (required): Type: array.
- `params_required` (required): Whether params are required for this check.
- `params_schema` (required): JSON schema for check params.
- `provider_id` (required): Provider identifier.
- `result_schema` (required): JSON schema for check result value.

### Notes

- Returns compiled schema metadata for a single check.
- Includes comparator allow-lists and check examples.
- Subject to provider disclosure policy and authz.

### Example

Fetch check schema details for a provider.

Input:
```json
{
  "check_id": "path",
  "provider_id": "json"
}
```
Output:
```json
{
  "allowed_comparators": [
    "equals",
    "in_set",
    "exists",
    "not_exists"
  ],
  "anchor_types": [],
  "check_id": "path",
  "content_types": [
    "application/json"
  ],
  "contract_hash": {
    "algorithm": "sha256",
    "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
  },
  "determinism": "external",
  "examples": [],
  "params_required": true,
  "params_schema": {
    "properties": {
      "file": {
        "type": "string"
      },
      "jsonpath": {
        "type": "string"
      }
    },
    "required": [
      "file"
    ],
    "type": "object"
  },
  "provider_id": "json",
  "result_schema": {
    "type": [
      "null",
      "string",
      "number",
      "boolean",
      "array",
      "object"
    ]
  }
}
```
## schemas_register

Register a data shape schema for a tenant and namespace.

### Inputs

- `record` (required): Type: object.

### Outputs

- `record` (required): Type: object.

### Notes

- Schemas are immutable; registering the same version twice fails.
- Provide created_at to record when the schema was authored.

### Example

Register a data shape schema.

Input:
```json
{
  "record": {
    "created_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "description": "Asserted payload schema.",
    "namespace_id": 1,
    "schema": {
      "additionalProperties": false,
      "properties": {
        "deploy_env": {
          "type": "string"
        }
      },
      "required": [
        "deploy_env"
      ],
      "type": "object"
    },
    "schema_id": "asserted_payload",
    "tenant_id": 1,
    "version": "v1"
  }
}
```
Output:
```json
{
  "record": {
    "created_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "description": "Asserted payload schema.",
    "namespace_id": 1,
    "schema": {
      "additionalProperties": false,
      "properties": {
        "deploy_env": {
          "type": "string"
        }
      },
      "required": [
        "deploy_env"
      ],
      "type": "object"
    },
    "schema_id": "asserted_payload",
    "tenant_id": 1,
    "version": "v1"
  }
}
```
## schemas_list

List registered data shapes for a tenant and namespace.

### Inputs

- `cursor` (optional, nullable): One of: null, string.
- `limit` (optional): Maximum number of records to return.
- `namespace_id` (required): Namespace identifier.
- `tenant_id` (required): Tenant identifier.

### Outputs

- `items` (required): Type: array.
- `next_token` (required, nullable): One of: null, string.

### Notes

- Requires tenant_id and namespace_id.
- Supports pagination via cursor + limit.

### Example

List data shapes for a namespace.

Input:
```json
{
  "cursor": null,
  "limit": 50,
  "namespace_id": 1,
  "tenant_id": 1
}
```
Output:
```json
{
  "items": [
    {
      "created_at": {
        "kind": "unix_millis",
        "value": 1710000000000
      },
      "description": "Asserted payload schema.",
      "namespace_id": 1,
      "schema": {
        "additionalProperties": false,
        "properties": {
          "deploy_env": {
            "type": "string"
          }
        },
        "required": [
          "deploy_env"
        ],
        "type": "object"
      },
      "schema_id": "asserted_payload",
      "tenant_id": 1,
      "version": "v1"
    }
  ],
  "next_token": null
}
```
## schemas_get

Fetch a specific data shape by identifier and version.

### Inputs

- `namespace_id` (required): Namespace identifier.
- `schema_id` (required): Data shape identifier.
- `tenant_id` (required): Tenant identifier.
- `version` (required): Data shape version identifier.

### Outputs

- `record` (required): Type: object.

### Notes

- Requires tenant_id, namespace_id, schema_id, and version.
- Fails closed when schema is missing.

### Example

Fetch a data shape by identifier and version.

Input:
```json
{
  "namespace_id": 1,
  "schema_id": "asserted_payload",
  "tenant_id": 1,
  "version": "v1"
}
```
Output:
```json
{
  "record": {
    "created_at": {
      "kind": "unix_millis",
      "value": 1710000000000
    },
    "description": "Asserted payload schema.",
    "namespace_id": 1,
    "schema": {
      "additionalProperties": false,
      "properties": {
        "deploy_env": {
          "type": "string"
        }
      },
      "required": [
        "deploy_env"
      ],
      "type": "object"
    },
    "schema_id": "asserted_payload",
    "tenant_id": 1,
    "version": "v1"
  }
}
```
## scenarios_list

List registered scenarios for a tenant and namespace.

### Inputs

- `cursor` (optional, nullable): One of: null, string.
- `limit` (optional): Maximum number of records to return.
- `namespace_id` (required): Namespace identifier.
- `tenant_id` (required): Tenant identifier.

### Outputs

- `items` (required): Type: array.
- `next_token` (required, nullable): One of: null, string.

### Notes

- Requires tenant_id and namespace_id.
- Returns scenario identifiers and hashes.

### Example

List scenarios for a namespace.

Input:
```json
{
  "cursor": null,
  "limit": 50,
  "namespace_id": 1,
  "tenant_id": 1
}
```
Output:
```json
{
  "items": [
    {
      "namespace_id": 1,
      "scenario_id": "example-scenario",
      "spec_hash": {
        "algorithm": "sha256",
        "value": "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21"
      }
    }
  ],
  "next_token": null
}
```
## precheck

Evaluate a scenario against asserted data without mutating state.

### Inputs

- `data_shape` (required): Type: object.
- `namespace_id` (required): Namespace identifier.
- `payload` (required): Asserted data payload.
- `scenario_id` (optional, nullable): One of: null, string.
- `spec` (optional, nullable): One of: null, ref decision-gate://contract/schemas/scenario.schema.json.
- `stage_id` (optional, nullable): One of: null, string.
- `tenant_id` (required): Tenant identifier.

### Outputs

- `decision` (required): One of: object, object, object, object, object.
- `gate_evaluations` (required): Type: array.

### Notes

- Validates asserted data against a registered shape.
- Does not mutate run state; intended for simulation.

### Example

Precheck a scenario with asserted data.

Input:
```json
{
  "data_shape": {
    "schema_id": "asserted_payload",
    "version": "v1"
  },
  "namespace_id": 1,
  "payload": {
    "deploy_env": "production"
  },
  "scenario_id": "example-scenario",
  "spec": null,
  "stage_id": null,
  "tenant_id": 1
}
```
Output:
```json
{
  "decision": {
    "kind": "hold",
    "summary": {
      "policy_tags": [],
      "retry_hint": "await_evidence",
      "status": "hold",
      "unmet_gates": [
        "ready"
      ]
    }
  },
  "gate_evaluations": []
}
```
