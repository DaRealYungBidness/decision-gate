// crates/decision-gate-contract/src/tooling.rs
// ============================================================================
// Module: MCP Tool Contracts
// Description: Canonical MCP tool definitions and schemas for Decision Gate.
// Purpose: Provide tool contracts for docs, SDK generation, and MCP listing.
// Dependencies: serde_json, std, decision-gate-contract::examples, decision-gate-contract::schemas,
// decision-gate-contract::types
// ============================================================================

//! ## Overview
//! This module defines the canonical MCP tool surface. Tool contracts are used
//! both to drive MCP tool listings and to generate docs/SDKs with strict,
//! deterministic schemas.
//! Security posture: tool inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeSet;

use serde_json::Value;
use serde_json::json;

use crate::examples;
use crate::schemas;
use crate::types::ToolContract;
// ============================================================================
// SECTION: Re-Exports
// ============================================================================
/// Tool definition shape used by MCP tool listings.
pub use crate::types::ToolDefinition;
use crate::types::ToolExample;
use crate::types::ToolName;

// ============================================================================
// SECTION: Tool Contracts
// ============================================================================

/// Returns the canonical MCP tool contracts.
///
/// The order is intentional: it is preserved in generated docs/SDKs to keep
/// diffs stable across releases. Append new tools at the end.
#[must_use]
pub fn tool_contracts() -> Vec<ToolContract> {
    vec![
        scenario_define_contract(),
        scenario_start_contract(),
        scenario_status_contract(),
        scenario_next_contract(),
        scenario_submit_contract(),
        scenario_trigger_contract(),
        evidence_query_contract(),
        runpack_export_contract(),
        runpack_verify_contract(),
        providers_list_contract(),
        provider_contract_get_contract(),
        provider_check_schema_get_contract(),
        schemas_register_contract(),
        schemas_list_contract(),
        schemas_get_contract(),
        scenarios_list_contract(),
        precheck_contract(),
        decision_gate_docs_search_contract(),
    ]
}

/// Builds the tool contract for `scenario_define`.
fn scenario_define_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenarioDefine,
        "Register a ScenarioSpec, validate it, and return the canonical hash used for integrity \
         checks.",
        scenario_define_input_schema(),
        scenario_define_output_schema(),
        tool_examples(ToolName::ScenarioDefine),
        vec![
            "Use before starting runs; scenario_id becomes the stable handle for later calls."
                .to_string(),
            "Validates stage/gate/condition IDs, RET trees, and condition references.".to_string(),
            "Spec hash is deterministic; store it for audit and runpack integrity.".to_string(),
            "Fails closed on invalid specs or duplicate scenario IDs.".to_string(),
        ],
    )
}

/// Builds the tool contract for `scenario_start`.
fn scenario_start_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenarioStart,
        "Create a new run state for a scenario and optionally emit entry packets.",
        scenario_start_input_schema(),
        schemas::run_state_schema(),
        tool_examples(ToolName::ScenarioStart),
        vec![
            "Requires RunConfig (tenant_id, run_id, scenario_id, dispatch_targets).".to_string(),
            "Use started_at to record the caller-supplied start timestamp.".to_string(),
            "If issue_entry_packets is true, entry packets are disclosed immediately.".to_string(),
            "Fails closed if run_id already exists or scenario_id is unknown.".to_string(),
        ],
    )
}

/// Builds the tool contract for `scenario_status`.
fn scenario_status_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenarioStatus,
        "Fetch a read-only run snapshot and safe summary without changing state.",
        scenario_status_input_schema(),
        schemas::scenario_status_schema(),
        tool_examples(ToolName::ScenarioStatus),
        vec![
            "Use for polling or UI state; does not evaluate gates.".to_string(),
            "Safe summaries omit evidence values and may include retry hints.".to_string(),
            "Returns issued packet IDs to help track disclosures.".to_string(),
        ],
    )
}

/// Builds the tool contract for `scenario_next`.
fn scenario_next_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenarioNext,
        "Evaluate gates in response to an agent-driven next request.",
        scenario_next_input_schema(),
        schemas::scenario_next_result_schema(),
        tool_examples(ToolName::ScenarioNext),
        vec![
            "Idempotent by trigger_id; repeated calls return the same decision.".to_string(),
            "Records decision, evidence, and packet disclosures in run state.".to_string(),
            "Requires an active run; completed or failed runs do not advance.".to_string(),
            "Optional feedback can include gate trace or evidence when permitted by server \
             feedback policy."
                .to_string(),
        ],
    )
}

/// Builds the tool contract for `scenario_submit`.
fn scenario_submit_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenarioSubmit,
        "Submit external artifacts into run state for audit and later evaluation.",
        scenario_submit_input_schema(),
        schemas::submit_result_schema(),
        tool_examples(ToolName::ScenarioSubmit),
        vec![
            "Payload is hashed and stored as a submission record.".to_string(),
            "Does not advance the run by itself.".to_string(),
            "Use for artifacts the model or operator supplies.".to_string(),
        ],
    )
}

/// Builds the tool contract for `scenario_trigger`.
fn scenario_trigger_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenarioTrigger,
        "Submit a trigger event (scheduler/external) and evaluate the run.",
        scenario_trigger_input_schema(),
        schemas::trigger_result_schema(),
        tool_examples(ToolName::ScenarioTrigger),
        vec![
            "Trigger time is supplied by the caller; no wall-clock reads.".to_string(),
            "Records the trigger event and resulting decision.".to_string(),
            "Use for time-based or external system triggers.".to_string(),
        ],
    )
}

/// Builds the tool contract for `evidence_query`.
fn evidence_query_contract() -> ToolContract {
    build_tool_contract(
        ToolName::EvidenceQuery,
        "Query an evidence provider with full run context and disclosure policy.",
        evidence_query_input_schema(),
        evidence_query_output_schema(),
        tool_examples(ToolName::EvidenceQuery),
        vec![
            "Disclosure policy may redact raw values; hashes/anchors still returned.".to_string(),
            "Use for diagnostics or preflight checks; runtime uses the same provider logic."
                .to_string(),
            "Requires provider_id, check_id, and full EvidenceContext.".to_string(),
        ],
    )
}

/// Builds the tool contract for `runpack_export`.
fn runpack_export_contract() -> ToolContract {
    build_tool_contract(
        ToolName::RunpackExport,
        "Export deterministic runpack artifacts for offline verification.",
        runpack_export_input_schema(),
        runpack_export_output_schema(),
        tool_examples(ToolName::RunpackExport),
        vec![
            "Writes manifest and logs to output_dir; generated_at is recorded in the manifest."
                .to_string(),
            "include_verification adds a verification report artifact.".to_string(),
            "Use after runs complete or for audit snapshots.".to_string(),
        ],
    )
}

/// Builds the tool contract for `runpack_verify`.
fn runpack_verify_contract() -> ToolContract {
    build_tool_contract(
        ToolName::RunpackVerify,
        "Verify a runpack manifest and artifacts offline.",
        runpack_verify_input_schema(),
        runpack_verify_output_schema(),
        tool_examples(ToolName::RunpackVerify),
        vec![
            "Validates hashes, integrity root, and decision log structure.".to_string(),
            "Fails closed on missing or tampered files.".to_string(),
            "Use in CI or offline audit pipelines.".to_string(),
        ],
    )
}

/// Builds the tool contract for `providers_list`.
fn providers_list_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ProvidersList,
        "List registered evidence providers and capabilities summary.",
        providers_list_input_schema(),
        providers_list_output_schema(),
        tool_examples(ToolName::ProvidersList),
        vec![
            "Returns provider identifiers and transport metadata.".to_string(),
            "Results are scoped by auth policy.".to_string(),
        ],
    )
}

/// Builds the tool contract for `provider_contract_get`.
fn provider_contract_get_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ProviderContractGet,
        "Fetch the canonical provider contract JSON and hash for a provider.",
        provider_contract_get_input_schema(),
        provider_contract_get_output_schema(),
        tool_examples(ToolName::ProviderContractGet),
        vec![
            "Returns the provider contract as loaded by the MCP server.".to_string(),
            "Includes a canonical hash for audit and reproducibility.".to_string(),
            "Subject to provider disclosure policy and authz.".to_string(),
        ],
    )
}

/// Builds the tool contract for `provider_check_schema_get`.
fn provider_check_schema_get_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ProviderCheckSchemaGet,
        "Fetch check schema details (params/result/comparators) for a provider.",
        provider_check_schema_get_input_schema(),
        provider_check_schema_get_output_schema(),
        tool_examples(ToolName::ProviderCheckSchemaGet),
        vec![
            "Returns compiled schema metadata for a single check.".to_string(),
            "Includes comparator allow-lists and check examples.".to_string(),
            "Subject to provider disclosure policy and authz.".to_string(),
        ],
    )
}

/// Builds the tool contract for `schemas_register`.
fn schemas_register_contract() -> ToolContract {
    build_tool_contract(
        ToolName::SchemasRegister,
        "Register a data shape schema for a tenant and namespace.",
        schemas_register_input_schema(),
        schemas_register_output_schema(),
        tool_examples(ToolName::SchemasRegister),
        vec![
            "Schemas are immutable; registering the same version twice fails.".to_string(),
            "Provide created_at to record when the schema was authored.".to_string(),
        ],
    )
}

/// Builds the tool contract for `schemas_list`.
fn schemas_list_contract() -> ToolContract {
    build_tool_contract(
        ToolName::SchemasList,
        "List registered data shapes for a tenant and namespace.",
        schemas_list_input_schema(),
        schemas_list_output_schema(),
        tool_examples(ToolName::SchemasList),
        vec![
            "Requires tenant_id and namespace_id.".to_string(),
            "Supports pagination via cursor + limit.".to_string(),
        ],
    )
}

/// Builds the tool contract for `schemas_get`.
fn schemas_get_contract() -> ToolContract {
    build_tool_contract(
        ToolName::SchemasGet,
        "Fetch a specific data shape by identifier and version.",
        schemas_get_input_schema(),
        schemas_get_output_schema(),
        tool_examples(ToolName::SchemasGet),
        vec![
            "Requires tenant_id, namespace_id, schema_id, and version.".to_string(),
            "Fails closed when schema is missing.".to_string(),
        ],
    )
}

/// Builds the tool contract for `scenarios_list`.
fn scenarios_list_contract() -> ToolContract {
    build_tool_contract(
        ToolName::ScenariosList,
        "List registered scenarios for a tenant and namespace.",
        scenarios_list_input_schema(),
        scenarios_list_output_schema(),
        tool_examples(ToolName::ScenariosList),
        vec![
            "Requires tenant_id and namespace_id.".to_string(),
            "Returns scenario identifiers and hashes.".to_string(),
        ],
    )
}

/// Builds the tool contract for `precheck`.
fn precheck_contract() -> ToolContract {
    build_tool_contract(
        ToolName::Precheck,
        "Evaluate a scenario against asserted data without mutating state.",
        precheck_input_schema(),
        precheck_output_schema(),
        tool_examples(ToolName::Precheck),
        vec![
            "Validates asserted data against a registered shape.".to_string(),
            "Does not mutate run state; intended for simulation.".to_string(),
        ],
    )
}

/// Builds the tool contract for `decision_gate_docs_search`.
fn decision_gate_docs_search_contract() -> ToolContract {
    build_tool_contract(
        ToolName::DecisionGateDocsSearch,
        "Search Decision Gate documentation for runtime guidance.",
        decision_gate_docs_search_input_schema(),
        decision_gate_docs_search_output_schema(),
        tool_examples(ToolName::DecisionGateDocsSearch),
        vec![
            "Use for quick lookups on evidence flow, comparators, and provider semantics."
                .to_string(),
            "Returns ranked sections with role tags and suggested follow-ups.".to_string(),
            "Search is deterministic and scoped to the configured doc catalog.".to_string(),
        ],
    )
}

/// Returns the MCP tool definitions for tool listing.
#[must_use]
pub fn tool_definitions() -> Vec<ToolDefinition> {
    let contracts = tool_contracts();
    let mut definitions = Vec::with_capacity(contracts.len());
    for contract in contracts {
        definitions.push(ToolDefinition {
            name: contract.name,
            description: contract.description,
            input_schema: contract.input_schema,
        });
    }
    definitions
}

/// Builds markdown documentation for the tool contracts.
#[must_use]
pub fn tooling_markdown(contracts: &[ToolContract]) -> String {
    let mut out = String::new();
    out.push_str("# Decision Gate MCP Tools\n\n");
    out.push_str("This document summarizes the MCP tool surface and expected usage. ");
    out.push_str("Full schemas are in `tooling.json`, with supporting schemas under ");
    out.push_str("`schemas/` and examples under `examples/`.\n\n");
    out.push_str("## Lifecycle quickstart\n\n");
    out.push_str("- `scenario_define` registers and validates a ScenarioSpec.\n");
    out.push_str("- `scenario_start` creates a run and optionally issues entry packets.\n");
    out.push_str("- `scenario_next` advances an agent-driven run; `scenario_trigger` ");
    out.push_str("advances time/external triggers.\n");
    out.push_str("- `scenario_status` polls run state without mutating it.\n");
    out.push_str("- `scenario_submit` appends external artifacts for audit and later checks.\n");
    out.push_str("- `runpack_export` and `runpack_verify` support offline verification.\n\n");
    out.push_str("## Artifact references\n\n");
    out.push_str("- `authoring.md`: authoring formats and normalization guidance.\n");
    out.push_str("- `examples/scenario.json`: full ScenarioSpec example.\n");
    out.push_str("- `examples/scenario.ron`: authoring-friendly ScenarioSpec example.\n");
    out.push_str("- `examples/run-config.json`: run config example for scenario_start.\n");
    out.push_str("- `examples/decision-gate.toml`: MCP config example for providers.\n\n");
    out.push_str("| Tool | Description |\n");
    out.push_str("| --- | --- |\n");
    for contract in contracts {
        out.push_str("| ");
        out.push_str(contract.name.as_str());
        out.push_str(" | ");
        out.push_str(&contract.description);
        out.push_str(" |\n");
    }
    out.push('\n');
    for contract in contracts {
        out.push_str("## ");
        out.push_str(contract.name.as_str());
        out.push('\n');
        out.push('\n');
        out.push_str(contract.description.as_str());
        out.push('\n');
        out.push('\n');
        out.push_str("### Inputs\n\n");
        render_schema_fields(&mut out, &contract.input_schema);
        out.push('\n');
        out.push_str("### Outputs\n\n");
        render_schema_fields(&mut out, &contract.output_schema);
        out.push('\n');
        if !contract.notes.is_empty() {
            out.push_str("### Notes\n\n");
            for note in &contract.notes {
                out.push_str("- ");
                out.push_str(note);
                out.push('\n');
            }
            out.push('\n');
        }
        append_tool_examples(&mut out, &contract.examples);
    }
    out
}

// ============================================================================
// SECTION: Tooling Markdown Helpers
// ============================================================================

/// Render top-level schema fields as markdown bullet points.
fn render_schema_fields(out: &mut String, schema: &Value) {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        out.push_str("_No fields._\n");
        return;
    };
    let required = required_field_set(schema);
    let mut keys: Vec<&String> = properties.keys().collect();
    keys.sort();
    for key in keys {
        let value = &properties[key];
        let required_label = if required.contains(key) { "required" } else { "optional" };
        let nullable_label = if schema_is_nullable(value) { "nullable" } else { "" };
        let mut qualifiers = vec![required_label.to_string()];
        if !nullable_label.is_empty() {
            qualifiers.push(nullable_label.to_string());
        }
        let qualifier_text = qualifiers.join(", ");
        let description = schema_description(value).unwrap_or_else(|| {
            schema_summary(value).unwrap_or_else(|| String::from("See schema for details."))
        });
        out.push_str("- `");
        out.push_str(key);
        out.push_str("` (");
        out.push_str(&qualifier_text);
        out.push_str("): ");
        out.push_str(&description);
        out.push('\n');
    }
}

/// Collect required field names from a JSON schema object.
fn required_field_set(schema: &Value) -> BTreeSet<String> {
    let mut required = BTreeSet::new();
    if let Some(items) = schema.get("required").and_then(Value::as_array) {
        for item in items {
            if let Some(field) = item.as_str() {
                required.insert(field.to_string());
            }
        }
    }
    required
}

/// Extract a description from a schema if present.
fn schema_description(schema: &Value) -> Option<String> {
    schema.get("description").and_then(Value::as_str).map(str::to_string)
}

/// Provide a short fallback summary when a schema lacks a description.
fn schema_summary(schema: &Value) -> Option<String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return Some(format!("Schema reference `{reference}`."));
    }
    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        let mut options = Vec::new();
        for option in one_of {
            if let Some(label) = schema_type_label(option) {
                options.push(label);
            }
        }
        if !options.is_empty() {
            return Some(format!("One of: {}.", options.join(", ")));
        }
    }
    schema.get("type").and_then(Value::as_str).map(|value| {
        let mut summary = String::from("Type: ");
        summary.push_str(value);
        summary.push('.');
        summary
    })
}

/// Return a concise label for schema types used in summaries.
fn schema_type_label(schema: &Value) -> Option<String> {
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        return Some(kind.to_string());
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return Some(format!("ref {reference}"));
    }
    if let Some(constant) = schema.get("const").and_then(Value::as_str) {
        return Some(format!("const {constant}"));
    }
    None
}

/// Determine whether a schema allows null values.
fn schema_is_nullable(schema: &Value) -> bool {
    schema.get("oneOf").and_then(Value::as_array).is_some_and(|options| {
        options.iter().any(|option| option.get("type").and_then(Value::as_str) == Some("null"))
    })
}

/// Append example input/output payloads for a tool, if defined.
fn append_tool_examples(out: &mut String, examples: &[ToolExample]) {
    if examples.is_empty() {
        return;
    }
    out.push_str("### Example\n\n");
    for (idx, example) in examples.iter().enumerate() {
        if examples.len() > 1 {
            out.push_str("Example ");
            out.push_str(&(idx + 1).to_string());
            out.push_str(": ");
        }
        out.push_str(&example.description);
        out.push('\n');
        out.push('\n');
        out.push_str("Input:\n");
        render_json_block(out, &example.input);
        out.push_str("Output:\n");
        render_json_block(out, &example.output);
    }
}

/// Render a JSON value in a fenced markdown code block.
fn render_json_block(out: &mut String, value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| String::from("{}"));
    out.push_str("```json\n");
    out.push_str(&rendered);
    out.push_str("\n```\n");
}

/// Return example payloads for a tool, if any are defined.
fn tool_examples(tool_name: ToolName) -> Vec<ToolExample> {
    match tool_name {
        ToolName::ScenarioDefine => scenario_define_examples(),
        ToolName::ScenarioStart => scenario_start_examples(),
        ToolName::ScenarioStatus => scenario_status_examples(),
        ToolName::ScenarioNext => scenario_next_examples(),
        ToolName::ScenarioSubmit => scenario_submit_examples(),
        ToolName::ScenarioTrigger => scenario_trigger_examples(),
        ToolName::EvidenceQuery => evidence_query_examples(),
        ToolName::RunpackExport => runpack_export_examples(),
        ToolName::RunpackVerify => runpack_verify_examples(),
        ToolName::ProvidersList => providers_list_examples(),
        ToolName::ProviderContractGet => provider_contract_get_examples(),
        ToolName::ProviderCheckSchemaGet => provider_check_schema_get_examples(),
        ToolName::SchemasRegister => schemas_register_examples(),
        ToolName::SchemasList => schemas_list_examples(),
        ToolName::SchemasGet => schemas_get_examples(),
        ToolName::ScenariosList => scenarios_list_examples(),
        ToolName::Precheck => precheck_examples(),
        ToolName::DecisionGateDocsSearch => decision_gate_docs_search_examples(),
    }
}

// ============================================================================
// SECTION: Schema Helpers (Local)
// ============================================================================

/// Returns a JSON schema for strings.
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

/// Returns example payloads for `scenario_define`.
fn scenario_define_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Register the example scenario spec."),
        input: json!({
            "spec": example_scenario_spec()
        }),
        output: json!({
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "spec_hash": example_hash_digest()
        }),
    }]
}

/// Returns example payloads for `scenario_start`.
fn scenario_start_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Start a run for the example scenario and issue entry packets."),
        input: json!({
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "run_config": example_run_config(),
            "started_at": example_timestamp(),
            "issue_entry_packets": true
        }),
        output: example_run_state(),
    }]
}

/// Returns example payloads for `scenario_status`.
fn scenario_status_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Poll run status without advancing the run."),
        input: json!({
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "request": {
                "tenant_id": EXAMPLE_TENANT_ID,
                "namespace_id": EXAMPLE_NAMESPACE_ID,
                "run_id": EXAMPLE_RUN_ID,
                "requested_at": example_timestamp(),
                "correlation_id": null
            }
        }),
        output: json!({
            "run_id": EXAMPLE_RUN_ID,
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "current_stage_id": EXAMPLE_STAGE_ID,
            "status": "active",
            "last_decision": null,
            "issued_packet_ids": [],
            "safe_summary": null
        }),
    }]
}

/// Returns example payloads for `scenario_next`.
fn scenario_next_examples() -> Vec<ToolExample> {
    vec![
        ToolExample {
            description: String::from("Evaluate the next agent-driven step for a run."),
            input: json!({
                "scenario_id": EXAMPLE_SCENARIO_ID,
                "request": {
                    "tenant_id": EXAMPLE_TENANT_ID,
                    "namespace_id": EXAMPLE_NAMESPACE_ID,
                    "run_id": EXAMPLE_RUN_ID,
                    "trigger_id": EXAMPLE_TRIGGER_ID,
                    "agent_id": EXAMPLE_AGENT_ID,
                    "time": example_timestamp(),
                    "correlation_id": null
                }
            }),
            output: json!({
                "decision": example_decision_record(),
                "packets": [],
                "status": "completed"
            }),
        },
        ToolExample {
            description: String::from("Evaluate a run and request trace feedback."),
            input: json!({
                "scenario_id": EXAMPLE_SCENARIO_ID,
                "request": {
                    "tenant_id": EXAMPLE_TENANT_ID,
                    "namespace_id": EXAMPLE_NAMESPACE_ID,
                    "run_id": EXAMPLE_RUN_ID,
                    "trigger_id": EXAMPLE_TRIGGER_ID,
                    "agent_id": EXAMPLE_AGENT_ID,
                    "time": example_timestamp(),
                    "correlation_id": null
                },
                "feedback": "trace"
            }),
            output: json!({
                "decision": example_decision_record(),
                "packets": [],
                "status": "completed",
                "feedback": {
                    "level": "trace",
                    "gate_evaluations": []
                }
            }),
        },
    ]
}

/// Returns example payloads for `scenario_submit`.
fn scenario_submit_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Submit an external artifact for audit and later evaluation."),
        input: json!({
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "request": {
                "tenant_id": EXAMPLE_TENANT_ID,
                "namespace_id": EXAMPLE_NAMESPACE_ID,
                "run_id": EXAMPLE_RUN_ID,
                "submission_id": EXAMPLE_SUBMISSION_ID,
                "payload": {
                    "kind": "json",
                    "value": {
                        "artifact": "attestation",
                        "status": "approved"
                    }
                },
                "content_type": "application/json",
                "submitted_at": example_timestamp(),
                "correlation_id": null
            }
        }),
        output: json!({
            "record": example_submission_record()
        }),
    }]
}

/// Returns example payloads for `scenario_trigger`.
fn scenario_trigger_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Advance a run from a scheduler or external trigger."),
        input: json!({
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "trigger": {
                "trigger_id": EXAMPLE_TRIGGER_ID,
                "tenant_id": EXAMPLE_TENANT_ID,
                "namespace_id": EXAMPLE_NAMESPACE_ID,
                "run_id": EXAMPLE_RUN_ID,
                "kind": "tick",
                "time": example_timestamp(),
                "source_id": "scheduler-01",
                "payload": null,
                "correlation_id": null
            }
        }),
        output: json!({
            "decision": example_decision_record(),
            "packets": [],
            "status": "completed"
        }),
    }]
}

/// Returns example payloads for `evidence_query`.
fn evidence_query_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Query an evidence provider using the run context."),
        input: json!({
            "query": {
                "provider_id": "env",
                "check_id": "get",
                "params": { "key": "DEPLOY_ENV" }
            },
            "context": {
                "tenant_id": EXAMPLE_TENANT_ID,
                "namespace_id": EXAMPLE_NAMESPACE_ID,
                "run_id": EXAMPLE_RUN_ID,
                "scenario_id": EXAMPLE_SCENARIO_ID,
                "stage_id": EXAMPLE_STAGE_ID,
                "trigger_id": EXAMPLE_TRIGGER_ID,
                "trigger_time": example_timestamp(),
                "correlation_id": null
            }
        }),
        output: json!({
            "result": {
                "value": {
                    "kind": "json",
                    "value": "production"
                },
                "lane": "verified",
                "error": null,
                "evidence_hash": example_hash_digest(),
                "evidence_ref": null,
                "evidence_anchor": {
                    "anchor_type": "env",
                    "anchor_value": "DEPLOY_ENV"
                },
                "signature": null,
                "content_type": "text/plain"
            }
        }),
    }]
}

/// Returns example payloads for `runpack_export`.
fn runpack_export_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Export a runpack with manifest metadata."),
        input: json!({
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "tenant_id": EXAMPLE_TENANT_ID,
            "namespace_id": EXAMPLE_NAMESPACE_ID,
            "run_id": EXAMPLE_RUN_ID,
            "output_dir": "/var/lib/decision-gate/runpacks/run-0001",
            "manifest_name": "manifest.json",
            "generated_at": example_timestamp(),
            "include_verification": false
        }),
        output: json!({
            "manifest": example_runpack_manifest(),
            "report": null,
            "storage_uri": null
        }),
    }]
}

/// Returns example payloads for `runpack_verify`.
fn runpack_verify_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Verify a runpack manifest and artifacts offline."),
        input: json!({
            "runpack_dir": "/var/lib/decision-gate/runpacks/run-0001",
            "manifest_path": "manifest.json"
        }),
        output: json!({
            "report": {
                "status": "pass",
                "checked_files": 12,
                "errors": []
            },
            "status": "pass"
        }),
    }]
}

/// Returns example payloads for `providers_list`.
fn providers_list_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("List registered evidence providers."),
        input: json!({}),
        output: json!({
            "providers": [
                {
                    "provider_id": "env",
                    "transport": "builtin",
                    "checks": ["get"]
                }
            ]
        }),
    }]
}

/// Returns example payloads for `provider_contract_get`.
fn provider_contract_get_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Fetch the contract JSON for a provider."),
        input: json!({
            "provider_id": "json"
        }),
        output: json!({
            "provider_id": "json",
            "contract": {
                "provider_id": "json",
                "name": "JSON Provider",
                "description": "Reads JSON or YAML files and evaluates JSONPath.",
                "transport": "builtin",
                "config_schema": { "type": "object", "additionalProperties": false },
                "checks": [],
                "notes": []
            },
            "contract_hash": example_hash_digest(),
            "source": "builtin",
            "version": null
        }),
    }]
}

/// Returns example payloads for `provider_check_schema_get`.
fn provider_check_schema_get_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Fetch check schema details for a provider."),
        input: json!({
            "provider_id": "json",
            "check_id": "path"
        }),
        output: json!({
            "provider_id": "json",
            "check_id": "path",
            "params_required": true,
            "params_schema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "jsonpath": { "type": "string" }
                },
                "required": ["file"]
            },
            "result_schema": { "type": ["null", "string", "number", "boolean", "array", "object"] },
            "allowed_comparators": ["equals", "in_set", "exists", "not_exists"],
            "determinism": "external",
            "anchor_types": [],
            "content_types": ["application/json"],
            "examples": [],
            "contract_hash": example_hash_digest()
        }),
    }]
}

/// Returns example payloads for `schemas_register`.
fn schemas_register_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Register a data shape schema."),
        input: json!({
            "record": example_data_shape_record()
        }),
        output: json!({
            "record": example_data_shape_record()
        }),
    }]
}

/// Returns example payloads for `schemas_list`.
fn schemas_list_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("List data shapes for a namespace."),
        input: json!({
            "tenant_id": EXAMPLE_TENANT_ID,
            "namespace_id": EXAMPLE_NAMESPACE_ID,
            "cursor": null,
            "limit": 50
        }),
        output: json!({
            "items": [example_data_shape_record()],
            "next_token": null
        }),
    }]
}

/// Returns example payloads for `schemas_get`.
fn schemas_get_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Fetch a data shape by identifier and version."),
        input: json!({
            "tenant_id": EXAMPLE_TENANT_ID,
            "namespace_id": EXAMPLE_NAMESPACE_ID,
            "schema_id": "asserted_payload",
            "version": "v1"
        }),
        output: json!({
            "record": example_data_shape_record()
        }),
    }]
}

/// Returns example payloads for `scenarios_list`.
fn scenarios_list_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("List scenarios for a namespace."),
        input: json!({
            "tenant_id": EXAMPLE_TENANT_ID,
            "namespace_id": EXAMPLE_NAMESPACE_ID,
            "cursor": null,
            "limit": 50
        }),
        output: json!({
            "items": [
                {
                    "scenario_id": EXAMPLE_SCENARIO_ID,
                    "namespace_id": EXAMPLE_NAMESPACE_ID,
                    "spec_hash": example_hash_digest()
                }
            ],
            "next_token": null
        }),
    }]
}

/// Returns example payloads for `precheck`.
fn precheck_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Precheck a scenario with asserted data."),
        input: json!({
            "tenant_id": EXAMPLE_TENANT_ID,
            "namespace_id": EXAMPLE_NAMESPACE_ID,
            "scenario_id": EXAMPLE_SCENARIO_ID,
            "spec": null,
            "stage_id": null,
            "data_shape": {
                "schema_id": "asserted_payload",
                "version": "v1"
            },
            "payload": {
                "deploy_env": "production"
            }
        }),
        output: json!({
            "decision": {
                "kind": "hold",
                "summary": {
                    "status": "hold",
                    "unmet_gates": ["ready"],
                    "retry_hint": "await_evidence",
                    "policy_tags": []
                }
            },
            "gate_evaluations": []
        }),
    }]
}

/// Returns example payloads for `decision_gate_docs_search`.
fn decision_gate_docs_search_examples() -> Vec<ToolExample> {
    vec![ToolExample {
        description: String::from("Search for evidence flow and trust lane guidance."),
        input: json!({
            "query": "precheck vs live run trust lanes",
            "max_sections": 2
        }),
        output: json!({
            "sections": [
                {
                    "rank": 0,
                    "doc_id": "evidence_flow_and_execution_model",
                    "doc_title": "Evidence Flow + Execution Model",
                    "doc_role": "reasoning",
                    "heading": "Core Data Flow",
                    "content": "..."
                }
            ],
            "docs_covered": [
                {
                    "doc_id": "evidence_flow_and_execution_model",
                    "doc_title": "Evidence Flow + Execution Model",
                    "doc_role": "reasoning"
                }
            ],
            "suggested_followups": [
                "Refine the query with comparator or provider keywords for targeted guidance."
            ]
        }),
    }]
}

/// Example tenant identifier used in tooling samples.
const EXAMPLE_TENANT_ID: u64 = 1;
/// Example namespace identifier used in tooling samples.
const EXAMPLE_NAMESPACE_ID: u64 = 1;
/// Example run identifier used in tooling samples.
const EXAMPLE_RUN_ID: &str = "run-0001";
/// Example scenario identifier used in tooling samples.
const EXAMPLE_SCENARIO_ID: &str = "example-scenario";
/// Example stage identifier used in tooling samples.
const EXAMPLE_STAGE_ID: &str = "main";
/// Example trigger identifier used in tooling samples.
const EXAMPLE_TRIGGER_ID: &str = "trigger-0001";
/// Example agent identifier used in tooling samples.
const EXAMPLE_AGENT_ID: &str = "agent-alpha";
/// Example submission identifier used in tooling samples.
const EXAMPLE_SUBMISSION_ID: &str = "submission-0001";
/// Example SHA-256 digest value used in tooling samples.
const EXAMPLE_HASH: &str = "5c3a5b6bce0f4a2c9e22c4fa6a1e6d8d90b0f2dfed1b7f1e9b3d3b3d1f0c9b21";

/// Example timestamp payload used in tooling samples.
fn example_timestamp() -> Value {
    json!({
        "kind": "unix_millis",
        "value": 1_710_000_000_000_i64
    })
}

/// Example hash digest payload used in tooling samples.
fn example_hash_digest() -> Value {
    json!({
        "algorithm": "sha256",
        "value": EXAMPLE_HASH
    })
}

/// Example [`decision_gate_core::ScenarioSpec`] payload rendered from core types.
fn example_scenario_spec() -> Value {
    serde_json::to_value(examples::scenario_example()).unwrap_or_else(|_| json!({}))
}

/// Example [`decision_gate_core::RunConfig`] payload rendered from core types.
fn example_run_config() -> Value {
    serde_json::to_value(examples::run_config_example()).unwrap_or_else(|_| json!({}))
}

/// Example [`decision_gate_core::RunState`] payload used in tooling docs.
fn example_run_state() -> Value {
    json!({
        "tenant_id": EXAMPLE_TENANT_ID,
        "namespace_id": EXAMPLE_NAMESPACE_ID,
        "run_id": EXAMPLE_RUN_ID,
        "scenario_id": EXAMPLE_SCENARIO_ID,
        "spec_hash": example_hash_digest(),
        "current_stage_id": EXAMPLE_STAGE_ID,
        "stage_entered_at": example_timestamp(),
        "status": "active",
        "dispatch_targets": [
            {
                "kind": "agent",
                "agent_id": EXAMPLE_AGENT_ID
            }
        ],
        "triggers": [],
        "gate_evals": [],
        "decisions": [],
        "packets": [],
        "submissions": [],
        "tool_calls": []
    })
}

/// Example decision record payload used in tooling docs.
fn example_decision_record() -> Value {
    json!({
        "decision_id": "decision-0001",
        "seq": 0,
        "trigger_id": EXAMPLE_TRIGGER_ID,
        "stage_id": EXAMPLE_STAGE_ID,
        "decided_at": example_timestamp(),
        "outcome": {
            "kind": "complete",
            "stage_id": EXAMPLE_STAGE_ID
        },
        "correlation_id": null
    })
}

/// Example submission record payload used in tooling docs.
fn example_submission_record() -> Value {
    json!({
        "submission_id": EXAMPLE_SUBMISSION_ID,
        "run_id": EXAMPLE_RUN_ID,
        "payload": {
            "kind": "json",
            "value": {
                "artifact": "attestation",
                "status": "approved"
            }
        },
        "content_type": "application/json",
        "content_hash": example_hash_digest(),
        "submitted_at": example_timestamp(),
        "correlation_id": null
    })
}

/// Example data shape record payload used in tooling docs.
fn example_data_shape_record() -> Value {
    json!({
        "tenant_id": EXAMPLE_TENANT_ID,
        "namespace_id": EXAMPLE_NAMESPACE_ID,
        "schema_id": "asserted_payload",
        "version": "v1",
        "schema": {
            "type": "object",
            "properties": {
                "deploy_env": { "type": "string" }
            },
            "required": ["deploy_env"],
            "additionalProperties": false
        },
        "description": "Asserted payload schema.",
        "created_at": example_timestamp()
    })
}

/// Example runpack manifest payload used in tooling docs.
fn example_runpack_manifest() -> Value {
    json!({
        "manifest_version": "v1",
        "generated_at": example_timestamp(),
        "scenario_id": EXAMPLE_SCENARIO_ID,
        "tenant_id": EXAMPLE_TENANT_ID,
        "namespace_id": EXAMPLE_NAMESPACE_ID,
        "run_id": EXAMPLE_RUN_ID,
        "spec_hash": example_hash_digest(),
        "hash_algorithm": "sha256",
        "verifier_mode": "offline_strict",
        "integrity": {
            "file_hashes": [
                {
                    "path": "decision_log.json",
                    "hash": example_hash_digest()
                }
            ],
            "root_hash": example_hash_digest()
        },
        "artifacts": [
            {
                "artifact_id": "decision_log",
                "kind": "decision_log",
                "path": "decision_log.json",
                "content_type": "application/json",
                "hash": example_hash_digest(),
                "required": true
            }
        ]
    })
}

// ============================================================================
// SECTION: Tool Schema Builders
// ============================================================================

/// Builds the input schema for `scenario_define`.
#[must_use]
fn scenario_define_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "spec": {
                "$ref": "decision-gate://contract/schemas/scenario.schema.json",
                "description": "Scenario specification to register."
            }
        }),
        &["spec"],
    )
}

/// Builds the output schema for `scenario_define`.
#[must_use]
fn scenario_define_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "spec_hash": schemas::hash_digest_schema()
        }),
        &["scenario_id", "spec_hash"],
    )
}

/// Builds the input schema for `scenario_start`.
#[must_use]
fn scenario_start_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "run_config": describe_schema(schemas::run_config_schema(), "Run configuration and dispatch targets."),
            "started_at": describe_schema(schemas::timestamp_schema(), "Caller-supplied run start timestamp."),
            "issue_entry_packets": describe_schema(json!({ "type": "boolean" }), "Issue entry packets immediately.")
        }),
        &["scenario_id", "run_config", "started_at", "issue_entry_packets"],
    )
}

/// Builds the input schema for `scenario_status`.
#[must_use]
fn scenario_status_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "request": describe_schema(schemas::status_request_schema(), "Status request payload.")
        }),
        &["scenario_id", "request"],
    )
}

/// Builds the input schema for `scenario_next`.
#[must_use]
fn scenario_next_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "request": describe_schema(schemas::next_request_schema(), "Next request payload from an agent."),
            "feedback": describe_schema(json!({
                "oneOf": [
                    { "type": "null" },
                    schemas::feedback_level_schema()
                ]
            }), "Optional feedback level override for scenario_next.")
        }),
        &["scenario_id", "request"],
    )
}

/// Builds the input schema for `scenario_submit`.
#[must_use]
fn scenario_submit_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "request": describe_schema(schemas::submit_request_schema(), "Submission payload and metadata.")
        }),
        &["scenario_id", "request"],
    )
}

/// Builds the input schema for `scenario_trigger`.
#[must_use]
fn scenario_trigger_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "trigger": describe_schema(schemas::trigger_event_schema(), "Trigger event payload.")
        }),
        &["scenario_id", "trigger"],
    )
}

/// Builds the input schema for `evidence_query`.
#[must_use]
fn evidence_query_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "query": describe_schema(schemas::evidence_query_schema(), "Evidence query payload."),
            "context": describe_schema(evidence_context_schema(), "Evidence context used for evaluation.")
        }),
        &["query", "context"],
    )
}

/// Builds the output schema for `evidence_query`.
#[must_use]
fn evidence_query_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "result": schemas::evidence_result_schema()
        }),
        &["result"],
    )
}

/// Builds the input schema for `runpack_export`.
#[must_use]
fn runpack_export_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "scenario_id": schema_identifier("Scenario identifier."),
            "tenant_id": schema_numeric_identifier("Tenant identifier."),
            "namespace_id": schema_numeric_identifier("Namespace identifier."),
            "run_id": schema_identifier("Run identifier."),
            "output_dir": describe_schema(json!({
                "oneOf": [
                    { "type": "null" },
                    schema_path("Output directory path.")
                ]
            }), "Optional output directory (required for filesystem export)."),
            "manifest_name": describe_schema(json!({
                "oneOf": [
                    { "type": "null" },
                    schema_filename("Manifest file name.")
                ]
            }), "Optional override for the manifest file name."),
            "generated_at": describe_schema(schemas::timestamp_schema(), "Timestamp recorded in the manifest."),
            "include_verification": describe_schema(json!({ "type": "boolean" }), "Generate a verification report artifact.")
        }),
        &[
            "scenario_id",
            "tenant_id",
            "namespace_id",
            "run_id",
            "generated_at",
            "include_verification",
        ],
    )
}

/// Builds the output schema for `runpack_export`.
#[must_use]
fn runpack_export_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "manifest": schemas::runpack_manifest_schema(),
            "report": {
                "oneOf": [
                    { "type": "null" },
                    schemas::verification_report_schema()
                ]
            },
            "storage_uri": describe_schema(json!({
                "oneOf": [
                    { "type": "null" },
                    { "type": "string" }
                ]
            }), "Optional storage URI for managed runpack storage backends.")
        }),
        &["manifest", "report"],
    )
}

/// Builds the input schema for `runpack_verify`.
#[must_use]
fn runpack_verify_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "runpack_dir": schema_path("Runpack root directory."),
            "manifest_path": schema_path("Manifest path relative to runpack root.")
        }),
        &["runpack_dir", "manifest_path"],
    )
}

/// Builds the output schema for `runpack_verify`.
#[must_use]
fn runpack_verify_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "report": schemas::verification_report_schema(),
            "status": schemas::verification_status_schema()
        }),
        &["report", "status"],
    )
}

/// Builds the input schema for `providers_list`.
#[must_use]
fn providers_list_input_schema() -> Value {
    tool_input_schema(&json!({}), &[])
}

/// Builds the output schema for `providers_list`.
#[must_use]
fn providers_list_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "providers": {
                "type": "array",
                "items": provider_summary_schema()
            }
        }),
        &["providers"],
    )
}

/// Builds the input schema for `provider_contract_get`.
#[must_use]
fn provider_contract_get_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "provider_id": schema_identifier("Provider identifier.")
        }),
        &["provider_id"],
    )
}

/// Builds the output schema for `provider_contract_get`.
#[must_use]
fn provider_contract_get_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "provider_id": schema_identifier("Provider identifier."),
            "contract": schemas::provider_contract_schema(),
            "contract_hash": schemas::hash_digest_schema(),
            "source": {
                "type": "string",
                "enum": ["builtin", "file"],
                "description": "Contract source origin."
            },
            "version": {
                "oneOf": [
                    { "type": "null" },
                    { "type": "string" }
                ],
                "description": "Optional contract version label."
            }
        }),
        &["provider_id", "contract", "contract_hash", "source", "version"],
    )
}

/// Builds the input schema for `provider_check_schema_get`.
#[must_use]
fn provider_check_schema_get_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "provider_id": schema_identifier("Provider identifier."),
            "check_id": schema_for_string("Provider check identifier.")
        }),
        &["provider_id", "check_id"],
    )
}

/// Builds the output schema for `provider_check_schema_get`.
#[must_use]
fn provider_check_schema_get_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "provider_id": schema_identifier("Provider identifier."),
            "check_id": schema_for_string("Check identifier."),
            "params_required": {
                "type": "boolean",
                "description": "Whether params are required for this check."
            },
            "params_schema": schema_for_json_value("JSON schema for check params."),
            "result_schema": schema_for_json_value("JSON schema for check result value."),
            "allowed_comparators": {
                "type": "array",
                "items": schemas::comparator_schema(),
                "description": "Comparator allow-list for this check."
            },
            "determinism": schemas::determinism_class_schema(),
            "anchor_types": schema_for_string_array("Anchor types emitted by this check."),
            "content_types": schema_for_string_array("Content types for check output."),
            "examples": {
                "type": "array",
                "items": schemas::check_example_schema()
            },
            "contract_hash": schemas::hash_digest_schema()
        }),
        &[
            "provider_id",
            "check_id",
            "params_required",
            "params_schema",
            "result_schema",
            "allowed_comparators",
            "determinism",
            "anchor_types",
            "content_types",
            "examples",
            "contract_hash",
        ],
    )
}

/// Builds the input schema for `schemas_register`.
#[must_use]
fn schemas_register_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "record": schemas::data_shape_record_schema()
        }),
        &["record"],
    )
}

/// Builds the output schema for `schemas_register`.
#[must_use]
fn schemas_register_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "record": schemas::data_shape_record_schema()
        }),
        &["record"],
    )
}

/// Builds the input schema for `schemas_list`.
#[must_use]
fn schemas_list_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "tenant_id": schema_numeric_identifier("Tenant identifier."),
            "namespace_id": schema_numeric_identifier("Namespace identifier."),
            "cursor": {
                "oneOf": [
                    { "type": "null" },
                    schema_identifier("Pagination cursor.")
                ]
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 1000,
                "description": "Maximum number of records to return."
            }
        }),
        &["tenant_id", "namespace_id"],
    )
}

/// Builds the output schema for `schemas_list`.
#[must_use]
fn schemas_list_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "items": {
                "type": "array",
                "items": schemas::data_shape_record_schema()
            },
            "next_token": {
                "oneOf": [
                    { "type": "null" },
                    schema_identifier("Pagination token for the next page.")
                ]
            }
        }),
        &["items", "next_token"],
    )
}

/// Builds the input schema for `schemas_get`.
#[must_use]
fn schemas_get_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "tenant_id": schema_numeric_identifier("Tenant identifier."),
            "namespace_id": schema_numeric_identifier("Namespace identifier."),
            "schema_id": schema_identifier("Data shape identifier."),
            "version": schema_identifier("Data shape version identifier.")
        }),
        &["tenant_id", "namespace_id", "schema_id", "version"],
    )
}

/// Builds the output schema for `schemas_get`.
#[must_use]
fn schemas_get_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "record": schemas::data_shape_record_schema()
        }),
        &["record"],
    )
}

/// Builds the input schema for `scenarios_list`.
#[must_use]
fn scenarios_list_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "tenant_id": schema_numeric_identifier("Tenant identifier."),
            "namespace_id": schema_numeric_identifier("Namespace identifier."),
            "cursor": {
                "oneOf": [
                    { "type": "null" },
                    schema_identifier("Pagination cursor.")
                ]
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 1000,
                "description": "Maximum number of records to return."
            }
        }),
        &["tenant_id", "namespace_id"],
    )
}

/// Builds the output schema for `scenarios_list`.
#[must_use]
fn scenarios_list_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "items": {
                "type": "array",
                "items": scenario_summary_schema()
            },
            "next_token": {
                "oneOf": [
                    { "type": "null" },
                    schema_identifier("Pagination token for the next page.")
                ]
            }
        }),
        &["items", "next_token"],
    )
}

/// Builds the input schema for `precheck`.
#[must_use]
fn precheck_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "tenant_id": schema_numeric_identifier("Tenant identifier."),
            "namespace_id": schema_numeric_identifier("Namespace identifier."),
            "scenario_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_identifier("Scenario identifier.")
                ]
            },
            "spec": {
                "oneOf": [
                    { "type": "null" },
                    { "$ref": "decision-gate://contract/schemas/scenario.schema.json" }
                ]
            },
            "stage_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_identifier("Stage identifier override.")
                ]
            },
            "data_shape": schemas::data_shape_ref_schema(),
            "payload": {
                "type": ["null", "boolean", "number", "string", "array", "object"],
                "description": "Asserted data payload."
            }
        }),
        &["tenant_id", "namespace_id", "data_shape", "payload"],
    )
}

/// Builds the output schema for `precheck`.
#[must_use]
fn precheck_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "decision": schemas::decision_outcome_schema(),
            "gate_evaluations": {
                "type": "array",
                "items": schemas::gate_evaluation_schema()
            }
        }),
        &["decision", "gate_evaluations"],
    )
}

/// Builds the input schema for `decision_gate_docs_search`.
#[must_use]
fn decision_gate_docs_search_input_schema() -> Value {
    tool_input_schema(
        &json!({
            "query": schema_for_string("Search query for documentation sections."),
            "max_sections": {
                "type": "integer",
                "minimum": 1,
                "maximum": 10,
                "description": "Maximum number of sections to return (default 3, hard cap 10)."
            }
        }),
        &["query"],
    )
}

/// Builds the output schema for `decision_gate_docs_search`.
#[must_use]
fn decision_gate_docs_search_output_schema() -> Value {
    tool_output_schema(
        &json!({
            "sections": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["rank", "doc_id", "doc_title", "doc_role", "heading", "content"],
                    "properties": {
                        "rank": { "type": "integer", "minimum": 0 },
                        "doc_id": schema_identifier("Document identifier."),
                        "doc_title": schema_for_string("Document title."),
                        "doc_role": doc_role_schema(),
                        "heading": schema_for_string("Section heading."),
                        "content": schema_for_string("Section content (raw Markdown).")
                    },
                    "additionalProperties": false
                }
            },
            "docs_covered": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["doc_id", "doc_title", "doc_role"],
                    "properties": {
                        "doc_id": schema_identifier("Document identifier."),
                        "doc_title": schema_for_string("Document title."),
                        "doc_role": doc_role_schema()
                    },
                    "additionalProperties": false
                }
            },
            "suggested_followups": schema_for_string_array("Role-aware follow-up prompts.")
        }),
        &["sections", "docs_covered", "suggested_followups"],
    )
}

/// Returns the JSON schema for provider summaries.
#[must_use]
fn provider_summary_schema() -> Value {
    json!({
        "type": "object",
        "required": ["provider_id", "transport", "checks"],
        "properties": {
            "provider_id": schema_identifier("Provider identifier."),
            "transport": {
                "type": "string",
                "enum": ["builtin", "mcp"],
                "description": "Provider transport type."
            },
            "checks": {
                "type": "array",
                "items": schema_identifier("Check identifier.")
            }
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for scenario summaries.
#[must_use]
fn scenario_summary_schema() -> Value {
    json!({
        "type": "object",
        "required": ["scenario_id", "namespace_id", "spec_hash"],
        "properties": {
            "scenario_id": schema_identifier("Scenario identifier."),
            "namespace_id": schema_numeric_identifier("Namespace identifier."),
            "spec_hash": schemas::hash_digest_schema()
        },
        "additionalProperties": false
    })
}

/// Returns the JSON schema for documentation roles.
#[must_use]
fn doc_role_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["reasoning", "decision", "ontology", "pattern"],
        "description": "Documentation role for search weighting and display."
    })
}

// ============================================================================
// SECTION: Tool Schema Helpers
// ============================================================================

/// Builds a tool contract from the provided schema payloads.
///
/// Notes and examples are surfaced in SDK docstrings and generated docs, so
/// keep them user-facing and implementation-agnostic.
#[must_use]
fn build_tool_contract(
    name: ToolName,
    description: &str,
    input_schema: Value,
    output_schema: Value,
    examples: Vec<ToolExample>,
    notes: Vec<String>,
) -> ToolContract {
    ToolContract {
        name,
        description: description.to_string(),
        input_schema,
        output_schema,
        examples,
        notes,
    }
}

/// Builds a standard tool input schema wrapper.
#[must_use]
fn tool_input_schema(properties: &Value, required: &[&str]) -> Value {
    with_schema(object_schema(properties, required))
}

/// Builds a standard tool output schema wrapper.
#[must_use]
fn tool_output_schema(properties: &Value, required: &[&str]) -> Value {
    with_schema(object_schema(properties, required))
}

/// Returns the JSON schema for [`decision_gate_core::EvidenceContext`].
#[must_use]
fn evidence_context_schema() -> Value {
    let properties = json!({
        "tenant_id": schema_numeric_identifier("Tenant identifier."),
        "namespace_id": schema_numeric_identifier("Namespace identifier."),
        "run_id": schema_identifier("Run identifier."),
        "scenario_id": schema_identifier("Scenario identifier."),
        "stage_id": schema_identifier("Stage identifier."),
        "trigger_id": schema_identifier("Trigger identifier."),
        "trigger_time": schemas::timestamp_schema(),
        "correlation_id": {
            "oneOf": [
                { "type": "null" },
                schema_identifier("Correlation identifier.")
            ]
        }
    });
    object_schema(
        &properties,
        &["tenant_id", "run_id", "scenario_id", "stage_id", "trigger_id", "trigger_time"],
    )
}

/// Builds an object schema without the top-level `$schema` annotation.
#[must_use]
fn object_schema(properties: &Value, required: &[&str]) -> Value {
    let required_values: Vec<Value> =
        required.iter().map(|value| Value::String((*value).to_string())).collect();
    json!({
        "type": "object",
        "required": required_values,
        "properties": properties,
        "additionalProperties": false
    })
}

/// Adds a `$schema` header to a top-level JSON schema.
#[must_use]
fn with_schema(schema: Value) -> Value {
    let Value::Object(mut map) = schema else {
        return schema;
    };
    map.insert(
        String::from("$schema"),
        Value::String(String::from("https://json-schema.org/draft/2020-12/schema")),
    );
    Value::Object(map)
}

/// Returns a schema describing identifiers.
#[must_use]
fn schema_identifier(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

/// Returns a schema describing numeric identifiers (1-based, non-zero).
#[must_use]
fn schema_numeric_identifier(description: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "description": description
    })
}

/// Returns a schema describing filesystem paths.
#[must_use]
fn schema_path(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

/// Returns a schema describing filenames.
#[must_use]
fn schema_filename(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

/// Attach a description to a JSON schema object when possible.
fn describe_schema(schema: Value, description: &str) -> Value {
    let Value::Object(mut map) = schema else {
        return schema;
    };
    map.insert(String::from("description"), Value::String(description.to_string()));
    Value::Object(map)
}

#[cfg(test)]
mod tests;
