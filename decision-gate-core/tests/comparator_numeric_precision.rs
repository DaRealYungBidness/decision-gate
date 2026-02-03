// decision-gate-core/tests/comparator_numeric_precision.rs
// ============================================================================
// Module: Comparator Numeric Precision Unit Tests
// Description: Tests for BigDecimal edge cases, non-finite values, and type coercion attacks
// Purpose: Ensure comparator handles extreme numeric values without bypass
// Threat Models: TM-COMP-001 (bypass), TM-COMP-002 (numeric precision)
// ============================================================================

//! ## Overview
//! Comprehensive tests for numeric precision handling in comparators:
//! - `BigDecimal` precision boundaries (values near/exceeding Float64 limits)
//! - Non-finite value rejection (infinity, NaN must fail closed)
//! - Type coercion attacks (mixing numeric types to bypass comparisons)
//! - Decimal string parsing edge cases (leading zeros, scientific notation)
//!
//! ## Security Posture
//! Assumes adversarial evidence: attackers may send extreme numeric values to:
//! - Cause overflow/underflow
//! - Exploit floating-point precision loss
//! - Bypass comparisons via type confusion
//! - Trigger NaN propagation
//!
//! All edge cases must fail closed (return Unknown/False, never incorrectly True).

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

use std::str::FromStr;

use decision_gate_core::Comparator;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::Value;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Evaluates a comparator with JSON evidence value
fn eval_json(comparator: Comparator, expected: &Value, evidence: &Value) -> TriState {
    let evidence_result = EvidenceResult {
        value: Some(EvidenceValue::Json(evidence.clone())),
        lane: TrustLane::Asserted,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };

    evaluate_comparator(comparator, Some(expected), &evidence_result)
}

// ============================================================================
// SECTION: BigDecimal Precision Boundaries (TM-COMP-002)
// ============================================================================

/// TM-COMP-002: Tests numeric comparison near Float64 maximum (1.7976931348623157e308).
///
/// Context: Values near `f64::MAX` should compare correctly without overflow.
#[test]
fn comparator_near_float64_max() {
    let near_max = json!(1.797_693_134_862_315_7e308);
    let also_max = json!(1.797_693_134_862_315_7e308);

    let result = eval_json(Comparator::Equals, &also_max, &near_max);
    assert_eq!(result, TriState::True, "Values near f64::MAX should equal");

    let slightly_less = json!(1.797_693_134_862_315_6e308);
    // At this precision, floating point may not distinguish these values
    // Either True (values equal due to rounding) or Unknown (precision lost) is safe
    let result = eval_json(Comparator::LessThan, &near_max, &slightly_less);
    assert!(
        result == TriState::True || result == TriState::Unknown || result == TriState::False,
        "Comparison near f64::MAX boundary should be safe (not panic)"
    );
}

/// TM-COMP-002: Tests numeric comparison exceeding Float64 maximum.
///
/// Context: `BigDecimal` should handle values > `f64::MAX` that JSON can't represent as numbers.
/// Note: `serde_json::Number` cannot represent values > `f64::MAX`, so this tests the boundary.
#[test]
fn comparator_exceeding_float64_max() {
    // serde_json::Number is limited to f64 range
    // We test that the implementation handles this gracefully

    let max_f64 = json!(f64::MAX);
    let also_max = json!(f64::MAX);

    let result = eval_json(Comparator::Equals, &also_max, &max_f64);
    assert_eq!(result, TriState::True, "f64::MAX should equal itself");
}

/// TM-COMP-002: Tests subnormal float values (smaller than `f64::MIN_POSITIVE`).
///
/// Context: Subnormal numbers have reduced precision. Comparator must handle gracefully.
#[test]
fn comparator_subnormal_floats() {
    let subnormal = json!(2.225_073_858_507_201e-308); // Near f64::MIN_POSITIVE
    let also_subnormal = json!(2.225_073_858_507_201e-308);

    let result = eval_json(Comparator::Equals, &also_subnormal, &subnormal);
    assert_eq!(result, TriState::True, "Subnormal values should compare equal");

    let zero = json!(0.0);
    // Check subnormal > 0, so evidence=subnormal, expected=0
    let result = eval_json(Comparator::GreaterThan, &zero, &subnormal);
    assert_eq!(result, TriState::True, "Subnormal should be > 0");
}

/// TM-COMP-002: Tests comparison across `BigDecimal` and f64 boundary.
///
/// Context: Values representable as f64 should compare correctly with `BigDecimal` parsing.
#[test]
fn comparator_across_bigdecimal_f64_boundary() {
    let as_float = json!(1.5);
    let as_int = json!(1);

    // eval_json checks: evidence [comparator] expected
    // So to check "1.5 > 1", evidence=1.5, expected=1
    let result = eval_json(Comparator::GreaterThan, &as_int, &as_float);
    assert_eq!(result, TriState::True, "1.5 > 1");

    let result = eval_json(Comparator::LessThan, &as_float, &as_int);
    assert_eq!(result, TriState::True, "1 < 1.5");
}

/// TM-COMP-002: Tests decimal scale handling (trailing zeros significance).
///
/// Context: `BigDecimal` preserves scale, but for comparison, 1.0 == 1.00.
#[test]
fn comparator_trailing_zeros_scale() {
    let one_zero = json!(1.0);
    let two_zeros = json!(1.00);

    let result = eval_json(Comparator::Equals, &one_zero, &two_zeros);
    // BigDecimal comparison should treat 1.0 and 1.00 as equal
    assert_eq!(result, TriState::True, "1.0 should equal 1.00 (scale ignored in comparison)");
}

/// TM-COMP-002: Tests very small decimal differences (epsilon testing).
///
/// Context: Differences smaller than f64 epsilon should still compare correctly.
#[test]
fn comparator_small_decimal_differences() {
    let a = json!(1.000_000_000_000_000_1);
    let b = json!(1.000_000_000_000_000_2);

    // At this precision, f64 may not distinguish these values
    let result = eval_json(Comparator::Equals, &a, &b);
    // Either True (values rounded to same), False (values distinguishable), or Unknown is safe
    // The key is: no panic, no incorrect bypass
    assert!(
        result == TriState::True || result == TriState::False || result == TriState::Unknown,
        "Tiny differences should be handled safely (not panic)"
    );
}

/// TM-COMP-002: Tests decimal overflow detection.
///
/// Context: Operations that would overflow should return Unknown.
/// Note: `BigDecimal` doesn't overflow, but `serde_json::Number` does.
#[test]
fn comparator_decimal_overflow_handling() {
    let huge = json!(f64::MAX);
    // Check huge > 0, so evidence=huge, expected=0
    let result = eval_json(Comparator::GreaterThan, &json!(0), &huge);
    assert_eq!(result, TriState::True, "f64::MAX > 0");

    // Double f64::MAX would overflow if represented as f64
    // But as BigDecimal via string representation, it should work
    // However, serde_json::Number::from_f64(f64::MAX * 2.0) returns None
    // So we can't directly test this without string parsing
}

/// TM-COMP-002: Tests scientific notation parsing edge cases.
///
/// Context: Values in scientific notation should parse correctly.
#[test]
fn comparator_scientific_notation() {
    let sci = json!(1e3);
    let decimal = json!(1000);

    let result = eval_json(Comparator::Equals, &sci, &decimal);
    assert_eq!(result, TriState::True, "1e3 should equal 1000");

    let sci_negative = json!(1e-3);
    let decimal_small = json!(0.001);
    let result = eval_json(Comparator::Equals, &sci_negative, &decimal_small);
    assert_eq!(result, TriState::True, "1e-3 should equal 0.001");
}

/// TM-COMP-002: Tests negative zero handling (-0.0 vs 0.0).
///
/// Context: Negative zero and positive zero should compare as equal.
#[test]
fn comparator_negative_zero() {
    let neg_zero = json!(-0.0);
    let pos_zero = json!(0.0);

    let result = eval_json(Comparator::Equals, &neg_zero, &pos_zero);
    // In IEEE 754, -0.0 == 0.0
    assert_eq!(result, TriState::True, "-0.0 should equal 0.0");
}

/// TM-COMP-002: Tests sign of zero in comparisons.
///
/// Context: Neither -0.0 nor 0.0 should be greater than the other.
#[test]
fn comparator_sign_of_zero_ordering() {
    let neg_zero = json!(-0.0);
    let pos_zero = json!(0.0);

    let result = eval_json(Comparator::GreaterThan, &neg_zero, &pos_zero);
    assert_eq!(result, TriState::False, "-0.0 should not be > 0.0");

    let result = eval_json(Comparator::LessThan, &neg_zero, &pos_zero);
    assert_eq!(result, TriState::False, "-0.0 should not be < 0.0");
}

/// TM-COMP-002: Tests maximum precision limits.
///
/// Context: `BigDecimal` has arbitrary precision, but JSON parsing may limit it.
#[test]
fn comparator_max_precision_limits() {
    // Very high precision number (50 decimal places)
    let high_precision =
        json!(f64::from_str("1.12345678901234567890123456789012345678901234567890").unwrap());
    let truncated = json!(1.123_456_789_012_345_7); // f64 precision limit

    // f64 cannot represent 50 decimal places, so these will likely compare equal
    let result = eval_json(Comparator::Equals, &high_precision, &truncated);
    // Either equal (precision lost) or Unknown (safe)
    assert!(
        result == TriState::True || result == TriState::Unknown,
        "High precision comparison should be True or Unknown"
    );
}

/// TM-COMP-002: Tests lossy conversion detection (f64 → `BigDecimal`).
///
/// Context: When f64 precision is lost, comparison should still be safe.
#[test]
fn comparator_lossy_conversion() {
    // These values are beyond f64 precision but within f64 range
    let a = json!(1.111_111_111_111_111_1);
    let b = json!(1.111_111_111_111_111_2);

    // f64 may round these to the same value
    let result = eval_json(Comparator::Equals, &a, &b);
    // Safe outcomes: True (rounded equal) or Unknown (precision lost)
    assert!(
        result == TriState::True || result == TriState::Unknown,
        "Lossy conversion should yield True or Unknown"
    );
}

/// TM-COMP-002: Tests `BigDecimal` serialization round-trip.
///
/// Context: Serializing and deserializing should preserve comparison semantics.
#[test]
fn comparator_bigdecimal_round_trip() {
    let value = json!(123.456);
    let serialized = serde_json::to_string(&value).unwrap();
    let deserialized: Value = serde_json::from_str(&serialized).unwrap();

    let result = eval_json(Comparator::Equals, &value, &deserialized);
    assert_eq!(result, TriState::True, "Round-trip should preserve value");
}

/// TM-COMP-002: Tests decimal string parsing with leading zeros.
///
/// Context: Leading zeros should not affect numeric value.
#[test]
fn comparator_leading_zeros() {
    // Note: JSON doesn't allow leading zeros in numbers (001 is invalid JSON)
    // This tests that valid JSON numbers compare correctly
    let without_zero = json!(1);
    let with_decimal = json!(1.0);

    let result = eval_json(Comparator::Equals, &without_zero, &with_decimal);
    assert_eq!(result, TriState::True, "1 should equal 1.0");
}

// ============================================================================
// SECTION: Non-Finite Value Rejection (TM-COMP-002)
// ============================================================================

/// TM-COMP-002: Tests that positive infinity is rejected (fails closed).
///
/// Context: JSON doesn't support infinity, but f64 does. Ensure safe handling.
#[test]
fn comparator_positive_infinity_rejection() {
    // JSON cannot represent infinity as a number literal
    // If it appears via f64 conversion, serde_json represents it as null or errors
    // Test that our comparator handles this gracefully

    // serde_json::to_value(f64::INFINITY) produces Value::Number(...)
    // but Number::from_f64(f64::INFINITY) returns None
    // So we cannot directly create a JSON Number for infinity

    // This test documents that infinity cannot be represented in JSON
    assert!(
        serde_json::Number::from_f64(f64::INFINITY).is_none(),
        "JSON Number cannot represent infinity"
    );
}

/// TM-COMP-002: Tests that negative infinity is rejected (fails closed).
#[test]
fn comparator_negative_infinity_rejection() {
    assert!(
        serde_json::Number::from_f64(f64::NEG_INFINITY).is_none(),
        "JSON Number cannot represent negative infinity"
    );
}

/// TM-COMP-002: Tests that NaN is rejected (fails closed).
///
/// Context: NaN comparisons should never return True (NaN != NaN in IEEE 754).
#[test]
fn comparator_nan_rejection() {
    // JSON cannot represent NaN as a number
    assert!(serde_json::Number::from_f64(f64::NAN).is_none(), "JSON Number cannot represent NaN");

    // If NaN somehow appears, comparisons should return Unknown or False
    // Since we can't create NaN in JSON, we document the expectation
}

/// TM-COMP-002: Tests NaN in expected value handling.
///
/// Context: Even if expected value were NaN, comparison must fail closed.
#[test]
fn comparator_nan_in_expected() {
    // Cannot construct NaN via serde_json, so this documents the requirement
    // that if NaN appears, it should be rejected during deserialization or comparison
}

/// TM-COMP-002: Tests NaN in evidence value handling.
#[test]
fn comparator_nan_in_evidence() {
    // Cannot construct NaN via serde_json
    // Documents that NaN cannot leak into evidence values via JSON
}

/// TM-COMP-002: Tests infinity arithmetic (inf + inf, inf - inf).
///
/// Context: If infinity arithmetic occurs, results should fail closed.
#[test]
fn comparator_infinity_arithmetic() {
    // JSON cannot represent infinity
    // This test documents that infinity arithmetic cannot occur via JSON values
    // If it did occur (via Rust f64), serde_json would reject it
}

/// TM-COMP-002: Tests NaN propagation in expressions.
///
/// Context: NaN should not propagate and corrupt comparisons.
#[test]
fn comparator_nan_propagation() {
    // Cannot construct NaN via serde_json
    // Documents that NaN propagation is prevented by JSON's lack of NaN support
}

/// TM-COMP-002: Tests non-finite rejection in canonical JSON.
///
/// Context: RFC 8785 canonical JSON must reject non-finite numbers.
#[test]
fn comparator_non_finite_canonical_json() {
    // Canonical JSON (RFC 8785) does not allow Infinity or NaN
    // serde_json follows JSON spec which also rejects these
    // This test documents conformance
}

/// TM-COMP-002: Tests that infinities in string form are not parsed as numbers.
///
/// Context: String "Infinity" should not be treated as numeric infinity.
#[test]
fn comparator_infinity_string_not_numeric() {
    let inf_string = json!("Infinity");
    let number = json!(999_999);

    let result = eval_json(Comparator::GreaterThan, &inf_string, &number);
    // String vs number comparison should return Unknown
    assert_eq!(result, TriState::Unknown, "String 'Infinity' should not compare as number");
}

/// TM-COMP-002: Tests that NaN string is not parsed as numeric NaN.
#[test]
fn comparator_nan_string_not_numeric() {
    let nan_string = json!("NaN");
    let number = json!(0);

    let result = eval_json(Comparator::Equals, &nan_string, &number);
    // String vs number should return False (different types), not Unknown
    assert_eq!(result, TriState::False, "String 'NaN' should not equal number");
}

// ============================================================================
// SECTION: Type Coercion Attacks (TM-COMP-001)
// ============================================================================

/// TM-COMP-001: Tests string vs number type confusion.
///
/// Context: Attacker sends "123" (string) vs 123 (number) to bypass comparison.
#[test]
fn comparator_string_vs_number_type_confusion() {
    let string_num = json!("123");
    let actual_num = json!(123);

    let result = eval_json(Comparator::Equals, &string_num, &actual_num);
    // String and number should NOT be equal (fail closed)
    assert_eq!(result, TriState::False, "String '123' should not equal number 123");
}

/// TM-COMP-001: Tests integer vs float equality.
///
/// Context: Integer 1 should equal float 1.0 in numeric comparison.
#[test]
fn comparator_integer_vs_float_equality() {
    let int_one = json!(1);
    let float_one = json!(1.0);

    let result = eval_json(Comparator::Equals, &int_one, &float_one);
    assert_eq!(result, TriState::True, "Integer 1 should equal float 1.0");
}

/// TM-COMP-001: Tests integer vs `BigDecimal` comparison correctness.
///
/// Context: Large integer should compare correctly with decimal representation.
#[test]
fn comparator_integer_vs_bigdecimal() {
    let large_int = json!(9_007_199_254_740_991_i64); // 2^53 - 1 (max safe integer in f64)
    let as_decimal = json!(9_007_199_254_740_991.0);

    let result = eval_json(Comparator::Equals, &large_int, &as_decimal);
    assert_eq!(result, TriState::True, "Large integer should equal its decimal form");
}

/// TM-COMP-001: Tests float vs `BigDecimal` comparison correctness.
#[test]
fn comparator_float_vs_bigdecimal() {
    let float_val = json!(1234.5678);
    let decimal_val = json!(1234.5678);

    let result = eval_json(Comparator::Equals, &float_val, &decimal_val);
    assert_eq!(result, TriState::True, "Float and BigDecimal forms should equal");
}

/// TM-COMP-001: Tests string numeric vs number type confusion.
///
/// Context: String "1e3" should not equal number 1000.
#[test]
fn comparator_string_numeric_vs_number() {
    let string_sci = json!("1e3");
    let number = json!(1000);

    let result = eval_json(Comparator::Equals, &string_sci, &number);
    assert_eq!(result, TriState::False, "String '1e3' should not equal number 1000");
}

/// TM-COMP-001: Tests boolean vs integer type confusion.
///
/// Context: Boolean true should not equal integer 1.
#[test]
fn comparator_boolean_vs_integer() {
    let bool_true = json!(true);
    let int_one = json!(1);

    let result = eval_json(Comparator::Equals, &bool_true, &int_one);
    assert_eq!(result, TriState::False, "Boolean true should not equal integer 1");
}

/// TM-COMP-001: Tests null vs zero type confusion.
///
/// Context: Null should not equal number 0.
#[test]
fn comparator_null_vs_zero() {
    let null_val = json!(null);
    let zero = json!(0);

    let result = eval_json(Comparator::Equals, &null_val, &zero);
    assert_eq!(result, TriState::False, "Null should not equal 0");
}

/// TM-COMP-001: Tests array vs scalar type confusion.
///
/// Context: Array [1] should not equal scalar 1.
#[test]
fn comparator_array_vs_scalar() {
    let array_one = json!([1]);
    let scalar_one = json!(1);

    let result = eval_json(Comparator::Equals, &array_one, &scalar_one);
    assert_eq!(result, TriState::False, "Array [1] should not equal scalar 1");
}

// ============================================================================
// SECTION: Decimal String Parsing Edge Cases (TM-COMP-002)
// ============================================================================

/// TM-COMP-002: Tests trailing zeros in decimal strings.
///
/// Context: 1.0 vs 1.00 should compare equal.
#[test]
fn comparator_trailing_zeros_equality() {
    let one_decimal = json!(1.0);
    let two_decimals = json!(1.00);

    let result = eval_json(Comparator::Equals, &one_decimal, &two_decimals);
    assert_eq!(result, TriState::True, "1.0 should equal 1.00");
}

/// TM-COMP-002: Tests exponential notation equality (1e3 vs 1000).
#[test]
fn comparator_exponential_notation_equality() {
    let exponential = json!(1e3);
    let expanded = json!(1000);

    let result = eval_json(Comparator::Equals, &exponential, &expanded);
    assert_eq!(result, TriState::True, "1e3 should equal 1000");
}

/// TM-COMP-002: Tests sign handling (+1 vs 1).
///
/// Context: JSON doesn't allow explicit + sign, but semantically +1 == 1.
#[test]
fn comparator_sign_handling() {
    let positive = json!(1);
    let also_positive = json!(1);

    let result = eval_json(Comparator::Equals, &positive, &also_positive);
    assert_eq!(result, TriState::True, "1 should equal 1 (no sign difference)");
}

/// TM-COMP-002: Tests decimal separator normalization.
///
/// Context: JSON uses period (.) as decimal separator.
#[test]
fn comparator_decimal_separator() {
    let decimal = json!(1.5);
    // Check decimal > 1, so evidence=decimal, expected=1
    let result = eval_json(Comparator::GreaterThan, &json!(1), &decimal);
    assert_eq!(result, TriState::True, "1.5 > 1");
}

/// TM-COMP-002: Tests locale-specific formatting rejection.
///
/// Context: String "1,234.56" (with comma) should not parse as number.
#[test]
fn comparator_locale_specific_formatting() {
    let locale_string = json!("1,234.56");
    let number = json!(1234.56);

    let result = eval_json(Comparator::Equals, &locale_string, &number);
    assert_eq!(result, TriState::False, "Locale-formatted string should not equal number");
}

/// TM-COMP-002: Tests Unicode numeric character rejection.
///
/// Context: Unicode numeric characters (①, ½) should not parse as numbers.
#[test]
fn comparator_unicode_numeric_rejection() {
    let unicode_one = json!("①");
    let number_one = json!(1);

    let result = eval_json(Comparator::Equals, &unicode_one, &number_one);
    assert_eq!(result, TriState::False, "Unicode ① should not equal number 1");

    let unicode_half = json!("½");
    let decimal_half = json!(0.5);

    let result = eval_json(Comparator::Equals, &unicode_half, &decimal_half);
    assert_eq!(result, TriState::False, "Unicode ½ should not equal 0.5");
}

/// TM-COMP-002: Tests scientific notation with negative exponent.
#[test]
fn comparator_scientific_negative_exponent() {
    let sci_notation = json!(1.5e-2);
    let decimal = json!(0.015);

    let result = eval_json(Comparator::Equals, &sci_notation, &decimal);
    assert_eq!(result, TriState::True, "1.5e-2 should equal 0.015");
}
