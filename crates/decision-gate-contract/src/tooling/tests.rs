// crates/decision-gate-contract/src/tooling/tests.rs
// ============================================================================
// Module: Tooling Schema Unit Tests
// Description: Validates tool examples against their JSON schemas.
// Purpose: Ensure contract examples are kept in sync with schema definitions.
// Dependencies: decision-gate-contract, jsonschema, serde_json
// ============================================================================

//! ## Overview
//! Verifies that tool input/output examples satisfy their JSON schemas.
//!
//! Security posture: Tests validate untrusted input contracts; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    clippy::missing_docs_in_private_items,
    reason = "Test-only validation helpers use panic-based assertions for clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeSet;

use jsonschema::Draft;
use jsonschema::Registry;
use jsonschema::Validator;
use serde_json::Value;

use super::tool_contracts;
use super::tool_examples;
use crate::schemas;
use crate::types::ToolName;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

fn compile_schema(schema: &Value, registry: &Registry) -> Validator {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(registry.clone())
        .build(schema)
        .expect("schema compilation failed")
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn tool_examples_match_tool_schemas() {
    let scenario_schema = schemas::scenario_schema();
    let id =
        scenario_schema.get("$id").and_then(Value::as_str).expect("scenario schema missing $id");
    let registry =
        Registry::try_new(id, Draft::Draft202012.create_resource(scenario_schema.clone()))
            .expect("schema registry build failed");

    for contract in tool_contracts() {
        let input_schema = compile_schema(&contract.input_schema, &registry);
        let output_schema = compile_schema(&contract.output_schema, &registry);
        let examples = tool_examples(contract.name);
        assert!(!examples.is_empty(), "tool examples missing for {}", contract.name);
        for example in examples {
            assert!(
                input_schema.is_valid(&example.input),
                "input example failed for {}",
                contract.name
            );
            assert!(
                output_schema.is_valid(&example.output),
                "output example failed for {}",
                contract.name
            );
        }
    }
}

#[test]
fn evidence_query_context_schema_requires_namespace_id() {
    let contracts = tool_contracts();
    let Some(contract) =
        contracts.into_iter().find(|contract| contract.name == ToolName::EvidenceQuery)
    else {
        panic!("evidence_query contract missing");
    };
    let required = contract
        .input_schema
        .get("properties")
        .and_then(|value| value.get("context"))
        .and_then(|value| value.get("required"))
        .and_then(Value::as_array)
        .expect("evidence_query context.required missing");
    let required: BTreeSet<&str> = required.iter().filter_map(Value::as_str).collect();
    let expected = BTreeSet::from([
        "tenant_id",
        "namespace_id",
        "run_id",
        "scenario_id",
        "stage_id",
        "trigger_id",
        "trigger_time",
    ]);
    assert_eq!(required, expected, "evidence_query context required fields drifted");
}
