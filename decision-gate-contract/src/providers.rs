// decision-gate-contract/src/providers.rs
// ============================================================================
// Module: Provider Contracts
// Description: Canonical provider capability definitions for Decision Gate.
// Purpose: Describe check schemas and provider configuration contracts.
// Dependencies: serde_json, decision-gate-contract::schemas
// ============================================================================

//! ## Overview
//! Provider contracts describe the available checks, parameter schemas, and
//! output shapes for built-in providers. These contracts are intended to be
//! exported into docs and SDKs without hand-maintained duplication.
//! Security posture: provider inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fmt::Write;

use decision_gate_core::Comparator;
use serde_json::Value;
use serde_json::json;

use crate::schemas;
use crate::types::CheckContract;
use crate::types::CheckExample;
use crate::types::DeterminismClass;
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
        out.push_str("**Provider contract**\n\n");
        out.push_str("- Name: ");
        out.push_str(&provider.name);
        out.push('\n');
        out.push_str("- Transport: ");
        out.push_str(&provider.transport);
        out.push('\n');
        out.push('\n');
        if !provider.notes.is_empty() {
            out.push_str("**Notes**\n\n");
            for note in &provider.notes {
                out.push_str("- ");
                out.push_str(note);
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str("### Configuration schema\n\n");
        out.push_str("Config fields:\n\n");
        for line in render_schema_fields(&provider.config_schema) {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
        render_json_block(&mut out, &provider.config_schema);
        out.push('\n');
        out.push_str("### Checks\n\n");
        for check in &provider.checks {
            out.push_str("#### ");
            out.push_str(&check.check_id);
            out.push('\n');
            out.push('\n');
            out.push_str(&check.description);
            out.push('\n');
            out.push('\n');
            out.push_str("- Determinism: ");
            out.push_str(check.determinism.as_str());
            out.push('\n');
            out.push_str("- Params required: ");
            out.push_str(if check.params_required { "yes" } else { "no" });
            out.push('\n');
            out.push_str("- Allowed comparators: ");
            out.push_str(&render_comparator_list(&check.allowed_comparators));
            out.push('\n');
            if !check.anchor_types.is_empty() {
                out.push_str("- Anchor types: ");
                out.push_str(&check.anchor_types.join(", "));
                out.push('\n');
            }
            if !check.content_types.is_empty() {
                out.push_str("- Content types: ");
                out.push_str(&check.content_types.join(", "));
                out.push('\n');
            }
            out.push('\n');
            out.push_str("Params fields:\n\n");
            for line in render_schema_fields(&check.params_schema) {
                out.push_str(&line);
                out.push('\n');
            }
            out.push('\n');
            out.push_str("Params schema:\n");
            render_json_block(&mut out, &check.params_schema);
            out.push_str("Result schema:\n");
            render_json_block(&mut out, &check.result_schema);
            if !check.examples.is_empty() {
                out.push_str("Examples:\n\n");
                render_check_examples(&mut out, &check.examples);
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
    let now_schema = timestamp_value_schema();
    let now_allowed = allowed_comparators_for_schema(&now_schema);
    let threshold_schema = time_threshold_schema();
    let bool_schema = json!({ "type": "boolean" });
    let bool_allowed = allowed_comparators_for_schema(&bool_schema);
    ProviderContract {
        provider_id: String::from("time"),
        name: String::from("Time Provider"),
        description: String::from(
            "Deterministic checks derived from the trigger timestamp supplied by the caller.",
        ),
        transport: String::from("builtin"),
        config_schema: time_config_schema(),
        checks: vec![
            CheckContract {
                check_id: String::from("now"),
                description: String::from("Return the trigger timestamp as a JSON number."),
                determinism: DeterminismClass::TimeDependent,
                params_required: false,
                params_schema: empty_params_schema("No parameters required."),
                result_schema: now_schema,
                allowed_comparators: now_allowed,
                anchor_types: vec![
                    String::from("trigger_time_unix_millis"),
                    String::from("trigger_time_logical"),
                ],
                content_types: vec![String::from("application/json")],
                examples: vec![CheckExample {
                    description: String::from("Return trigger time."),
                    params: json!({}),
                    result: json!(1_710_000_000_000_i64),
                }],
            },
            CheckContract {
                check_id: String::from("after"),
                description: String::from("Return true if trigger time is after the threshold."),
                determinism: DeterminismClass::TimeDependent,
                params_required: true,
                params_schema: threshold_schema.clone(),
                result_schema: bool_schema.clone(),
                allowed_comparators: bool_allowed.clone(),
                anchor_types: vec![
                    String::from("trigger_time_unix_millis"),
                    String::from("trigger_time_logical"),
                ],
                content_types: vec![String::from("application/json")],
                examples: vec![CheckExample {
                    description: String::from("Trigger time after threshold."),
                    params: json!({ "timestamp": 1_710_000_000_000_i64 }),
                    result: json!(true),
                }],
            },
            CheckContract {
                check_id: String::from("before"),
                description: String::from("Return true if trigger time is before the threshold."),
                determinism: DeterminismClass::TimeDependent,
                params_required: true,
                params_schema: threshold_schema,
                result_schema: bool_schema,
                allowed_comparators: bool_allowed,
                anchor_types: vec![
                    String::from("trigger_time_unix_millis"),
                    String::from("trigger_time_logical"),
                ],
                content_types: vec![String::from("application/json")],
                examples: vec![CheckExample {
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
    let result_schema = json!({
        "oneOf": [
            { "type": "string" },
            { "type": "null" }
        ]
    });
    let allowed_comparators = allowed_comparators_for_schema(&result_schema);
    ProviderContract {
        provider_id: String::from("env"),
        name: String::from("Environment Provider"),
        description: String::from(
            "Reads process environment variables with allow/deny policy and size limits.",
        ),
        transport: String::from("builtin"),
        config_schema: env_config_schema(),
        checks: vec![CheckContract {
            check_id: String::from("get"),
            description: String::from("Fetch an environment variable by key."),
            determinism: DeterminismClass::External,
            params_required: true,
            params_schema: json!({
                "type": "object",
                "required": ["key"],
                "properties": {
                    "key": { "type": "string", "description": "Environment variable key." }
                },
                "additionalProperties": false
            }),
            result_schema,
            allowed_comparators,
            anchor_types: vec![String::from("env")],
            content_types: vec![String::from("text/plain")],
            examples: vec![CheckExample {
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
    let result_schema = json!({
        "description": "JSONPath result value (dynamic JSON type).",
        "x-decision-gate": {
            "dynamic_type": true
        }
    });
    let allowed_comparators = canonicalize_comparators(vec![
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
        Comparator::LexGreaterThan,
        Comparator::LexGreaterThanOrEqual,
        Comparator::LexLessThan,
        Comparator::LexLessThanOrEqual,
        Comparator::Contains,
        Comparator::InSet,
        Comparator::DeepEquals,
        Comparator::DeepNotEquals,
        Comparator::Exists,
        Comparator::NotExists,
    ]);
    ProviderContract {
        provider_id: String::from("json"),
        name: String::from("JSON Provider"),
        description: String::from(
            "Reads JSON or YAML files and evaluates JSONPath queries against them.",
        ),
        transport: String::from("builtin"),
        config_schema: json_config_schema(),
        checks: vec![CheckContract {
            check_id: String::from("path"),
            description: String::from("Select values via JSONPath from a JSON/YAML file."),
            determinism: DeterminismClass::External,
            params_required: true,
            params_schema: json!({
                "type": "object",
                "required": ["file"],
                "properties": {
                    "file": { "type": "string", "description": "Path to a JSON or YAML file." },
                    "jsonpath": { "type": "string", "description": "Optional JSONPath selector." }
                },
                "additionalProperties": false
            }),
            result_schema,
            allowed_comparators,
            anchor_types: vec![String::from("file_path")],
            content_types: vec![String::from("application/json"), String::from("application/yaml")],
            examples: vec![
                CheckExample {
                    description: String::from("Read version from config.json."),
                    params: json!({ "file": "/etc/config.json", "jsonpath": "$.version" }),
                    result: json!("1.2.3"),
                },
                CheckExample {
                    description: String::from("Return full document when jsonpath is omitted."),
                    params: json!({ "file": "/etc/config.json" }),
                    result: json!({ "version": "1.2.3" }),
                },
            ],
        }],
        notes: vec![
            String::from("File access is constrained by root policy and size limits."),
            String::from("JSONPath is optional; omitted means the full document."),
            String::from(
                "Missing JSONPath yields a null value with error metadata (jsonpath_not_found).",
            ),
        ],
    }
}

/// Returns the contract for the built-in http provider.
#[must_use]
fn http_provider_contract() -> ProviderContract {
    let status_schema = json!({ "type": "integer" });
    let status_allowed = allowed_comparators_for_schema(&status_schema);
    let hash_schema = schemas::hash_digest_schema();
    let hash_allowed = allowed_comparators_for_schema(&hash_schema);
    ProviderContract {
        provider_id: String::from("http"),
        name: String::from("HTTP Provider"),
        description: String::from(
            "Issues bounded HTTP GET requests and returns status codes or body hashes.",
        ),
        transport: String::from("builtin"),
        config_schema: http_config_schema(),
        checks: vec![
            CheckContract {
                check_id: String::from("status"),
                description: String::from("Return HTTP status code for a URL."),
                determinism: DeterminismClass::External,
                params_required: true,
                params_schema: http_url_schema(),
                result_schema: status_schema,
                allowed_comparators: status_allowed,
                anchor_types: vec![String::from("url")],
                content_types: vec![String::from("application/json")],
                examples: vec![CheckExample {
                    description: String::from("Fetch status for a health endpoint."),
                    params: json!({ "url": "https://api.example.com/health" }),
                    result: json!(200),
                }],
            },
            CheckContract {
                check_id: String::from("body_hash"),
                description: String::from("Return a hash of the response body."),
                determinism: DeterminismClass::External,
                params_required: true,
                params_schema: http_url_schema(),
                result_schema: hash_schema,
                allowed_comparators: hash_allowed,
                anchor_types: vec![String::from("url")],
                content_types: vec![String::from("application/json")],
                examples: vec![CheckExample {
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
// SECTION: Comparator Defaults
// ============================================================================

/// Returns the comparator allow-list for a check result schema.
#[must_use]
fn allowed_comparators_for_schema(schema: &Value) -> Vec<Comparator> {
    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        return intersect_comparators(options);
    }
    if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
        return intersect_comparators(options);
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array)
        && !values.is_empty()
    {
        return canonicalize_comparators(vec![
            Comparator::Equals,
            Comparator::NotEquals,
            Comparator::InSet,
            Comparator::Exists,
            Comparator::NotExists,
        ]);
    }

    let mut allowed = Vec::new();
    if let Some(schema_type) = schema.get("type") {
        match schema_type {
            Value::String(kind) => {
                merge_comparators(&mut allowed, comparators_for_type(kind));
            }
            Value::Array(kinds) => {
                for kind in kinds {
                    if let Some(kind) = kind.as_str() {
                        merge_comparators(&mut allowed, comparators_for_type(kind));
                    }
                }
            }
            _ => {}
        }
    }

    if allowed.is_empty() {
        merge_comparators(&mut allowed, default_comparators());
    }

    canonicalize_comparators(allowed)
}

/// Returns comparators for a JSON schema type.
#[must_use]
fn comparators_for_type(kind: &str) -> Vec<Comparator> {
    match kind {
        "integer" | "number" => vec![
            Comparator::Equals,
            Comparator::NotEquals,
            Comparator::GreaterThan,
            Comparator::GreaterThanOrEqual,
            Comparator::LessThan,
            Comparator::LessThanOrEqual,
            Comparator::InSet,
            Comparator::Exists,
            Comparator::NotExists,
        ],
        "string" => vec![
            Comparator::Equals,
            Comparator::NotEquals,
            Comparator::Contains,
            Comparator::InSet,
            Comparator::Exists,
            Comparator::NotExists,
        ],
        "array" => vec![Comparator::Contains, Comparator::Exists, Comparator::NotExists],
        "boolean" => vec![
            Comparator::Equals,
            Comparator::NotEquals,
            Comparator::InSet,
            Comparator::Exists,
            Comparator::NotExists,
        ],
        "object" | "null" => vec![Comparator::Exists, Comparator::NotExists],
        _ => default_comparators(),
    }
}

/// Returns the default comparator allow-list for untyped schemas.
#[must_use]
fn default_comparators() -> Vec<Comparator> {
    vec![Comparator::Equals, Comparator::NotEquals, Comparator::Exists, Comparator::NotExists]
}

/// Ensures comparator lists are unique and in canonical order.
#[must_use]
fn canonicalize_comparators(input: Vec<Comparator>) -> Vec<Comparator> {
    let mut unique = Vec::new();
    for comparator in input {
        if !unique.contains(&comparator) {
            unique.push(comparator);
        }
    }
    comparator_order().iter().filter(|candidate| unique.contains(candidate)).copied().collect()
}

/// Returns the canonical comparator ordering.
#[must_use]
const fn comparator_order() -> [Comparator; 16] {
    [
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
        Comparator::LexGreaterThan,
        Comparator::LexGreaterThanOrEqual,
        Comparator::LexLessThan,
        Comparator::LexLessThanOrEqual,
        Comparator::Contains,
        Comparator::InSet,
        Comparator::DeepEquals,
        Comparator::DeepNotEquals,
        Comparator::Exists,
        Comparator::NotExists,
    ]
}

/// Merges comparator lists without duplicates.
fn merge_comparators(target: &mut Vec<Comparator>, source: Vec<Comparator>) {
    for comparator in source {
        if !target.contains(&comparator) {
            target.push(comparator);
        }
    }
}

/// Renders comparator names in a stable order.
#[must_use]
fn render_comparator_list(comparators: &[Comparator]) -> String {
    let items: Vec<&str> =
        comparators.iter().map(|comparator| comparator_label(*comparator)).collect();
    items.join(", ")
}

/// Returns the comparator label used in docs.
#[must_use]
const fn comparator_label(comparator: Comparator) -> &'static str {
    match comparator {
        Comparator::Equals => "equals",
        Comparator::NotEquals => "not_equals",
        Comparator::GreaterThan => "greater_than",
        Comparator::GreaterThanOrEqual => "greater_than_or_equal",
        Comparator::LessThan => "less_than",
        Comparator::LessThanOrEqual => "less_than_or_equal",
        Comparator::LexGreaterThan => "lex_greater_than",
        Comparator::LexGreaterThanOrEqual => "lex_greater_than_or_equal",
        Comparator::LexLessThan => "lex_less_than",
        Comparator::LexLessThanOrEqual => "lex_less_than_or_equal",
        Comparator::Contains => "contains",
        Comparator::InSet => "in_set",
        Comparator::DeepEquals => "deep_equals",
        Comparator::DeepNotEquals => "deep_not_equals",
        Comparator::Exists => "exists",
        Comparator::NotExists => "not_exists",
    }
}

/// Intersects comparator lists across schema variants, ignoring null-only options.
fn intersect_comparators(options: &[Value]) -> Vec<Comparator> {
    let (non_null, null_only) = partition_null_variants(options);
    let candidates = if non_null.is_empty() { null_only } else { non_null };
    let mut iter = candidates.into_iter();
    let Some(first) = iter.next() else {
        return default_comparators();
    };
    let mut allowed = allowed_comparators_for_schema(first);
    for option in iter {
        let option_allowed = allowed_comparators_for_schema(option);
        allowed.retain(|comparator| option_allowed.contains(comparator));
    }
    canonicalize_comparators(allowed)
}

/// Splits schema variants into null-only and non-null sets.
fn partition_null_variants(options: &[Value]) -> (Vec<&Value>, Vec<&Value>) {
    let mut non_null = Vec::new();
    let mut null_only = Vec::new();
    for option in options {
        if is_null_schema(option) {
            null_only.push(option);
        } else {
            non_null.push(option);
        }
    }
    (non_null, null_only)
}

/// Returns true if a schema represents only null values.
fn is_null_schema(schema: &Value) -> bool {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return !values.is_empty() && values.iter().all(Value::is_null);
    }
    if let Some(schema_type) = schema.get("type") {
        match schema_type {
            Value::String(kind) => return kind == "null",
            Value::Array(kinds) => {
                let mut has_null = false;
                let mut has_other = false;
                for kind in kinds {
                    if let Some(kind) = kind.as_str() {
                        if kind == "null" {
                            has_null = true;
                        } else {
                            has_other = true;
                        }
                    }
                }
                return has_null && !has_other;
            }
            _ => {}
        }
    }
    false
}

/// Render a JSON value in a fenced markdown code block.
fn render_json_block(out: &mut String, value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| String::from("{}"));
    out.push_str("```json\n");
    out.push_str(&rendered);
    out.push_str("\n```\n");
}

/// Render check examples with params and result payloads.
fn render_check_examples(out: &mut String, examples: &[CheckExample]) {
    for (idx, example) in examples.iter().enumerate() {
        if examples.len() > 1 {
            out.push_str("Example ");
            out.push_str(&(idx + 1).to_string());
            out.push_str(": ");
        }
        out.push_str(&example.description);
        out.push('\n');
        out.push('\n');
        out.push_str("Params:\n");
        render_json_block(out, &example.params);
        out.push_str("Result:\n");
        render_json_block(out, &example.result);
    }
}

/// Renders schema fields as markdown list entries.
fn render_schema_fields(schema: &Value) -> Vec<String> {
    let props = schema.get("properties").and_then(Value::as_object);
    let Some(props) = props else {
        return vec![String::from("_No fields._")];
    };
    if props.is_empty() {
        return vec![String::from("_No fields._")];
    }
    let required: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();

    let mut keys: Vec<&String> = props.keys().collect();
    keys.sort();
    keys.into_iter()
        .map(|key| {
            let entry = &props[key];
            let required_label = if required.contains(key) { "required" } else { "optional" };
            let description = schema_description(entry);
            format!("- `{key}` ({required_label}): {description}")
        })
        .collect()
}

/// Builds a short description for a JSON schema fragment.
fn schema_description(schema: &Value) -> String {
    let mut description = schema.get("description").and_then(Value::as_str).map_or_else(
        || {
            if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
                return format!("Schema reference {reference}.");
            }
            if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
                let labels: Vec<&str> = enum_values.iter().filter_map(Value::as_str).collect();
                return if labels.is_empty() {
                    String::from("See schema for details.")
                } else {
                    format!("Enum: {}.", labels.join(", "))
                };
            }
            if let Some(schema_type) = schema.get("type") {
                return match schema_type {
                    Value::String(kind) => format!("Type: {kind}."),
                    Value::Array(kinds) => {
                        let labels: Vec<&str> = kinds.iter().filter_map(Value::as_str).collect();
                        if labels.is_empty() {
                            String::from("See schema for details.")
                        } else {
                            format!("Type: {}.", labels.join("|"))
                        }
                    }
                    _ => String::from("See schema for details."),
                };
            }
            if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
                return format!("One of {} schema variants.", options.len());
            }
            if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
                return format!("Any of {} schema variants.", options.len());
            }
            if let Some(options) = schema.get("allOf").and_then(Value::as_array) {
                return format!("All of {} schema variants.", options.len());
            }
            String::from("See schema for details.")
        },
        str::to_string,
    );

    if let Some(default_value) = schema.get("default") {
        let default_text =
            serde_json::to_string(default_value).unwrap_or_else(|_| String::from("null"));
        if !description.ends_with('.') {
            description.push('.');
        }
        description.push(' ');
        let _ = write!(description, "Default: {default_text}.");
    }

    description
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
            "allow_logical": {
                "type": "boolean",
                "description": "Allow logical trigger timestamps in comparisons.",
                "default": true
            }
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
                "items": { "type": "string" },
                "description": "Optional allowlist of environment keys."
            },
            "denylist": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Explicit denylist of environment keys.",
                "default": []
            },
            "max_value_bytes": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum bytes allowed for an environment value.",
                "default": 65536
            },
            "max_key_bytes": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum bytes allowed for an environment key.",
                "default": 255
            },
            "overrides": {
                "type": "object",
                "additionalProperties": { "type": "string" },
                "description": "Optional deterministic override map for env lookups."
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
            "root": { "type": "string", "description": "Optional root directory for file resolution." },
            "max_bytes": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum file size in bytes.",
                "default": 1_048_576
            },
            "allow_yaml": {
                "type": "boolean",
                "description": "Allow YAML parsing for .yaml/.yml files.",
                "default": true
            }
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
            "allow_http": {
                "type": "boolean",
                "description": "Allow cleartext http:// URLs.",
                "default": false
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 0,
                "description": "Request timeout in milliseconds.",
                "default": 5000
            },
            "max_response_bytes": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum response size in bytes.",
                "default": 1_048_576
            },
            "allowed_hosts": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Optional allowlist of hostnames."
            },
            "user_agent": {
                "type": "string",
                "description": "User agent string for outbound requests.",
                "default": "decision-gate/0.1"
            },
            "hash_algorithm": {
                "type": "string",
                "enum": ["sha256"],
                "description": "Hash algorithm used for body_hash responses.",
                "default": "sha256"
            }
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
                ],
                "description": "Unix millis number or RFC3339 timestamp string."
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
            "url": { "type": "string", "description": "URL to query." }
        },
        "additionalProperties": false
    })
}

/// Returns a schema for checks with no params.
#[must_use]
fn empty_params_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {},
        "additionalProperties": false
    })
}

/// Returns a schema for time check results.
#[must_use]
fn timestamp_value_schema() -> Value {
    json!({
        "type": "integer"
    })
}
