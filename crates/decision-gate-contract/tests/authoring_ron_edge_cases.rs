// crates/decision-gate-contract/tests/authoring_ron_edge_cases.rs
// ============================================================================
// Module: Authoring RON Edge Case Tests
// Description: Validate RON authoring edge cases and diagnostics.
// Purpose: Ensure schema enforcement and null handling for RON inputs.
// Dependencies: decision-gate-contract, ron, serde_json
// ============================================================================

//! ## Overview
//! Exercises RON-specific edge cases and schema enforcement behavior.
//! Security posture: authoring inputs are untrusted; see
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
    reason = "Test-only authoring validation uses panic-based assertions."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_contract::AuthoringError;
use decision_gate_contract::AuthoringFormat;
use decision_gate_contract::authoring::normalize_scenario;
use ron::ser::PrettyConfig;
use serde_json::Value;
use serde_json::json;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

fn minimal_spec_value() -> Value {
    json!({
        "scenario_id": "edge-scenario",
        "namespace_id": 1,
        "spec_version": "v1",
        "stages": [
            {
                "stage_id": "stage-1",
                "entry_packets": [],
                "gates": [
                    {
                        "gate_id": "gate-1",
                        "requirement": { "Condition": "cond-1" }
                    }
                ],
                "advance_to": { "kind": "terminal" },
                "on_timeout": "fail"
            }
        ],
        "conditions": [
            {
                "condition_id": "cond-1",
                "query": {
                    "provider_id": "time",
                    "check_id": "after",
                    "params": { "timestamp": 0 }
                },
                "comparator": "equals",
                "expected": true,
                "policy_tags": []
            }
        ],
        "policies": [],
        "schemas": []
    })
}

fn ron_from_value(value: &Value) -> String {
    let pretty = PrettyConfig::new().depth_limit(6).separate_tuple_members(true);
    ron::ser::to_string_pretty(value, pretty).expect("ron serialize")
}

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Confirms explicit nulls for optional fields are accepted.
#[test]
fn ron_null_optional_fields_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = minimal_spec_value();
    value["default_tenant_id"] = Value::Null;
    value["stages"][0]["timeout"] = Value::Null;
    let ron_input = ron_from_value(&value);
    let _normalized = normalize_scenario(&ron_input, AuthoringFormat::Ron)?;
    Ok(())
}

/// Confirms empty required arrays are rejected.
#[test]
fn ron_empty_stages_rejected() {
    let mut value = minimal_spec_value();
    value["stages"] = json!([]);
    let ron_input = ron_from_value(&value);
    let err = normalize_scenario(&ron_input, AuthoringFormat::Ron).unwrap_err();
    assert!(matches!(err, AuthoringError::Schema { .. }));
}

/// Confirms invalid enum values are rejected by schema validation.
#[test]
fn ron_invalid_enum_rejected() {
    let mut value = minimal_spec_value();
    value["conditions"][0]["comparator"] = json!("bogus");
    let ron_input = ron_from_value(&value);
    let err = normalize_scenario(&ron_input, AuthoringFormat::Ron).unwrap_err();
    assert!(matches!(err, AuthoringError::Schema { .. }));
}

/// Confirms invalid requirement tags are rejected by schema validation.
#[test]
fn ron_invalid_requirement_tag_rejected() {
    let mut value = minimal_spec_value();
    value["stages"][0]["gates"][0]["requirement"] = json!({ "Unknown": "pred-1" });
    let ron_input = ron_from_value(&value);
    let err = normalize_scenario(&ron_input, AuthoringFormat::Ron).unwrap_err();
    assert!(matches!(err, AuthoringError::Schema { .. }));
}
