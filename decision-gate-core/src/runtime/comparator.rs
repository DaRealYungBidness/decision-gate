// decision-gate-core/src/runtime/comparator.rs
// ============================================================================
// Module: Decision Gate Comparator Logic
// Description: Comparator evaluation for evidence predicates.
// Purpose: Convert evidence values into tri-state predicate outcomes.
// Dependencies: crate::core, ret-logic
// ============================================================================

//! ## Overview
//! Comparator evaluation converts evidence results into tri-state outcomes.
//! Missing or invalid evidence yields `Unknown` to preserve fail-closed
//! behavior. Numeric ordering is integer-only; decimal values return `Unknown`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use ret_logic::TriState;
use serde_json::Number;
use serde_json::Value;

use crate::core::Comparator;
use crate::core::EvidenceResult;
use crate::core::EvidenceValue;

// ============================================================================
// SECTION: Comparator Evaluation
// ============================================================================

/// Evaluates a comparator against an evidence result.
#[must_use]
pub fn evaluate_comparator(
    comparator: Comparator,
    expected: Option<&Value>,
    evidence: &EvidenceResult,
) -> TriState {
    match comparator {
        Comparator::Exists => {
            if evidence.value.is_some() {
                TriState::True
            } else {
                TriState::False
            }
        }
        Comparator::NotExists => {
            if evidence.value.is_some() {
                TriState::False
            } else {
                TriState::True
            }
        }
        _ => evaluate_value_comparator(comparator, expected, evidence),
    }
}

/// Evaluates comparators against evidence values.
fn evaluate_value_comparator(
    comparator: Comparator,
    expected: Option<&Value>,
    evidence: &EvidenceResult,
) -> TriState {
    let Some(value) = &evidence.value else {
        return TriState::Unknown;
    };

    match value {
        EvidenceValue::Json(json) => evaluate_json_comparator(comparator, expected, json),
        EvidenceValue::Bytes(bytes) => evaluate_bytes_comparator(comparator, expected, bytes),
    }
}

/// Evaluates JSON comparators against a JSON value.
fn evaluate_json_comparator(
    comparator: Comparator,
    expected: Option<&Value>,
    evidence: &Value,
) -> TriState {
    let Some(expected) = expected else {
        return TriState::Unknown;
    };

    match comparator {
        Comparator::Equals => TriState::from(evidence == expected),
        Comparator::NotEquals => TriState::from(evidence != expected),
        Comparator::GreaterThan
        | Comparator::GreaterThanOrEqual
        | Comparator::LessThan
        | Comparator::LessThanOrEqual => compare_numbers(comparator, evidence, expected),
        Comparator::Contains => compare_contains(evidence, expected),
        Comparator::InSet => compare_in_set(evidence, expected),
        Comparator::Exists | Comparator::NotExists => TriState::Unknown,
    }
}

/// Evaluates byte-array comparators against evidence bytes.
fn evaluate_bytes_comparator(
    comparator: Comparator,
    expected: Option<&Value>,
    bytes: &[u8],
) -> TriState {
    let Some(expected) = expected else {
        return TriState::Unknown;
    };

    let expected_bytes = match expected {
        Value::Array(values) => {
            let mut out = Vec::with_capacity(values.len());
            for value in values {
                if let Some(byte) = value.as_u64() {
                    if byte > u64::from(u8::MAX) {
                        return TriState::Unknown;
                    }
                    let Ok(byte) = u8::try_from(byte) else {
                        return TriState::Unknown;
                    };
                    out.push(byte);
                } else {
                    return TriState::Unknown;
                }
            }
            out
        }
        _ => return TriState::Unknown,
    };

    match comparator {
        Comparator::Equals => TriState::from(bytes == expected_bytes.as_slice()),
        Comparator::NotEquals => TriState::from(bytes != expected_bytes.as_slice()),
        _ => TriState::Unknown,
    }
}

/// Compares numeric JSON values using the comparator.
fn compare_numbers(comparator: Comparator, left: &Value, right: &Value) -> TriState {
    let Some(left_num) = left.as_number() else {
        return TriState::Unknown;
    };
    let Some(right_num) = right.as_number() else {
        return TriState::Unknown;
    };

    let Some(ordering) = numeric_cmp(left_num, right_num) else {
        return TriState::Unknown;
    };

    let result = match comparator {
        Comparator::GreaterThan => ordering.is_gt(),
        Comparator::GreaterThanOrEqual => ordering.is_ge(),
        Comparator::LessThan => ordering.is_lt(),
        Comparator::LessThanOrEqual => ordering.is_le(),
        _ => return TriState::Unknown,
    };

    TriState::from(result)
}

/// Evaluates containment semantics for JSON values.
fn compare_contains(left: &Value, right: &Value) -> TriState {
    match (left, right) {
        (Value::String(haystack), Value::String(needle)) => {
            TriState::from(haystack.contains(needle))
        }
        (Value::Array(haystack), Value::Array(needle)) => {
            let contains_all = needle.iter().all(|item| haystack.contains(item));
            TriState::from(contains_all)
        }
        _ => TriState::Unknown,
    }
}

/// Evaluates set membership for JSON values.
fn compare_in_set(value: &Value, expected: &Value) -> TriState {
    match expected {
        Value::Array(values) => TriState::from(values.contains(value)),
        _ => TriState::Unknown,
    }
}

/// Compares two JSON numbers using integer-only semantics.
fn numeric_cmp(left: &Number, right: &Number) -> Option<std::cmp::Ordering> {
    let left = integer_value(left)?;
    let right = integer_value(right)?;

    match (left, right) {
        (IntegerValue::Signed(left), IntegerValue::Signed(right)) => Some(left.cmp(&right)),
        (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => Some(left.cmp(&right)),
        (IntegerValue::Signed(left), IntegerValue::Unsigned(right)) => {
            if left < 0 {
                Some(std::cmp::Ordering::Less)
            } else {
                let left = u64::try_from(left).ok()?;
                Some(left.cmp(&right))
            }
        }
        (IntegerValue::Unsigned(left), IntegerValue::Signed(right)) => {
            if right < 0 {
                Some(std::cmp::Ordering::Greater)
            } else {
                let right = u64::try_from(right).ok()?;
                Some(left.cmp(&right))
            }
        }
    }
}

/// Integer representation of JSON numbers for deterministic comparison.
enum IntegerValue {
    Signed(i64),
    Unsigned(u64),
}

/// Extracts integer values and rejects decimals.
fn integer_value(value: &Number) -> Option<IntegerValue> {
    if let Some(value) = value.as_i64() {
        return Some(IntegerValue::Signed(value));
    }
    value.as_u64().map(IntegerValue::Unsigned)
}
