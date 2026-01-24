// decision-gate-core/tests/comparator.rs
// ============================================================================
// Module: Comparator Evaluation Tests
// Description: Happy-path comparator evaluation tests.
// Purpose: Ensure comparators produce correct tri-state results.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================
//! ## Overview
//! Validates comparator behavior for JSON and byte evidence values.
//!
//! Security posture: Comparator logic must remain deterministic and fail closed.
//! Threat model: TM-COMP-001 - Comparator bypass via malformed inputs.

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
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::json;

const fn result_with_json(value: serde_json::Value) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Json(value)),
        lane: TrustLane::Verified,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    }
}

const fn result_with_bytes(bytes: Vec<u8>) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Bytes(bytes)),
        lane: TrustLane::Verified,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    }
}

// ============================================================================
// SECTION: Exists / NotExists
// ============================================================================

/// Verifies Exists and `NotExists` handle presence correctly.
#[test]
fn comparator_exists_and_not_exists() {
    let present = result_with_json(json!(true));
    let absent = EvidenceResult {
        value: None,
        lane: TrustLane::Verified,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };

    assert_eq!(evaluate_comparator(Comparator::Exists, None, &present), TriState::True);
    assert_eq!(evaluate_comparator(Comparator::Exists, None, &absent), TriState::False);
    assert_eq!(evaluate_comparator(Comparator::NotExists, None, &present), TriState::False);
    assert_eq!(evaluate_comparator(Comparator::NotExists, None, &absent), TriState::True);
}

// ============================================================================
// SECTION: JSON Comparators
// ============================================================================

/// Verifies equality and ordering comparators for JSON values.
#[test]
fn comparator_json_equality_and_ordering() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::Equals, Some(&json!(10)), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::NotEquals, Some(&json!(9)), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(5)), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(20)), &evidence),
        TriState::True
    );
}

/// Verifies contains and in-set comparators for JSON values.
#[test]
fn comparator_json_contains_and_in_set() {
    let haystack = result_with_json(json!("nation-state"));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("state")), &haystack),
        TriState::True
    );

    let array = result_with_json(json!(["a", "b", "c"]));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!(["b"])), &array),
        TriState::True
    );

    let value = result_with_json(json!("beta"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!(["alpha", "beta"])), &value),
        TriState::True
    );
}

// ============================================================================
// SECTION: Byte Comparators
// ============================================================================

/// Verifies byte-array equality comparator.
#[test]
fn comparator_bytes_equality() {
    let evidence = result_with_bytes(vec![1, 2, 3]);
    assert_eq!(
        evaluate_comparator(Comparator::Equals, Some(&json!([1, 2, 3])), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::NotEquals, Some(&json!([1, 2, 4])), &evidence),
        TriState::True
    );
}

// ============================================================================
// SECTION: GreaterThan Edge Cases
// ============================================================================

#[test]
fn comparator_greater_than_with_equal_values_returns_false() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(10)), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_greater_than_with_lesser_value_returns_true() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(5)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_greater_than_with_greater_value_returns_false() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(15)), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_greater_than_with_null_expected_returns_unknown() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(null)), &evidence),
        TriState::Unknown
    );
}

#[test]
fn comparator_greater_than_with_string_types_returns_unknown() {
    let evidence = result_with_json(json!("ten"));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(5)), &evidence),
        TriState::Unknown
    );
}

#[test]
fn comparator_greater_than_float_returns_unknown() {
    // Implementation uses integer-only semantics; floats return Unknown
    let evidence = result_with_json(json!(0.1 + 0.2));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(0.3)), &evidence),
        TriState::Unknown
    );
}

#[test]
fn comparator_greater_than_integer_i64_max() {
    let evidence = result_with_json(json!(i64::MAX));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(i64::MAX - 1)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_greater_than_integer_i64_min() {
    let evidence = result_with_json(json!(i64::MIN + 1));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(i64::MIN)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_greater_than_negative_numbers() {
    let evidence = result_with_json(json!(-5));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(-10)), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(-3)), &evidence),
        TriState::False
    );
}

// ============================================================================
// SECTION: LessThan Edge Cases
// ============================================================================

#[test]
fn comparator_less_than_with_equal_values_returns_false() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(10)), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_less_than_with_greater_value_returns_true() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(20)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_less_than_with_lesser_value_returns_false() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(5)), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_less_than_with_negative_numbers() {
    let evidence = result_with_json(json!(-10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(-5)), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(-15)), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_less_than_with_zero_boundary() {
    let evidence = result_with_json(json!(0));
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(1)), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(-1)), &evidence),
        TriState::False
    );
}

// ============================================================================
// SECTION: GreaterThanOrEqual Edge Cases
// ============================================================================

#[test]
fn comparator_gte_with_equal_returns_true() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThanOrEqual, Some(&json!(10)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_gte_with_lesser_returns_true() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThanOrEqual, Some(&json!(5)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_gte_with_greater_returns_false() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThanOrEqual, Some(&json!(15)), &evidence),
        TriState::False
    );
}

// ============================================================================
// SECTION: LessThanOrEqual Edge Cases
// ============================================================================

#[test]
fn comparator_lte_with_equal_returns_true() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThanOrEqual, Some(&json!(10)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_lte_with_greater_returns_true() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThanOrEqual, Some(&json!(15)), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_lte_with_lesser_returns_false() {
    let evidence = result_with_json(json!(10));
    assert_eq!(
        evaluate_comparator(Comparator::LessThanOrEqual, Some(&json!(5)), &evidence),
        TriState::False
    );
}

// ============================================================================
// SECTION: Contains Edge Cases
// ============================================================================

#[test]
fn comparator_contains_empty_string_in_any_string_returns_true() {
    let evidence = result_with_json(json!("hello world"));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("")), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_contains_substring_not_found_returns_false() {
    let evidence = result_with_json(json!("hello world"));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("xyz")), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_contains_case_sensitivity_preserved() {
    let evidence = result_with_json(json!("Hello World"));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("Hello")), &evidence),
        TriState::True
    );
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("hello")), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_contains_unicode_characters() {
    let evidence = result_with_json(json!("hello 世界"));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("世界")), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_contains_array_element_present() {
    let evidence = result_with_json(json!(["a", "b", "c"]));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!(["b"])), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_contains_array_element_absent() {
    let evidence = result_with_json(json!(["a", "b", "c"]));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!(["d"])), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_contains_array_multiple_elements() {
    let evidence = result_with_json(json!(["a", "b", "c", "d"]));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!(["b", "c"])), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_contains_array_partial_match_fails() {
    let evidence = result_with_json(json!(["a", "b"]));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!(["b", "c"])), &evidence),
        TriState::False
    );
}

// ============================================================================
// SECTION: InSet Edge Cases
// ============================================================================

#[test]
fn comparator_in_set_empty_set_always_false() {
    let evidence = result_with_json(json!("value"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!([])), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_in_set_single_element_match() {
    let evidence = result_with_json(json!("alpha"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!(["alpha"])), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_in_set_single_element_no_match() {
    let evidence = result_with_json(json!("beta"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!(["alpha"])), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_in_set_type_coercion_rejected_int_vs_string() {
    // JSON "1" (string) should not match 1 (number)
    let evidence = result_with_json(json!("1"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!([1, 2, 3])), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_in_set_null_in_set_of_nulls() {
    let evidence = result_with_json(json!(null));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!([null])), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_in_set_multiple_element_match() {
    let evidence = result_with_json(json!("beta"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!(["alpha", "beta", "gamma"])), &evidence),
        TriState::True
    );
}

#[test]
fn comparator_in_set_number_in_number_set() {
    let evidence = result_with_json(json!(42));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!([1, 42, 100])), &evidence),
        TriState::True
    );
}

// ============================================================================
// SECTION: Type Mismatch Handling
// ============================================================================

#[test]
fn comparator_equals_json_object_vs_string_returns_false() {
    let evidence = result_with_json(json!({"key": "value"}));
    assert_eq!(
        evaluate_comparator(Comparator::Equals, Some(&json!("string")), &evidence),
        TriState::False
    );
}

#[test]
fn comparator_numeric_comparator_on_string_returns_unknown() {
    let evidence = result_with_json(json!("not a number"));
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(5)), &evidence),
        TriState::Unknown
    );
    assert_eq!(
        evaluate_comparator(Comparator::LessThan, Some(&json!(5)), &evidence),
        TriState::Unknown
    );
}

#[test]
fn comparator_contains_on_number_returns_unknown() {
    let evidence = result_with_json(json!(12345));
    assert_eq!(
        evaluate_comparator(Comparator::Contains, Some(&json!("123")), &evidence),
        TriState::Unknown
    );
}

#[test]
fn comparator_in_set_with_non_array_expected_returns_unknown() {
    let evidence = result_with_json(json!("value"));
    assert_eq!(
        evaluate_comparator(Comparator::InSet, Some(&json!("not an array")), &evidence),
        TriState::Unknown
    );
}

// ============================================================================
// SECTION: Missing Evidence Value
// ============================================================================

#[test]
fn comparator_equals_with_missing_value_returns_unknown() {
    let evidence = EvidenceResult {
        value: None,
        lane: TrustLane::Verified,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };
    assert_eq!(
        evaluate_comparator(Comparator::Equals, Some(&json!(true)), &evidence),
        TriState::Unknown
    );
}

#[test]
fn comparator_greater_than_with_missing_value_returns_unknown() {
    let evidence = EvidenceResult {
        value: None,
        lane: TrustLane::Verified,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };
    assert_eq!(
        evaluate_comparator(Comparator::GreaterThan, Some(&json!(5)), &evidence),
        TriState::Unknown
    );
}

// ============================================================================
// SECTION: Missing Expected Value
// ============================================================================

#[test]
fn comparator_equals_with_none_expected_returns_unknown() {
    let evidence = result_with_json(json!(10));
    assert_eq!(evaluate_comparator(Comparator::Equals, None, &evidence), TriState::Unknown);
}

#[test]
fn comparator_greater_than_with_none_expected_returns_unknown() {
    let evidence = result_with_json(json!(10));
    assert_eq!(evaluate_comparator(Comparator::GreaterThan, None, &evidence), TriState::Unknown);
}
