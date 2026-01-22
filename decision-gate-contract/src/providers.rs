// decision-gate-contract/src/providers.rs
// ============================================================================
// Module: Provider Contracts
// Description: Canonical provider capability definitions for Decision Gate.
// Purpose: Describe predicate schemas and provider configuration contracts.
// Dependencies: serde_json, decision-gate-contract::schemas
// ============================================================================

//! ## Overview
//! Provider contracts describe the available predicates, parameter schemas, and
//! output shapes for built-in providers. These contracts are intended to be
//! exported into docs and SDKs without hand-maintained duplication.
//! Security posture: provider inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde_json::Value;
use serde_json::json;

use crate::schemas;
use crate::types::PredicateContract;
use crate::types::PredicateExample;
use crate::types::ProviderContract;

// ============================================================================
// SECTION: Provider Contracts
// ============================================================================

/// Returns the canonical provider contracts for built-in providers.
#[must_use]
pub fn provider_contracts() -> Vec<ProviderContract> {
    vec![
        time_provider_contract(),
        env_provider_contract(),
        json_provider_contract(),
        http_provider_contract(),
    ]
}

/// Builds markdown documentation for provider contracts.
#[must_use]
pub fn providers_markdown(contracts: &[ProviderContract]) -> String {
    let mut out = String::new();
    out.push_str("# Decision Gate Built-in Providers\n\n");
    out.push_str("This document summarizes built-in providers. Full schemas are in ");
    out.push_str("`providers.json`.\n\n");
    for provider in contracts {
        out.push_str("## ");
        out.push_str(&provider.provider_id);
        out.push('\n');
        out.push('\n');
        out.push_str(provider.description.as_str());
        out.push('\n');
        out.push('\n');
        out.push_str("Predicates:\n");
        for predicate in &provider.predicates {
            out.push_str("- ");
            out.push_str(&predicate.name);
            out.push_str(": ");
            out.push_str(&predicate.description);
            out.push('\n');
        }
        out.push('\n');
        if !provider.notes.is_empty() {
            out.push_str("Notes:\n");
            for note in &provider.notes {
                out.push_str("- ");
                out.push_str(note);
                out.push('\n');
            }
            out.push('\n');
        }
    }
    out
}

// ============================================================================
// SECTION: Built-in Provider Definitions
// ============================================================================

/// Returns the contract for the built-in time provider.
#[must_use]
fn time_provider_contract() -> ProviderContract {
    ProviderContract {
        provider_id: String::from("time"),
        name: String::from("Time Provider"),
        description: String::from(
            "Deterministic predicates derived from the trigger timestamp supplied by the caller.",
        ),
        transport: String::from("builtin"),
        config_schema: time_config_schema(),
        predicates: vec![
            PredicateContract {
                name: String::from("now"),
                description: String::from("Return the trigger timestamp as a JSON number."),
                params_required: false,
                params_schema: empty_params_schema("No parameters required."),
                result_schema: timestamp_value_schema(),
                anchor_types: vec![
                    String::from("trigger_time_unix_millis"),
                    String::from("trigger_time_logical"),
                ],
                content_types: vec![String::from("application/json")],
                examples: vec![PredicateExample {
                    description: String::from("Return trigger time."),
                    params: json!({}),
                    result: json!(1_710_000_000_000_i64),
                }],
            },
            PredicateContract {
                name: String::from("after"),
                description: String::from("Return true if trigger time is after the threshold."),
                params_required: true,
                params_schema: time_threshold_schema(),
                result_schema: json!({ "type": "boolean" }),
                anchor_types: vec![
                    String::from("trigger_time_unix_millis"),
                    String::from("trigger_time_logical"),
                ],
                content_types: vec![String::from("application/json")],
                examples: vec![PredicateExample {
                    description: String::from("Trigger time after threshold."),
                    params: json!({ "timestamp": 1_710_000_000_000_i64 }),
                    result: json!(true),
                }],
            },
            PredicateContract {
                name: String::from("before"),
                description: String::from("Return true if trigger time is before the threshold."),
                params_required: true,
                params_schema: time_threshold_schema(),
                result_schema: json!({ "type": "boolean" }),
                anchor_types: vec![
                    String::from("trigger_time_unix_millis"),
                    String::from("trigger_time_logical"),
                ],
                content_types: vec![String::from("application/json")],
                examples: vec![PredicateExample {
                    description: String::from("Trigger time before threshold."),
                    params: json!({ "timestamp": "2024-01-01T00:00:00Z" }),
                    result: json!(false),
                }],
            },
        ],
        notes: vec![
            String::from("Deterministic: no wall-clock reads, only trigger timestamps."),
            String::from("Supports unix_millis and logical trigger timestamps."),
        ],
    }
}

/// Returns the contract for the built-in env provider.
#[must_use]
fn env_provider_contract() -> ProviderContract {
    ProviderContract {
        provider_id: String::from("env"),
        name: String::from("Environment Provider"),
        description: String::from(
            "Reads process environment variables with allow/deny policy and size limits.",
        ),
        transport: String::from("builtin"),
        config_schema: env_config_schema(),
        predicates: vec![PredicateContract {
            name: String::from("get"),
            description: String::from("Fetch an environment variable by key."),
            params_required: true,
            params_schema: json!({
                "type": "object",
                "required": ["key"],
                "properties": {
                    "key": { "type": "string" }
                },
                "additionalProperties": false
            }),
            result_schema: json!({
                "oneOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            }),
            anchor_types: vec![String::from("env")],
            content_types: vec![String::from("text/plain")],
            examples: vec![PredicateExample {
                description: String::from("Read DEPLOY_ENV."),
                params: json!({ "key": "DEPLOY_ENV" }),
                result: json!("production"),
            }],
        }],
        notes: vec![
            String::from("Returns null when a key is missing or blocked by policy."),
            String::from("Size limits apply to both key and value."),
        ],
    }
}

/// Returns the contract for the built-in json provider.
#[must_use]
fn json_provider_contract() -> ProviderContract {
    ProviderContract {
        provider_id: String::from("json"),
        name: String::from("JSON Provider"),
        description: String::from(
            "Reads JSON or YAML files and evaluates JSONPath queries against them.",
        ),
        transport: String::from("builtin"),
        config_schema: json_config_schema(),
        predicates: vec![PredicateContract {
            name: String::from("path"),
            description: String::from("Select values via JSONPath from a JSON/YAML file."),
            params_required: true,
            params_schema: json!({
                "type": "object",
                "required": ["file"],
                "properties": {
                    "file": { "type": "string" },
                    "jsonpath": { "type": "string" }
                },
                "additionalProperties": false
            }),
            result_schema: json!({
                "oneOf": [
                    { "description": "JSONPath result." },
                    { "type": "null" }
                ]
            }),
            anchor_types: vec![String::from("file_path")],
            content_types: vec![String::from("application/json"), String::from("application/yaml")],
            examples: vec![
                PredicateExample {
                    description: String::from("Read version from config.json."),
                    params: json!({ "file": "/etc/config.json", "jsonpath": "$.version" }),
                    result: json!("1.2.3"),
                },
                PredicateExample {
                    description: String::from("Return full document when jsonpath is omitted."),
                    params: json!({ "file": "/etc/config.json" }),
                    result: json!({ "version": "1.2.3" }),
                },
            ],
        }],
        notes: vec![
            String::from("File access is constrained by root policy and size limits."),
            String::from("JSONPath is optional; omitted means the full document."),
        ],
    }
}

/// Returns the contract for the built-in http provider.
#[must_use]
fn http_provider_contract() -> ProviderContract {
    ProviderContract {
        provider_id: String::from("http"),
        name: String::from("HTTP Provider"),
        description: String::from(
            "Issues bounded HTTP GET requests and returns status codes or body hashes.",
        ),
        transport: String::from("builtin"),
        config_schema: http_config_schema(),
        predicates: vec![
            PredicateContract {
                name: String::from("status"),
                description: String::from("Return HTTP status code for a URL."),
                params_required: true,
                params_schema: http_url_schema(),
                result_schema: json!({ "type": "integer" }),
                anchor_types: vec![String::from("url")],
                content_types: vec![String::from("application/json")],
                examples: vec![PredicateExample {
                    description: String::from("Fetch status for a health endpoint."),
                    params: json!({ "url": "https://api.example.com/health" }),
                    result: json!(200),
                }],
            },
            PredicateContract {
                name: String::from("body_hash"),
                description: String::from("Return a hash of the response body."),
                params_required: true,
                params_schema: http_url_schema(),
                result_schema: schemas::hash_digest_schema(),
                anchor_types: vec![String::from("url")],
                content_types: vec![String::from("application/json")],
                examples: vec![PredicateExample {
                    description: String::from("Hash the body of a health endpoint."),
                    params: json!({ "url": "https://api.example.com/health" }),
                    result: json!({
                        "algorithm": "sha256",
                        "value": "7b4d0d3d16c8f85f67ad79b0870a2c9f1e88924c4cbb4ed4bb7f5c6a1d1b7f9a"
                    }),
                }],
            },
        ],
        notes: vec![
            String::from("Scheme and host allowlists are enforced by configuration."),
            String::from("Responses are size-limited and hashed deterministically."),
        ],
    }
}

// ============================================================================
// SECTION: Provider Schema Helpers
// ============================================================================

/// Returns a schema for the time provider config.
#[must_use]
fn time_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "allow_logical": { "type": "boolean" }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for the env provider config.
#[must_use]
fn env_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "allowlist": {
                "type": "array",
                "items": { "type": "string" }
            },
            "denylist": {
                "type": "array",
                "items": { "type": "string" }
            },
            "max_value_bytes": { "type": "integer", "minimum": 0 },
            "max_key_bytes": { "type": "integer", "minimum": 0 },
            "overrides": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for the json provider config.
#[must_use]
fn json_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "root": { "type": "string" },
            "max_bytes": { "type": "integer", "minimum": 0 },
            "allow_yaml": { "type": "boolean" }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for the http provider config.
#[must_use]
fn http_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "allow_http": { "type": "boolean" },
            "timeout_ms": { "type": "integer", "minimum": 0 },
            "max_response_bytes": { "type": "integer", "minimum": 0 },
            "allowed_hosts": {
                "type": "array",
                "items": { "type": "string" }
            },
            "user_agent": { "type": "string" },
            "hash_algorithm": { "type": "string", "enum": ["sha256"] }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for time threshold parameters.
#[must_use]
fn time_threshold_schema() -> Value {
    json!({
        "type": "object",
        "required": ["timestamp"],
        "properties": {
            "timestamp": {
                "oneOf": [
                    { "type": "integer" },
                    { "type": "string" }
                ]
            }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for HTTP URL parameters.
#[must_use]
fn http_url_schema() -> Value {
    json!({
        "type": "object",
        "required": ["url"],
        "properties": {
            "url": { "type": "string" }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for predicates with no params.
#[must_use]
fn empty_params_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {},
        "additionalProperties": false
    })
}

/// Returns a schema for time predicate results.
#[must_use]
fn timestamp_value_schema() -> Value {
    json!({
        "type": "integer"
    })
}
