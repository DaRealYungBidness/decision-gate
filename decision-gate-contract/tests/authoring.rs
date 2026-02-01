// decision-gate-contract/tests/authoring.rs
// ============================================================================
// Module: Authoring Format Tests
// Description: Validate authoring normalization and schema enforcement.
// Purpose: Ensure JSON/RON authoring normalize to canonical JSON bytes.
// Dependencies: decision-gate-contract, decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! Tests normalization of JSON/RON authoring into canonical JSON and schema
//! enforcement outcomes.
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
use decision_gate_contract::authoring::MAX_AUTHORING_DEPTH;
use decision_gate_contract::authoring::MAX_AUTHORING_INPUT_BYTES;
use decision_gate_contract::authoring::normalize_scenario;
use decision_gate_contract::examples;
use decision_gate_core::hashing::canonical_json_bytes;
use serde_json::json;

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Confirms JSON authoring input normalizes to canonical JSON bytes.
#[test]
fn normalize_json_matches_canonical_output() -> Result<(), Box<dyn std::error::Error>> {
    let spec = examples::scenario_example();
    let input = serde_json::to_string(&spec)?;
    let normalized = normalize_scenario(&input, AuthoringFormat::Json)?;
    let expected = canonical_json_bytes(&spec)?;
    assert_eq!(normalized.spec, spec);
    assert_eq!(normalized.canonical_json, expected);
    Ok(())
}

/// Confirms RON authoring input normalizes to the same `ScenarioSpec`.
#[test]
fn normalize_ron_matches_canonical_output() -> Result<(), Box<dyn std::error::Error>> {
    let spec = examples::scenario_example();
    let ron = examples::scenario_example_ron()?;
    let normalized = normalize_scenario(&ron, AuthoringFormat::Ron)?;
    assert_eq!(normalized.spec, spec);
    Ok(())
}

/// Confirms schema validation rejects unknown fields.
#[test]
fn schema_validation_rejects_unknown_fields() -> Result<(), Box<dyn std::error::Error>> {
    let spec = examples::scenario_example();
    let mut value = serde_json::to_value(spec)?;
    value["unexpected"] = json!(true);
    let input = serde_json::to_string(&value)?;
    let err = normalize_scenario(&input, AuthoringFormat::Json).unwrap_err();
    assert!(matches!(err, AuthoringError::Schema { .. }));
    Ok(())
}

/// Confirms oversized authoring inputs are rejected before parsing.
#[test]
fn authoring_rejects_oversized_input() {
    let input = "a".repeat(MAX_AUTHORING_INPUT_BYTES + 1);
    let err = normalize_scenario(&input, AuthoringFormat::Json).unwrap_err();
    assert!(matches!(err, AuthoringError::InputTooLarge { .. }));
}

/// Confirms overly deep authoring inputs are rejected.
#[test]
fn authoring_rejects_overly_deep_inputs() {
    let mut input = String::new();
    for _ in 0 ..= MAX_AUTHORING_DEPTH {
        input.push('[');
    }
    input.push('0');
    for _ in 0 ..= MAX_AUTHORING_DEPTH {
        input.push(']');
    }
    let err = normalize_scenario(&input, AuthoringFormat::Json).unwrap_err();
    assert!(matches!(err, AuthoringError::DepthLimitExceeded { .. }));
}
