// decision-gate-contract/src/schemas.rs
// ============================================================================
// Module: Contract Schemas
// Description: JSON schema builders for core Decision Gate data shapes.
// Purpose: Provide canonical validation schemas for scenarios, config, and tools.
// Dependencies: serde_json
// ============================================================================

//! ## Overview
//! This module defines JSON Schema payloads that mirror core Decision Gate
//! structs. These schemas are used to validate authoring input and to
//! generate docs and SDKs from a single, canonical source.
//! Security posture: schemas gate untrusted inputs; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde_json::Value;
use serde_json::json;

// ============================================================================
// SECTION: Public Schema Entrypoints
// ============================================================================

/// Returns the JSON schema for `ScenarioSpec`.
#[allow(clippy::too_many_lines, reason = "schema definition is intentionally data-heavy")]
#[must_use]
pub fn scenario_schema() -> Value {
    let timestamp = timestamp_schema();
    let hash_digest = hash_digest_schema();
    let packet_payload = packet_payload_schema();
    let advance_to = advance_to_schema();
    let requirement = requirement_schema();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "decision-gate://contract/schemas/scenario.schema.json",
        "title": "Decision Gate ScenarioSpec",
        "description": "Canonical scenario specification used by Decision Gate.",
        "type": "object",
        "required": [
            "scenario_id",
            "spec_version",
            "stages",
            "predicates",
            "policies",
            "schemas"
        ],
        "properties": {
            "scenario_id": schema_for_identifier("Scenario identifier."),
            "spec_version": schema_for_identifier("Specification version identifier."),
            "stages": {
                "type": "array",
                "items": { "$ref": "#/$defs/StageSpec" },
                "minItems": 1
            },
            "predicates": {
                "type": "array",
                "items": { "$ref": "#/$defs/PredicateSpec" }
            },
            "policies": {
                "type": "array",
                "items": { "$ref": "#/$defs/PolicyRef" }
            },
            "schemas": {
                "type": "array",
                "items": { "$ref": "#/$defs/SchemaRef" }
            },
            "default_tenant_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Optional default tenant identifier.")
                ]
            }
        },
        "additionalProperties": false,
        "$defs": {
            "StageSpec": {
                "type": "object",
                "required": [
                    "stage_id",
                    "entry_packets",
                    "gates",
                    "advance_to",
                    "on_timeout"
                ],
                "properties": {
                    "stage_id": schema_for_identifier("Stage identifier."),
                    "entry_packets": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/PacketSpec" }
                    },
                    "gates": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/GateSpec" }
                    },
                    "advance_to": advance_to,
                    "timeout": {
                        "oneOf": [
                            { "type": "null" },
                            { "$ref": "#/$defs/TimeoutSpec" }
                        ]
                    },
                    "on_timeout": { "$ref": "#/$defs/TimeoutPolicy" }
                },
                "additionalProperties": false
            },
            "AdvanceTo": advance_to_schema(),
            "BranchRule": {
                "type": "object",
                "required": ["gate_id", "outcome", "next_stage_id"],
                "properties": {
                    "gate_id": schema_for_identifier("Gate identifier referenced by the branch rule."),
                    "outcome": { "$ref": "#/$defs/GateOutcome" },
                    "next_stage_id": schema_for_identifier("Stage identifier to advance to.")
                },
                "additionalProperties": false
            },
            "GateOutcome": {
                "type": "string",
                "enum": ["true", "false", "unknown"],
                "description": "Gate outcome for branch routing."
            },
            "TimeoutSpec": {
                "type": "object",
                "required": ["timeout_ms", "policy_tags"],
                "properties": {
                    "timeout_ms": {
                        "type": "integer",
                        "minimum": 0
                    },
                    "policy_tags": schema_for_string_array("Policy tags applied to timeout handling.")
                },
                "additionalProperties": false
            },
            "TimeoutPolicy": {
                "type": "string",
                "enum": ["fail", "advance_with_flag", "alternate_branch"],
                "description": "Timeout handling policy."
            },
            "GateSpec": {
                "type": "object",
                "required": ["gate_id", "requirement"],
                "properties": {
                    "gate_id": schema_for_identifier("Gate identifier."),
                    "requirement": requirement
                },
                "additionalProperties": false
            },
            "Requirement": requirement,
            "PredicateSpec": {
                "type": "object",
                "required": ["predicate", "query", "comparator", "policy_tags"],
                "properties": {
                    "predicate": schema_for_identifier("Predicate identifier."),
                    "query": evidence_query_schema(),
                    "comparator": comparator_schema(),
                    "expected": schema_for_json_value("Expected comparison value."),
                    "policy_tags": schema_for_string_array("Policy tags applied to predicate evaluation.")
                },
                "additionalProperties": false
            },
            "EvidenceQuery": evidence_query_schema(),
            "Comparator": comparator_schema(),
            "PacketSpec": {
                "type": "object",
                "required": [
                    "packet_id",
                    "schema_id",
                    "content_type",
                    "visibility_labels",
                    "policy_tags",
                    "payload"
                ],
                "properties": {
                    "packet_id": schema_for_identifier("Packet identifier."),
                    "schema_id": schema_for_identifier("Schema identifier."),
                    "content_type": schema_for_string("Content type for payload."),
                    "visibility_labels": schema_for_string_array("Visibility labels for packet disclosure."),
                    "policy_tags": schema_for_string_array("Policy tags applied to packet dispatch."),
                    "expiry": {
                        "oneOf": [
                            { "type": "null" },
                            timestamp
                        ]
                    },
                    "payload": packet_payload
                },
                "additionalProperties": false
            },
            "PacketPayload": packet_payload_schema(),
            "ContentRef": {
                "type": "object",
                "required": ["uri", "content_hash", "encryption"],
                "properties": {
                    "uri": schema_for_string("External content reference URI."),
                    "content_hash": hash_digest,
                    "encryption": {
                        "oneOf": [
                            { "type": "null" },
                            schema_for_string("Optional encryption metadata identifier.")
                        ]
                    }
                },
                "additionalProperties": false
            },
            "HashDigest": hash_digest,
            "Timestamp": timestamp,
            "PolicyRef": {
                "type": "object",
                "required": ["policy_id"],
                "properties": {
                    "policy_id": schema_for_identifier("Policy identifier."),
                    "description": {
                        "oneOf": [
                            { "type": "null" },
                            schema_for_string("Optional policy description.")
                        ]
                    }
                },
                "additionalProperties": false
            },
            "SchemaRef": {
                "type": "object",
                "required": ["schema_id"],
                "properties": {
                    "schema_id": schema_for_identifier("Schema identifier."),
                    "version": {
                        "oneOf": [
                            { "type": "null" },
                            schema_for_string("Schema version string.")
                        ]
                    },
                    "uri": {
                        "oneOf": [
                            { "type": "null" },
                            schema_for_string("Schema registry URI.")
                        ]
                    }
                },
                "additionalProperties": false
            }
        }
    })
}

/// Returns the JSON schema for `decision-gate.toml`.
#[must_use]
pub fn config_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "decision-gate://contract/schemas/config.schema.json",
        "title": "Decision Gate MCP Configuration",
        "description": "Configuration for the Decision Gate MCP server and providers.",
        "type": "object",
        "properties": {
            "server": server_config_schema(),
            "trust": trust_config_schema(),
            "evidence": evidence_policy_schema(),
            "providers": {
                "type": "array",
                "items": provider_config_schema(),
                "default": []
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `EvidenceQuery`.
#[must_use]
pub fn evidence_query_schema() -> Value {
    json!({
        "type": "object",
        "required": ["provider_id", "predicate"],
        "properties": {
            "provider_id": schema_for_identifier("Evidence provider identifier."),
            "predicate": schema_for_string("Provider predicate name."),
            "params": schema_for_json_value("Provider-specific parameter payload.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `EvidenceResult`.
#[must_use]
pub fn evidence_result_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "value",
            "evidence_hash",
            "evidence_ref",
            "evidence_anchor",
            "signature",
            "content_type"
        ],
        "properties": {
            "value": {
                "oneOf": [
                    { "type": "null" },
                    evidence_value_schema()
                ]
            },
            "evidence_hash": {
                "oneOf": [
                    { "type": "null" },
                    hash_digest_schema()
                ]
            },
            "evidence_ref": {
                "oneOf": [
                    { "type": "null" },
                    evidence_ref_schema()
                ]
            },
            "evidence_anchor": {
                "oneOf": [
                    { "type": "null" },
                    evidence_anchor_schema()
                ]
            },
            "signature": {
                "oneOf": [
                    { "type": "null" },
                    evidence_signature_schema()
                ]
            },
            "content_type": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Evidence content type.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `RunConfig`.
#[must_use]
pub fn run_config_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "tenant_id",
            "run_id",
            "scenario_id",
            "dispatch_targets",
            "policy_tags"
        ],
        "properties": {
            "tenant_id": schema_for_identifier("Tenant identifier."),
            "run_id": schema_for_identifier("Run identifier."),
            "scenario_id": schema_for_identifier("Scenario identifier."),
            "dispatch_targets": {
                "type": "array",
                "items": dispatch_target_schema()
            },
            "policy_tags": schema_for_string_array("Policy tags applied to the run.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `TriggerEvent`.
#[must_use]
pub fn trigger_event_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "trigger_id",
            "run_id",
            "kind",
            "time",
            "source_id"
        ],
        "properties": {
            "trigger_id": schema_for_identifier("Trigger identifier."),
            "run_id": schema_for_identifier("Run identifier."),
            "kind": trigger_kind_schema(),
            "time": timestamp_schema(),
            "source_id": schema_for_string("Trigger source identifier."),
            "payload_ref": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Optional payload reference.")
                ]
            },
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `StatusRequest`.
#[must_use]
pub fn status_request_schema() -> Value {
    json!({
        "type": "object",
        "required": ["run_id", "requested_at"],
        "properties": {
            "run_id": schema_for_identifier("Run identifier."),
            "requested_at": timestamp_schema(),
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `NextRequest`.
#[must_use]
pub fn next_request_schema() -> Value {
    json!({
        "type": "object",
        "required": ["run_id", "trigger_id", "agent_id", "time"],
        "properties": {
            "run_id": schema_for_identifier("Run identifier."),
            "trigger_id": schema_for_identifier("Trigger identifier."),
            "agent_id": schema_for_string("Agent identifier."),
            "time": timestamp_schema(),
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `SubmitRequest`.
#[must_use]
pub fn submit_request_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "run_id",
            "submission_id",
            "payload",
            "content_type",
            "submitted_at"
        ],
        "properties": {
            "run_id": schema_for_identifier("Run identifier."),
            "submission_id": schema_for_string("Submission identifier."),
            "payload": packet_payload_schema(),
            "content_type": schema_for_string("Submission content type."),
            "submitted_at": timestamp_schema(),
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `ScenarioStatus`.
#[must_use]
pub fn scenario_status_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "run_id",
            "scenario_id",
            "current_stage_id",
            "status",
            "last_decision",
            "issued_packet_ids",
            "safe_summary"
        ],
        "properties": {
            "run_id": schema_for_identifier("Run identifier."),
            "scenario_id": schema_for_identifier("Scenario identifier."),
            "current_stage_id": schema_for_identifier("Current stage identifier."),
            "status": run_status_schema(),
            "last_decision": {
                "oneOf": [
                    { "type": "null" },
                    decision_record_schema()
                ]
            },
            "issued_packet_ids": {
                "type": "array",
                "items": schema_for_identifier("Packet identifier.")
            },
            "safe_summary": {
                "oneOf": [
                    { "type": "null" },
                    safe_summary_schema()
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `NextResult`.
#[must_use]
pub fn next_result_schema() -> Value {
    json!({
        "type": "object",
        "required": ["decision", "packets", "status"],
        "properties": {
            "decision": decision_record_schema(),
            "packets": {
                "type": "array",
                "items": packet_record_schema()
            },
            "status": run_status_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `SubmitResult`.
#[must_use]
pub fn submit_result_schema() -> Value {
    json!({
        "type": "object",
        "required": ["record"],
        "properties": {
            "record": submission_record_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `TriggerResult`.
#[must_use]
pub fn trigger_result_schema() -> Value {
    next_result_schema()
}

/// Returns the JSON schema for `RunState`.
#[must_use]
pub fn run_state_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "tenant_id",
            "run_id",
            "scenario_id",
            "spec_hash",
            "current_stage_id",
            "status",
            "dispatch_targets",
            "triggers",
            "gate_evals",
            "decisions",
            "packets",
            "submissions",
            "tool_calls"
        ],
        "properties": {
            "tenant_id": schema_for_identifier("Tenant identifier."),
            "run_id": schema_for_identifier("Run identifier."),
            "scenario_id": schema_for_identifier("Scenario identifier."),
            "spec_hash": hash_digest_schema(),
            "current_stage_id": schema_for_identifier("Current stage identifier."),
            "status": run_status_schema(),
            "dispatch_targets": {
                "type": "array",
                "items": dispatch_target_schema()
            },
            "triggers": {
                "type": "array",
                "items": trigger_record_schema()
            },
            "gate_evals": {
                "type": "array",
                "items": gate_eval_record_schema()
            },
            "decisions": {
                "type": "array",
                "items": decision_record_schema()
            },
            "packets": {
                "type": "array",
                "items": packet_record_schema()
            },
            "submissions": {
                "type": "array",
                "items": submission_record_schema()
            },
            "tool_calls": {
                "type": "array",
                "items": tool_call_record_schema()
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `RunpackManifest`.
#[must_use]
pub fn runpack_manifest_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "manifest_version",
            "generated_at",
            "scenario_id",
            "run_id",
            "spec_hash",
            "hash_algorithm",
            "verifier_mode",
            "integrity",
            "artifacts"
        ],
        "properties": {
            "manifest_version": schema_for_string("Runpack manifest version."),
            "generated_at": timestamp_schema(),
            "scenario_id": schema_for_identifier("Scenario identifier."),
            "run_id": schema_for_identifier("Run identifier."),
            "spec_hash": hash_digest_schema(),
            "hash_algorithm": hash_algorithm_schema(),
            "verifier_mode": verifier_mode_schema(),
            "integrity": runpack_integrity_schema(),
            "artifacts": {
                "type": "array",
                "items": artifact_record_schema()
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `VerificationReport`.
#[must_use]
pub fn verification_report_schema() -> Value {
    json!({
        "type": "object",
        "required": ["status", "checked_files", "errors"],
        "properties": {
            "status": verification_status_schema(),
            "checked_files": {
                "type": "integer",
                "minimum": 0
            },
            "errors": schema_for_string_array("Verification error messages.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `VerificationStatus`.
#[must_use]
pub fn verification_status_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["pass", "fail"],
        "description": "Runpack verification status."
    })
}

/// Returns the JSON schema for `HashDigest`.
#[must_use]
pub fn hash_digest_schema() -> Value {
    json!({
        "type": "object",
        "required": ["algorithm", "value"],
        "properties": {
            "algorithm": {
                "type": "string",
                "enum": ["sha256"]
            },
            "value": schema_for_string("Lowercase hex digest.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for `Timestamp`.
#[must_use]
pub fn timestamp_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind", "value"],
                "properties": {
                    "kind": { "const": "unix_millis" },
                    "value": { "type": "integer" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "value"],
                "properties": {
                    "kind": { "const": "logical" },
                    "value": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for `DispatchTarget`.
#[must_use]
pub fn dispatch_target_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind", "agent_id"],
                "properties": {
                    "kind": { "const": "agent" },
                    "agent_id": schema_for_string("Agent identifier.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "session_id"],
                "properties": {
                    "kind": { "const": "session" },
                    "session_id": schema_for_string("Session identifier.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "system", "target"],
                "properties": {
                    "kind": { "const": "external" },
                    "system": schema_for_string("External system name."),
                    "target": schema_for_string("External system target.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "channel"],
                "properties": {
                    "kind": { "const": "channel" },
                    "channel": schema_for_string("Broadcast channel identifier.")
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for `PacketPayload`.
#[must_use]
pub fn packet_payload_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind", "value"],
                "properties": {
                    "kind": { "const": "json" },
                    "value": schema_for_json_value("Inline JSON payload.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "bytes"],
                "properties": {
                    "kind": { "const": "bytes" },
                    "bytes": {
                        "type": "array",
                        "items": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 255
                        }
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "content_ref"],
                "properties": {
                    "kind": { "const": "external" },
                    "content_ref": content_ref_schema()
                },
                "additionalProperties": false
            }
        ]
    })
}

// ============================================================================
// SECTION: Private Schema Helpers
// ============================================================================

/// Returns a JSON schema for a string identifier.
#[must_use]
fn schema_for_identifier(description: &str) -> Value {
    schema_for_string(description)
}

/// Returns a JSON schema for a plain string.
#[must_use]
fn schema_for_string(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

/// Returns a JSON schema for string arrays.
#[must_use]
fn schema_for_string_array(description: &str) -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" },
        "description": description
    })
}

/// Returns a permissive JSON schema accepting any JSON value.
#[must_use]
fn schema_for_json_value(description: &str) -> Value {
    json!({
        "type": ["null", "boolean", "number", "string", "array", "object"],
        "description": description
    })
}

/// Returns the JSON schema for `Comparator`.
#[must_use]
fn comparator_schema() -> Value {
    json!({
        "type": "string",
        "enum": [
            "equals",
            "not_equals",
            "greater_than",
            "greater_than_or_equal",
            "less_than",
            "less_than_or_equal",
            "contains",
            "in_set",
            "exists",
            "not_exists"
        ],
        "description": "Comparator applied to evidence values."
    })
}

/// Returns the JSON schema for `Requirement<PredicateKey>`.
#[must_use]
fn requirement_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["And"],
                "properties": {
                    "And": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/Requirement" }
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["Or"],
                "properties": {
                    "Or": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/Requirement" }
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["Not"],
                "properties": {
                    "Not": { "$ref": "#/$defs/Requirement" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["RequireGroup"],
                "properties": {
                    "RequireGroup": {
                        "type": "object",
                        "required": ["min", "reqs"],
                        "properties": {
                            "min": { "type": "integer", "minimum": 0, "maximum": 255 },
                            "reqs": {
                                "type": "array",
                                "items": { "$ref": "#/$defs/Requirement" }
                            }
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["Predicate"],
                "properties": {
                    "Predicate": schema_for_identifier("Predicate identifier reference.")
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for `AdvanceTo`.
#[must_use]
fn advance_to_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "const": "linear" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "stage_id"],
                "properties": {
                    "kind": { "const": "fixed" },
                    "stage_id": schema_for_identifier("Fixed stage identifier.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "branches", "default"],
                "properties": {
                    "kind": { "const": "branch" },
                    "branches": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/BranchRule" }
                    },
                    "default": {
                        "oneOf": [
                            { "type": "null" },
                            schema_for_identifier("Default branch stage identifier.")
                        ]
                    }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": { "const": "terminal" }
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for provider config.
#[must_use]
fn provider_config_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "type"],
        "properties": {
            "name": schema_for_identifier("Provider identifier."),
            "type": {
                "type": "string",
                "enum": ["builtin", "mcp"]
            },
            "command": {
                "type": "array",
                "items": { "type": "string" },
                "default": []
            },
            "url": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Provider HTTP URL.")
                ],
                "default": null
            },
            "allow_insecure_http": {
                "type": "boolean",
                "default": false
            },
            "auth": {
                "oneOf": [
                    { "type": "null" },
                    provider_auth_schema()
                ],
                "default": null
            },
            "trust": {
                "oneOf": [
                    { "type": "null" },
                    trust_policy_schema()
                ],
                "default": null
            },
            "allow_raw": {
                "type": "boolean",
                "default": false
            },
            "config": schema_for_json_value("Provider-specific config blob.")
        },
        "allOf": [
            {
                "if": {
                    "properties": {
                        "type": { "const": "mcp" }
                    }
                },
                "then": {
                    "anyOf": [
                        { "required": ["command"] },
                        { "required": ["url"] }
                    ]
                }
            }
        ],
        "additionalProperties": false
    })
}

/// Returns the JSON schema for provider auth config.
#[must_use]
fn provider_auth_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "bearer_token": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Bearer token for MCP providers.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for server configuration.
#[must_use]
fn server_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "transport": {
                "type": "string",
                "enum": ["stdio", "http", "sse"],
                "default": "stdio"
            },
            "bind": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Bind address for HTTP/SSE transport.")
                ],
                "default": null
            },
            "max_body_bytes": {
                "type": "integer",
                "minimum": 0,
                "default": 1_048_576
            }
        },
        "allOf": [
            {
                "if": {
                    "properties": {
                        "transport": { "enum": ["http", "sse"] }
                    }
                },
                "then": {
                    "required": ["bind"],
                    "properties": {
                        "bind": schema_for_string("Bind address for HTTP/SSE transport.")
                    }
                }
            }
        ],
        "additionalProperties": false
    })
}

/// Returns the JSON schema for trust configuration.
#[must_use]
fn trust_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "default_policy": trust_policy_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for trust policy.
#[must_use]
fn trust_policy_schema() -> Value {
    json!({
        "oneOf": [
            { "type": "string", "enum": ["audit"], "default": "audit" },
            {
                "type": "object",
                "required": ["require_signature"],
                "properties": {
                    "require_signature": {
                        "type": "object",
                        "required": ["keys"],
                        "properties": {
                            "keys": schema_for_string_array("Signature key identifiers.")
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for evidence policy configuration.
#[must_use]
fn evidence_policy_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "allow_raw_values": { "type": "boolean", "default": false },
            "require_provider_opt_in": { "type": "boolean", "default": true }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for trigger kind.
#[must_use]
fn trigger_kind_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["agent_request_next", "tick", "external_event", "backend_event"]
    })
}

/// Returns the JSON schema for tri-state values.
#[must_use]
fn tri_state_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["True", "False", "Unknown"],
        "description": "Tri-state evaluation result."
    })
}

/// Returns the JSON schema for safe summaries.
#[must_use]
fn safe_summary_schema() -> Value {
    json!({
        "type": "object",
        "required": ["status", "unmet_gates", "retry_hint", "policy_tags"],
        "properties": {
            "status": schema_for_string("Summary status."),
            "unmet_gates": {
                "type": "array",
                "items": schema_for_identifier("Gate identifier.")
            },
            "retry_hint": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Optional retry hint.")
                ]
            },
            "policy_tags": schema_for_string_array("Policy tags applied to the summary.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for run status values.
#[must_use]
fn run_status_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["active", "completed", "failed"]
    })
}

/// Returns the JSON schema for decision outcomes.
#[must_use]
fn decision_outcome_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind", "stage_id"],
                "properties": {
                    "kind": { "const": "start" },
                    "stage_id": schema_for_identifier("Initial stage identifier.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "stage_id"],
                "properties": {
                    "kind": { "const": "complete" },
                    "stage_id": schema_for_identifier("Terminal stage identifier.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "from_stage", "to_stage", "timeout"],
                "properties": {
                    "kind": { "const": "advance" },
                    "from_stage": schema_for_identifier("Previous stage identifier."),
                    "to_stage": schema_for_identifier("Next stage identifier."),
                    "timeout": { "type": "boolean" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "summary"],
                "properties": {
                    "kind": { "const": "hold" },
                    "summary": safe_summary_schema()
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "reason"],
                "properties": {
                    "kind": { "const": "fail" },
                    "reason": schema_for_string("Failure reason.")
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for decision records.
#[must_use]
fn decision_record_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "decision_id",
            "seq",
            "trigger_id",
            "stage_id",
            "decided_at",
            "outcome",
            "correlation_id"
        ],
        "properties": {
            "decision_id": schema_for_identifier("Decision identifier."),
            "seq": { "type": "integer", "minimum": 0 },
            "trigger_id": schema_for_identifier("Trigger identifier."),
            "stage_id": schema_for_identifier("Stage identifier."),
            "decided_at": timestamp_schema(),
            "outcome": decision_outcome_schema(),
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for packet envelopes.
#[must_use]
fn packet_envelope_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "scenario_id",
            "run_id",
            "stage_id",
            "packet_id",
            "schema_id",
            "content_type",
            "content_hash",
            "visibility",
            "expiry",
            "correlation_id",
            "issued_at"
        ],
        "properties": {
            "scenario_id": schema_for_identifier("Scenario identifier."),
            "run_id": schema_for_identifier("Run identifier."),
            "stage_id": schema_for_identifier("Stage identifier."),
            "packet_id": schema_for_identifier("Packet identifier."),
            "schema_id": schema_for_identifier("Schema identifier."),
            "content_type": schema_for_string("Packet content type."),
            "content_hash": hash_digest_schema(),
            "visibility": visibility_policy_schema(),
            "expiry": {
                "oneOf": [
                    { "type": "null" },
                    timestamp_schema()
                ]
            },
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            },
            "issued_at": timestamp_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for visibility policy.
#[must_use]
fn visibility_policy_schema() -> Value {
    json!({
        "type": "object",
        "required": ["labels", "policy_tags"],
        "properties": {
            "labels": schema_for_string_array("Visibility labels."),
            "policy_tags": schema_for_string_array("Policy tags.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for dispatch receipts.
#[must_use]
fn dispatch_receipt_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "dispatch_id",
            "target",
            "receipt_hash",
            "dispatched_at",
            "dispatcher"
        ],
        "properties": {
            "dispatch_id": schema_for_string("Dispatch identifier."),
            "target": dispatch_target_schema(),
            "receipt_hash": hash_digest_schema(),
            "dispatched_at": timestamp_schema(),
            "dispatcher": schema_for_string("Dispatcher identifier.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for packet records.
#[must_use]
fn packet_record_schema() -> Value {
    json!({
        "type": "object",
        "required": ["envelope", "payload", "receipts", "decision_id"],
        "properties": {
            "envelope": packet_envelope_schema(),
            "payload": packet_payload_schema(),
            "receipts": {
                "type": "array",
                "items": dispatch_receipt_schema()
            },
            "decision_id": schema_for_identifier("Decision identifier.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for trigger records.
#[must_use]
fn trigger_record_schema() -> Value {
    json!({
        "type": "object",
        "required": ["seq", "event"],
        "properties": {
            "seq": { "type": "integer", "minimum": 0 },
            "event": trigger_event_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for evidence values.
#[must_use]
fn evidence_value_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind", "value"],
                "properties": {
                    "kind": { "const": "json" },
                    "value": schema_for_json_value("Evidence JSON value.")
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "value"],
                "properties": {
                    "kind": { "const": "bytes" },
                    "value": {
                        "type": "array",
                        "items": { "type": "integer", "minimum": 0, "maximum": 255 }
                    }
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for evidence anchors.
#[must_use]
fn evidence_anchor_schema() -> Value {
    json!({
        "type": "object",
        "required": ["anchor_type", "anchor_value"],
        "properties": {
            "anchor_type": schema_for_string("Anchor type identifier."),
            "anchor_value": schema_for_string("Anchor value.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for evidence references.
#[must_use]
fn evidence_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["uri"],
        "properties": {
            "uri": schema_for_string("Evidence reference URI.")
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for evidence signatures.
#[must_use]
fn evidence_signature_schema() -> Value {
    json!({
        "type": "object",
        "required": ["scheme", "key_id", "signature"],
        "properties": {
            "scheme": schema_for_string("Signature scheme identifier."),
            "key_id": schema_for_string("Signing key identifier."),
            "signature": {
                "type": "array",
                "items": { "type": "integer", "minimum": 0, "maximum": 255 }
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for gate trace entries.
#[must_use]
fn gate_trace_entry_schema() -> Value {
    json!({
        "type": "object",
        "required": ["predicate", "status"],
        "properties": {
            "predicate": schema_for_identifier("Predicate identifier."),
            "status": tri_state_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for gate evaluation results.
#[must_use]
fn gate_evaluation_schema() -> Value {
    json!({
        "type": "object",
        "required": ["gate_id", "status", "trace"],
        "properties": {
            "gate_id": schema_for_identifier("Gate identifier."),
            "status": tri_state_schema(),
            "trace": {
                "type": "array",
                "items": gate_trace_entry_schema()
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for evidence records.
#[must_use]
fn evidence_record_schema() -> Value {
    json!({
        "type": "object",
        "required": ["predicate", "status", "result"],
        "properties": {
            "predicate": schema_for_identifier("Predicate identifier."),
            "status": tri_state_schema(),
            "result": evidence_result_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for gate evaluation records.
#[must_use]
fn gate_eval_record_schema() -> Value {
    json!({
        "type": "object",
        "required": ["trigger_id", "stage_id", "evaluation", "evidence"],
        "properties": {
            "trigger_id": schema_for_identifier("Trigger identifier."),
            "stage_id": schema_for_identifier("Stage identifier."),
            "evaluation": gate_evaluation_schema(),
            "evidence": {
                "type": "array",
                "items": evidence_record_schema()
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for submissions.
#[must_use]
fn submission_record_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "submission_id",
            "run_id",
            "payload",
            "content_type",
            "content_hash",
            "submitted_at",
            "correlation_id"
        ],
        "properties": {
            "submission_id": schema_for_string("Submission identifier."),
            "run_id": schema_for_identifier("Run identifier."),
            "payload": packet_payload_schema(),
            "content_type": schema_for_string("Submission content type."),
            "content_hash": hash_digest_schema(),
            "submitted_at": timestamp_schema(),
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for tool-call records.
#[must_use]
fn tool_call_record_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "call_id",
            "method",
            "request_hash",
            "response_hash",
            "called_at",
            "correlation_id",
            "error"
        ],
        "properties": {
            "call_id": schema_for_string("Tool-call identifier."),
            "method": schema_for_string("Tool method name."),
            "request_hash": hash_digest_schema(),
            "response_hash": hash_digest_schema(),
            "called_at": timestamp_schema(),
            "correlation_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_identifier("Correlation identifier.")
                ]
            },
            "error": {
                "oneOf": [
                    { "type": "null" },
                    tool_call_error_schema()
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for tool-call errors.
#[must_use]
fn tool_call_error_schema() -> Value {
    json!({
        "type": "object",
        "required": ["code", "message", "details"],
        "properties": {
            "code": schema_for_string("Stable error code."),
            "message": schema_for_string("Error message."),
            "details": {
                "oneOf": [
                    { "type": "null" },
                    tool_call_error_details_schema()
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for tool-call error details.
#[must_use]
fn tool_call_error_details_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "required": ["kind", "missing_providers", "required_capabilities", "blocked_by_policy"],
                "properties": {
                    "kind": { "const": "provider_missing" },
                    "missing_providers": schema_for_string_array("Missing provider identifiers."),
                    "required_capabilities": schema_for_string_array("Required capabilities."),
                    "blocked_by_policy": { "type": "boolean" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["kind", "info"],
                "properties": {
                    "kind": { "const": "message" },
                    "info": schema_for_string("Additional error details.")
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Returns the JSON schema for content references.
#[must_use]
fn content_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["uri", "content_hash", "encryption"],
        "properties": {
            "uri": schema_for_string("Content URI."),
            "content_hash": hash_digest_schema(),
            "encryption": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Encryption metadata.")
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for hash algorithms.
#[must_use]
fn hash_algorithm_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["sha256"]
    })
}

/// Returns the JSON schema for verifier modes.
#[must_use]
fn verifier_mode_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["offline_strict", "offline_with_fetch"]
    })
}

/// Returns the JSON schema for runpack integrity metadata.
#[must_use]
fn runpack_integrity_schema() -> Value {
    json!({
        "type": "object",
        "required": ["file_hashes", "root_hash"],
        "properties": {
            "file_hashes": {
                "type": "array",
                "items": file_hash_entry_schema()
            },
            "root_hash": hash_digest_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for file hash entries.
#[must_use]
fn file_hash_entry_schema() -> Value {
    json!({
        "type": "object",
        "required": ["path", "hash"],
        "properties": {
            "path": schema_for_string("Runpack-relative artifact path."),
            "hash": hash_digest_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for runpack artifact records.
#[must_use]
fn artifact_record_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "artifact_id",
            "kind",
            "path",
            "content_type",
            "hash",
            "required"
        ],
        "properties": {
            "artifact_id": schema_for_string("Artifact identifier."),
            "kind": artifact_kind_schema(),
            "path": schema_for_string("Runpack-relative artifact path."),
            "content_type": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_string("Artifact content type.")
                ]
            },
            "hash": hash_digest_schema(),
            "required": { "type": "boolean" }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for runpack artifact kinds.
#[must_use]
fn artifact_kind_schema() -> Value {
    json!({
        "type": "string",
        "enum": [
            "scenario_spec",
            "trigger_log",
            "gate_eval_log",
            "decision_log",
            "packet_log",
            "dispatch_log",
            "evidence_log",
            "submission_log",
            "tool_transcript",
            "verifier_report",
            "custom"
        ]
    })
}
