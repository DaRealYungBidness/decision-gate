// decision-gate-core/tests/adversarial_inputs.rs
// ============================================================================
// Module: Adversarial Input Tests
// Description: Ensures comparators fail closed on malformed evidence.
// ============================================================================
//! ## Overview
//! Validates that comparator evaluation returns Unknown on adversarial inputs.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

use decision_gate_core::Comparator;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::json;

/// Builds an evidence result with the provided value.
const fn empty_result_with_value(value: EvidenceValue) -> EvidenceResult {
    EvidenceResult {
        value: Some(value),
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    }
}

#[test]
/// Ensures non-numeric comparisons return Unknown.
fn comparator_returns_unknown_on_non_numeric_input() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(5)),
        &empty_result_with_value(EvidenceValue::Json(json!("not-a-number"))),
    );
    assert_eq!(result, TriState::Unknown);
}

#[test]
/// Ensures out-of-range byte arrays are rejected.
fn comparator_rejects_out_of_range_byte_arrays() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!([999])),
        &empty_result_with_value(EvidenceValue::Bytes(vec![1, 2, 3])),
    );
    assert_eq!(result, TriState::Unknown);
}

#[test]
/// Ensures contains comparisons fail on mismatched types.
fn comparator_rejects_mismatched_contains_types() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("needle")),
        &empty_result_with_value(EvidenceValue::Json(json!(42))),
    );
    assert_eq!(result, TriState::Unknown);
}

#[test]
/// Ensures missing evidence values return Unknown.
fn comparator_returns_unknown_when_missing_value() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!(true)),
        &EvidenceResult {
            value: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    assert_eq!(result, TriState::Unknown);
}

#[test]
/// Ensures decimal comparisons return Unknown.
fn comparator_rejects_decimal_numbers() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(1.5)),
        &empty_result_with_value(EvidenceValue::Json(json!(2.0))),
    );
    assert_eq!(result, TriState::Unknown);
}
