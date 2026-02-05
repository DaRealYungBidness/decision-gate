// crates/decision-gate-core/tests/decimal_precision_extended.rs
// ============================================================================
// Module: Decimal Precision Extended Tests
// Description: Additional numeric boundary and set/containment tests.
// Purpose: Harden comparator behavior for extreme numeric ranges.
// Threat Models: TM-COMP-002 (numeric precision attacks)
// ============================================================================

//! Extended numeric precision tests for comparator behavior.

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
    reason = "Test-only assertions and helpers are permitted."
)]

use decision_gate_core::Comparator;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::Value;
use serde_json::json;

fn eval_json(comparator: Comparator, expected: &Value, evidence: &Value) -> TriState {
    let evidence_result = EvidenceResult {
        value: Some(EvidenceValue::Json(evidence.clone())),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };

    evaluate_comparator(comparator, Some(expected), &evidence_result)
}

#[test]
fn decimal_precision_u64_vs_i64_bounds() {
    let max_unsigned = json!(u64::MAX);
    let max_signed = json!(i64::MAX);
    let result = eval_json(Comparator::GreaterThan, &max_signed, &max_unsigned);
    assert_eq!(result, TriState::True, "u64::MAX should be > i64::MAX");
}

#[test]
fn decimal_precision_negative_vs_positive_bounds() {
    let min_i64 = json!(i64::MIN);
    let zero = json!(0);
    let result = eval_json(Comparator::LessThan, &zero, &min_i64);
    assert_eq!(result, TriState::True, "i64::MIN should be < 0");
}

#[test]
fn decimal_precision_small_vs_zero_ordering() {
    let tiny = json!(1e-9);
    let zero = json!(0);
    let result = eval_json(Comparator::GreaterThan, &zero, &tiny);
    assert_eq!(result, TriState::True, "tiny positive should be > 0");
}

#[test]
fn decimal_precision_in_set_numeric_membership() {
    let value = json!(42);
    let expected = json!([1, 7, 42, 99]);
    let result = eval_json(Comparator::InSet, &expected, &value);
    assert_eq!(result, TriState::True, "value should be in numeric set");
}

#[test]
fn decimal_precision_in_set_numeric_absent() {
    let value = json!(41);
    let expected = json!([1, 7, 42, 99]);
    let result = eval_json(Comparator::InSet, &expected, &value);
    assert_eq!(result, TriState::False, "value should not be in numeric set");
}

#[test]
fn decimal_precision_contains_numeric_arrays() {
    let value = json!([1, 2, 3, 4]);
    let expected = json!([2, 4]);
    let result = eval_json(Comparator::Contains, &expected, &value);
    assert_eq!(result, TriState::True, "array should contain all expected numbers");
}

#[test]
fn decimal_precision_contains_numeric_arrays_missing() {
    let value = json!([1, 2, 3]);
    let expected = json!([2, 4]);
    let result = eval_json(Comparator::Contains, &expected, &value);
    assert_eq!(result, TriState::False, "array should not contain missing numbers");
}

#[test]
fn decimal_precision_lex_comparators_ignore_numbers() {
    let value = json!(10);
    let expected = json!(2);
    let result = eval_json(Comparator::LexGreaterThan, &expected, &value);
    assert_eq!(result, TriState::Unknown, "lex comparator should ignore numeric values");
}
