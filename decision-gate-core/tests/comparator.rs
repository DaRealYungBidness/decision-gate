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
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::json;

const fn result_with_json(value: serde_json::Value) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Json(value)),
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
