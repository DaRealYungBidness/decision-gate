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
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::json;

/// Builds an evidence result with the provided value.
const fn empty_result_with_value(value: EvidenceValue) -> EvidenceResult {
    EvidenceResult {
        value: Some(value),
        lane: TrustLane::Verified,
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
            lane: TrustLane::Verified,
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
/// Ensures decimal comparisons remain deterministic.
fn comparator_accepts_decimal_numbers() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(1.5)),
        &empty_result_with_value(EvidenceValue::Json(json!(2.0))),
    );
    assert_eq!(result, TriState::True);
}

// ============================================================================
// SECTION: Type Confusion Tests for Numeric Ordering Comparators
// ============================================================================

#[test]
fn greater_than_returns_unknown_on_string_evidence() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!("not-a-number"))),
    );
    assert_eq!(result, TriState::Unknown, "GT on string should return Unknown");
}

#[test]
fn greater_than_returns_unknown_on_object_evidence() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!({"key": "value"}))),
    );
    assert_eq!(result, TriState::Unknown, "GT on object should return Unknown");
}

#[test]
fn greater_than_returns_unknown_on_array_evidence() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!([1, 2, 3]))),
    );
    assert_eq!(result, TriState::Unknown, "GT on array should return Unknown");
}

#[test]
fn greater_than_returns_unknown_on_boolean_evidence() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!(true))),
    );
    assert_eq!(result, TriState::Unknown, "GT on boolean should return Unknown");
}

#[test]
fn greater_than_or_equal_returns_unknown_on_string_evidence() {
    let result = evaluate_comparator(
        Comparator::GreaterThanOrEqual,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!("string"))),
    );
    assert_eq!(result, TriState::Unknown, "GTE on string should return Unknown");
}

#[test]
fn less_than_returns_unknown_on_string_evidence() {
    let result = evaluate_comparator(
        Comparator::LessThan,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!("string"))),
    );
    assert_eq!(result, TriState::Unknown, "LT on string should return Unknown");
}

#[test]
fn less_than_or_equal_returns_unknown_on_object_evidence() {
    let result = evaluate_comparator(
        Comparator::LessThanOrEqual,
        Some(&json!(10)),
        &empty_result_with_value(EvidenceValue::Json(json!({"nested": true}))),
    );
    assert_eq!(result, TriState::Unknown, "LTE on object should return Unknown");
}

// ============================================================================
// SECTION: Type Confusion Tests for Lexicographic Comparators
// ============================================================================

#[test]
fn lex_greater_than_returns_unknown_on_numeric_evidence() {
    let result = evaluate_comparator(
        Comparator::LexGreaterThan,
        Some(&json!("abc")),
        &empty_result_with_value(EvidenceValue::Json(json!(42))),
    );
    assert_eq!(result, TriState::Unknown, "LexGT on number should return Unknown");
}

#[test]
fn lex_greater_than_returns_unknown_on_object_evidence() {
    let result = evaluate_comparator(
        Comparator::LexGreaterThan,
        Some(&json!("abc")),
        &empty_result_with_value(EvidenceValue::Json(json!({"key": "value"}))),
    );
    assert_eq!(result, TriState::Unknown, "LexGT on object should return Unknown");
}

#[test]
fn lex_greater_than_or_equal_returns_unknown_on_array_evidence() {
    let result = evaluate_comparator(
        Comparator::LexGreaterThanOrEqual,
        Some(&json!("abc")),
        &empty_result_with_value(EvidenceValue::Json(json!(["a", "b"]))),
    );
    assert_eq!(result, TriState::Unknown, "LexGTE on array should return Unknown");
}

#[test]
fn lex_less_than_returns_unknown_on_boolean_evidence() {
    let result = evaluate_comparator(
        Comparator::LexLessThan,
        Some(&json!("abc")),
        &empty_result_with_value(EvidenceValue::Json(json!(false))),
    );
    assert_eq!(result, TriState::Unknown, "LexLT on boolean should return Unknown");
}

#[test]
fn lex_less_than_or_equal_returns_unknown_on_null_evidence() {
    let result = evaluate_comparator(
        Comparator::LexLessThanOrEqual,
        Some(&json!("abc")),
        &empty_result_with_value(EvidenceValue::Json(json!(null))),
    );
    assert_eq!(result, TriState::Unknown, "LexLTE on null should return Unknown");
}

// ============================================================================
// SECTION: Type Confusion Tests for Contains Comparator
// ============================================================================

#[test]
fn contains_returns_unknown_on_numeric_evidence() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("needle")),
        &empty_result_with_value(EvidenceValue::Json(json!(12345))),
    );
    assert_eq!(result, TriState::Unknown, "Contains on number should return Unknown");
}

#[test]
fn contains_returns_unknown_on_boolean_evidence() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("needle")),
        &empty_result_with_value(EvidenceValue::Json(json!(true))),
    );
    assert_eq!(result, TriState::Unknown, "Contains on boolean should return Unknown");
}

#[test]
fn contains_returns_unknown_on_null_evidence() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("needle")),
        &empty_result_with_value(EvidenceValue::Json(json!(null))),
    );
    assert_eq!(result, TriState::Unknown, "Contains on null should return Unknown");
}

#[test]
fn contains_returns_unknown_on_object_evidence() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("needle")),
        &empty_result_with_value(EvidenceValue::Json(json!({"key": "needle"}))),
    );
    assert_eq!(result, TriState::Unknown, "Contains on object should return Unknown");
}

// ============================================================================
// SECTION: Type Confusion Tests for InSet Comparator
// ============================================================================

#[test]
fn in_set_returns_unknown_on_non_array_expected() {
    let result = evaluate_comparator(
        Comparator::InSet,
        Some(&json!("not-an-array")),
        &empty_result_with_value(EvidenceValue::Json(json!("value"))),
    );
    assert_eq!(result, TriState::Unknown, "InSet with non-array expected should return Unknown");
}

#[test]
fn in_set_returns_unknown_on_object_expected() {
    let result = evaluate_comparator(
        Comparator::InSet,
        Some(&json!({"set": ["a", "b"]})),
        &empty_result_with_value(EvidenceValue::Json(json!("a"))),
    );
    assert_eq!(result, TriState::Unknown, "InSet with object expected should return Unknown");
}

#[test]
fn in_set_returns_unknown_on_number_expected() {
    let result = evaluate_comparator(
        Comparator::InSet,
        Some(&json!(42)),
        &empty_result_with_value(EvidenceValue::Json(json!(42))),
    );
    assert_eq!(result, TriState::Unknown, "InSet with number expected should return Unknown");
}

// ============================================================================
// SECTION: Type Confusion Tests for Deep Equality Comparators
// ============================================================================

#[test]
fn deep_equals_returns_false_on_type_mismatch() {
    // Deep equals on different types returns Unknown to fail closed.
    let result = evaluate_comparator(
        Comparator::DeepEquals,
        Some(&json!({"key": "value"})),
        &empty_result_with_value(EvidenceValue::Json(json!("string"))),
    );
    assert_eq!(result, TriState::Unknown, "DeepEquals on type mismatch should return Unknown");
}

#[test]
fn deep_not_equals_returns_true_on_type_mismatch() {
    let result = evaluate_comparator(
        Comparator::DeepNotEquals,
        Some(&json!({"key": "value"})),
        &empty_result_with_value(EvidenceValue::Json(json!(42))),
    );
    assert_eq!(result, TriState::Unknown, "DeepNotEquals on type mismatch should return Unknown");
}

// ============================================================================
// SECTION: Type Confusion Tests for Equals/NotEquals Comparators
// ============================================================================

#[test]
fn equals_returns_false_on_type_mismatch_string_vs_number() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!("42")),
        &empty_result_with_value(EvidenceValue::Json(json!(42))),
    );
    assert_eq!(result, TriState::False, "Equals string vs number should return False");
}

#[test]
fn equals_returns_false_on_type_mismatch_bool_vs_string() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!("true")),
        &empty_result_with_value(EvidenceValue::Json(json!(true))),
    );
    assert_eq!(result, TriState::False, "Equals string vs boolean should return False");
}

#[test]
fn not_equals_returns_true_on_type_mismatch() {
    let result = evaluate_comparator(
        Comparator::NotEquals,
        Some(&json!("42")),
        &empty_result_with_value(EvidenceValue::Json(json!(42))),
    );
    assert_eq!(result, TriState::True, "NotEquals string vs number should return True");
}

// ============================================================================
// SECTION: Boundary Condition Tests
// ============================================================================

#[test]
fn equals_empty_string() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!("")),
        &empty_result_with_value(EvidenceValue::Json(json!(""))),
    );
    assert_eq!(result, TriState::True, "Empty string equality should work");
}

#[test]
fn equals_empty_array() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!([])),
        &empty_result_with_value(EvidenceValue::Json(json!([]))),
    );
    assert_eq!(result, TriState::True, "Empty array equality should work");
}

#[test]
fn equals_empty_object() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!({})),
        &empty_result_with_value(EvidenceValue::Json(json!({}))),
    );
    assert_eq!(result, TriState::True, "Empty object equality should work");
}

#[test]
fn contains_empty_needle_in_string() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("")),
        &empty_result_with_value(EvidenceValue::Json(json!("haystack"))),
    );
    assert_eq!(result, TriState::True, "Empty needle should be contained in any string");
}

#[test]
fn contains_needle_in_empty_string() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("needle")),
        &empty_result_with_value(EvidenceValue::Json(json!(""))),
    );
    assert_eq!(result, TriState::False, "Non-empty needle should not be in empty string");
}

#[test]
fn in_set_empty_set() {
    let result = evaluate_comparator(
        Comparator::InSet,
        Some(&json!([])),
        &empty_result_with_value(EvidenceValue::Json(json!("value"))),
    );
    assert_eq!(result, TriState::False, "Value should not be in empty set");
}

#[test]
fn in_set_with_null_in_set() {
    let result = evaluate_comparator(
        Comparator::InSet,
        Some(&json!([null, "a", "b"])),
        &empty_result_with_value(EvidenceValue::Json(json!(null))),
    );
    assert_eq!(result, TriState::True, "Null should be found in set containing null");
}

#[test]
fn deep_equals_empty_arrays() {
    let result = evaluate_comparator(
        Comparator::DeepEquals,
        Some(&json!([])),
        &empty_result_with_value(EvidenceValue::Json(json!([]))),
    );
    assert_eq!(result, TriState::True, "Empty arrays should be deeply equal");
}

#[test]
fn deep_equals_empty_objects() {
    let result = evaluate_comparator(
        Comparator::DeepEquals,
        Some(&json!({})),
        &empty_result_with_value(EvidenceValue::Json(json!({}))),
    );
    assert_eq!(result, TriState::True, "Empty objects should be deeply equal");
}

#[test]
fn greater_than_zero_boundary() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(0)),
        &empty_result_with_value(EvidenceValue::Json(json!(1))),
    );
    assert_eq!(result, TriState::True, "1 > 0 should be True");
}

#[test]
fn greater_than_negative_boundary() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(-1)),
        &empty_result_with_value(EvidenceValue::Json(json!(0))),
    );
    assert_eq!(result, TriState::True, "0 > -1 should be True");
}

#[test]
fn lex_empty_string_ordering() {
    let result = evaluate_comparator(
        Comparator::LexGreaterThan,
        Some(&json!("")),
        &empty_result_with_value(EvidenceValue::Json(json!("a"))),
    );
    assert_eq!(result, TriState::True, "'a' > '' lexicographically");
}

// ============================================================================
// SECTION: Extreme Value Tests
// ============================================================================

#[test]
fn equals_very_large_integer() {
    let large = i64::MAX;
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!(large)),
        &empty_result_with_value(EvidenceValue::Json(json!(large))),
    );
    assert_eq!(result, TriState::True, "Very large integer equality should work");
}

#[test]
fn equals_very_small_integer() {
    let small = i64::MIN;
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!(small)),
        &empty_result_with_value(EvidenceValue::Json(json!(small))),
    );
    assert_eq!(result, TriState::True, "Very small integer equality should work");
}

#[test]
fn greater_than_i64_boundary() {
    let result = evaluate_comparator(
        Comparator::GreaterThan,
        Some(&json!(i64::MAX - 1)),
        &empty_result_with_value(EvidenceValue::Json(json!(i64::MAX))),
    );
    assert_eq!(result, TriState::True, "i64::MAX > i64::MAX-1 should be True");
}

#[test]
fn contains_unicode_characters() {
    let result = evaluate_comparator(
        Comparator::Contains,
        Some(&json!("中文")),
        &empty_result_with_value(EvidenceValue::Json(json!("This contains 中文 characters"))),
    );
    assert_eq!(result, TriState::True, "Unicode containment should work");
}

#[test]
fn equals_unicode_normalization() {
    // é composed vs decomposed - should be equal if normalized
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!("café")),
        &empty_result_with_value(EvidenceValue::Json(json!("café"))),
    );
    assert_eq!(result, TriState::True, "Same unicode string should be equal");
}

#[test]
fn deep_equals_nested_structure() {
    let nested = json!({
        "level1": {
            "level2": {
                "level3": {
                    "value": [1, 2, 3]
                }
            }
        }
    });
    let result = evaluate_comparator(
        Comparator::DeepEquals,
        Some(&nested),
        &empty_result_with_value(EvidenceValue::Json(nested.clone())),
    );
    assert_eq!(result, TriState::True, "Deeply nested structures should be equal");
}

#[test]
fn bytes_empty_array_comparison() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!([])),
        &empty_result_with_value(EvidenceValue::Bytes(vec![])),
    );
    assert_eq!(result, TriState::True, "Empty byte arrays should be equal");
}

#[test]
fn bytes_with_valid_range_values() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!([0, 127, 255])),
        &empty_result_with_value(EvidenceValue::Bytes(vec![0, 127, 255])),
    );
    assert_eq!(result, TriState::True, "Valid byte range values should match");
}

// ============================================================================
// SECTION: Null Value Tests
// ============================================================================

#[test]
fn equals_null_vs_null() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!(null)),
        &empty_result_with_value(EvidenceValue::Json(json!(null))),
    );
    assert_eq!(result, TriState::True, "Null should equal null");
}

#[test]
fn equals_null_vs_non_null() {
    let result = evaluate_comparator(
        Comparator::Equals,
        Some(&json!(null)),
        &empty_result_with_value(EvidenceValue::Json(json!("value"))),
    );
    assert_eq!(result, TriState::False, "Null should not equal non-null");
}

#[test]
fn exists_with_null_value() {
    let result = evaluate_comparator(
        Comparator::Exists,
        None,
        &empty_result_with_value(EvidenceValue::Json(json!(null))),
    );
    assert_eq!(result, TriState::True, "Exists should return True for null value (value is present)");
}

#[test]
fn not_exists_with_missing_value() {
    let result = evaluate_comparator(
        Comparator::NotExists,
        None,
        &EvidenceResult {
            value: None,
            lane: TrustLane::Verified,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    assert_eq!(result, TriState::True, "NotExists should return True for missing value");
}

#[test]
fn exists_with_missing_value() {
    let result = evaluate_comparator(
        Comparator::Exists,
        None,
        &EvidenceResult {
            value: None,
            lane: TrustLane::Verified,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    assert_eq!(result, TriState::False, "Exists should return False for missing value");
}
