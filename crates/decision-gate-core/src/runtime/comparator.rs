// crates/decision-gate-core/src/runtime/comparator.rs
// ============================================================================
// Module: Decision Gate Comparator Logic
// Description: Comparator evaluation for evidence conditions.
// Purpose: Convert evidence values into tri-state condition outcomes.
// Dependencies: crate::core, ret-logic
// ============================================================================

//! ## Overview
//! Comparator evaluation converts evidence results into tri-state outcomes.
//! Missing or invalid evidence yields `Unknown` to preserve fail-closed
//! behavior. Numeric ordering is decimal-aware and deterministic.
//!
//! Security posture: evidence values are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::cmp::Ordering;
use std::str::FromStr;

use bigdecimal::BigDecimal;
use ret_logic::TriState;
use serde_json::Number;
use serde_json::Value;
use time::Date;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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
        Comparator::Equals => compare_equals(evidence, expected),
        Comparator::NotEquals => compare_not_equals(evidence, expected),
        Comparator::GreaterThan
        | Comparator::GreaterThanOrEqual
        | Comparator::LessThan
        | Comparator::LessThanOrEqual => compare_ordering(comparator, evidence, expected),
        Comparator::LexGreaterThan
        | Comparator::LexGreaterThanOrEqual
        | Comparator::LexLessThan
        | Comparator::LexLessThanOrEqual => compare_lexicographic(comparator, evidence, expected),
        Comparator::Contains => compare_contains(evidence, expected),
        Comparator::InSet => compare_in_set(evidence, expected),
        Comparator::DeepEquals => compare_deep_equals(evidence, expected),
        Comparator::DeepNotEquals => compare_deep_not_equals(evidence, expected),
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

/// Compares JSON values for equality, with decimal-aware numeric handling.
fn compare_equals(left: &Value, right: &Value) -> TriState {
    match (left, right) {
        (Value::Number(left_num), Value::Number(right_num)) => {
            compare_decimal_equality(left_num, right_num, true)
        }
        _ => TriState::from(left == right),
    }
}

/// Compares JSON values for inequality, with decimal-aware numeric handling.
fn compare_not_equals(left: &Value, right: &Value) -> TriState {
    match (left, right) {
        (Value::Number(left_num), Value::Number(right_num)) => {
            compare_decimal_equality(left_num, right_num, false)
        }
        _ => TriState::from(left != right),
    }
}

/// Compares numeric or temporal JSON values using ordering comparators.
fn compare_ordering(comparator: Comparator, left: &Value, right: &Value) -> TriState {
    if let (Some(left_num), Some(right_num)) = (left.as_number(), right.as_number()) {
        if let Some(ordering) = decimal_cmp(left_num, right_num) {
            let result = match comparator {
                Comparator::GreaterThan => ordering.is_gt(),
                Comparator::GreaterThanOrEqual => ordering.is_ge(),
                Comparator::LessThan => ordering.is_lt(),
                Comparator::LessThanOrEqual => ordering.is_le(),
                _ => return TriState::Unknown,
            };
            return TriState::from(result);
        }
        return TriState::Unknown;
    }

    if let (Value::String(left), Value::String(right)) = (left, right)
        && let Some(ordering) = temporal_cmp(left, right)
    {
        let result = match comparator {
            Comparator::GreaterThan => ordering.is_gt(),
            Comparator::GreaterThanOrEqual => ordering.is_ge(),
            Comparator::LessThan => ordering.is_lt(),
            Comparator::LessThanOrEqual => ordering.is_le(),
            _ => return TriState::Unknown,
        };
        return TriState::from(result);
    }

    TriState::Unknown
}

/// Compares string values using lexicographic ordering.
fn compare_lexicographic(comparator: Comparator, left: &Value, right: &Value) -> TriState {
    let (Value::String(left), Value::String(right)) = (left, right) else {
        return TriState::Unknown;
    };
    let ordering = left.cmp(right);
    let result = match comparator {
        Comparator::LexGreaterThan => ordering.is_gt(),
        Comparator::LexGreaterThanOrEqual => ordering.is_ge(),
        Comparator::LexLessThan => ordering.is_lt(),
        Comparator::LexLessThanOrEqual => ordering.is_le(),
        _ => return TriState::Unknown,
    };
    TriState::from(result)
}

/// Compares arrays/objects using deep structural equality.
fn compare_deep_equals(left: &Value, right: &Value) -> TriState {
    match (left, right) {
        (Value::Array(_), Value::Array(_)) | (Value::Object(_), Value::Object(_)) => {
            TriState::from(left == right)
        }
        _ => TriState::Unknown,
    }
}

/// Compares arrays/objects using deep structural inequality.
fn compare_deep_not_equals(left: &Value, right: &Value) -> TriState {
    match (left, right) {
        (Value::Array(_), Value::Array(_)) | (Value::Object(_), Value::Object(_)) => {
            TriState::from(left != right)
        }
        _ => TriState::Unknown,
    }
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
    let Value::Array(values) = expected else {
        return TriState::Unknown;
    };
    match value {
        Value::Array(_) | Value::Object(_) => TriState::Unknown,
        _ => TriState::from(values.contains(value)),
    }
}

/// Compares numbers by parsing them into `BigDecimal` values.
fn compare_decimal_equality(left: &Number, right: &Number, equals: bool) -> TriState {
    let Some(left) = decimal_from_number(left) else {
        return TriState::Unknown;
    };
    let Some(right) = decimal_from_number(right) else {
        return TriState::Unknown;
    };
    TriState::from(if equals { left == right } else { left != right })
}

/// Orders numeric JSON values using decimal-aware comparison.
fn decimal_cmp(left: &Number, right: &Number) -> Option<Ordering> {
    let left = decimal_from_number(left)?;
    let right = decimal_from_number(right)?;
    Some(left.cmp(&right))
}

/// Parses a JSON number into `BigDecimal` with a stable string representation.
fn decimal_from_number(number: &Number) -> Option<BigDecimal> {
    let rendered = number.to_string();
    BigDecimal::from_str(&rendered).ok()
}

/// Compares RFC3339 date-time or date-only strings.
fn temporal_cmp(left: &str, right: &str) -> Option<Ordering> {
    if let (Ok(left), Ok(right)) =
        (OffsetDateTime::parse(left, &Rfc3339), OffsetDateTime::parse(right, &Rfc3339))
    {
        return Some(left.cmp(&right));
    }
    let left = parse_rfc3339_date(left)?;
    let right = parse_rfc3339_date(right)?;
    Some(left.cmp(&right))
}

/// Parses an RFC3339 date-only value (YYYY-MM-DD).
fn parse_rfc3339_date(value: &str) -> Option<Date> {
    let mut parts = value.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let month = time::Month::try_from(month).ok()?;
    Date::from_calendar_date(year, month, day).ok()
}
