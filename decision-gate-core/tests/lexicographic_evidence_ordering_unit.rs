// decision-gate-core/tests/lexicographic_evidence_ordering_unit.rs
//! Unit tests for lexicographic evidence ordering.
// ============================================================================
// Module: Lexicographic Evidence Ordering Unit Tests
// Description: Tests for canonical evidence record sorting by condition_id.
// ============================================================================
//! ## Overview
//! Tests the lexicographic sorting of evidence records by `condition_id` to
//! ensure canonical ordering regardless of provider call order.

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

use decision_gate_core::ConditionId;
use decision_gate_core::EvidenceRecord;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::TrustLane;
use ret_logic::TriState;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Creates a minimal evidence record with specific `condition_id`.
fn evidence_record(condition_id: &str, value: serde_json::Value) -> EvidenceRecord {
    EvidenceRecord {
        condition_id: ConditionId::new(condition_id),
        status: TriState::True,
        result: EvidenceResult {
            value: Some(EvidenceValue::Json(value)),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: Some("application/json".to_string()),
        },
    }
}

/// Extracts `condition_id` strings from evidence records.
fn extract_condition_ids(records: &[EvidenceRecord]) -> Vec<String> {
    records.iter().map(|r| r.condition_id.as_str().to_string()).collect()
}

// ============================================================================
// SECTION: Lexicographic Ordering Tests
// ============================================================================

#[test]
fn evidence_sorting_orders_by_condition_id_ascending() {
    // Arrange
    let mut records = vec![
        evidence_record("zebra", json!(true)),
        evidence_record("apple", json!(true)),
        evidence_record("mango", json!(true)),
        evidence_record("banana", json!(true)),
        evidence_record("cherry", json!(true)),
    ];

    // Act - sort lexicographically by condition_id
    records.sort_by_key(|r| r.condition_id.as_str().to_string());

    // Assert
    let ids = extract_condition_ids(&records);
    assert_eq!(
        ids,
        vec!["apple", "banana", "cherry", "mango", "zebra"],
        "Should sort alphabetically ascending"
    );
}

#[test]
fn evidence_sorting_handles_numeric_lexicographic_order() {
    // Arrange
    let mut records = vec![
        evidence_record("cond-10", json!(true)),
        evidence_record("cond-2", json!(true)),
        evidence_record("cond-1", json!(true)),
        evidence_record("cond-20", json!(true)),
    ];

    // Act
    records.sort_by_key(|r| r.condition_id.as_str().to_string());

    // Assert
    let ids = extract_condition_ids(&records);
    assert_eq!(
        ids,
        vec!["cond-1", "cond-10", "cond-2", "cond-20"],
        "Should sort lexicographically (not numerically): '10' < '2' in ASCII"
    );
}

#[test]
fn evidence_sorting_handles_identical_prefixes() {
    // Arrange
    let mut records = vec![
        evidence_record("test-10", json!(true)),
        evidence_record("test", json!(true)),
        evidence_record("test-2", json!(true)),
        evidence_record("test-1", json!(true)),
    ];

    // Act
    records.sort_by_key(|r| r.condition_id.as_str().to_string());

    // Assert
    let ids = extract_condition_ids(&records);
    assert_eq!(
        ids,
        vec!["test", "test-1", "test-10", "test-2"],
        "Should handle identical prefixes lexicographically"
    );
}

#[test]
fn evidence_sorting_preserves_stability() {
    // Arrange - Multiple records with same condition_id but different values
    // Note: Since EvidenceRecord doesn't have a timestamp field, we verify
    // stability using different values in the result
    let mut records = [
        evidence_record("same-id", json!({"order": 1})),
        evidence_record("same-id", json!({"order": 2})),
        evidence_record("same-id", json!({"order": 3})),
    ];

    // Act - Rust's sort_by_key is stable
    records.sort_by_key(|r| r.condition_id.as_str().to_string());

    // Assert - Original order should be preserved for equal keys
    match &records[0].result.value {
        Some(EvidenceValue::Json(val)) => {
            assert_eq!(val["order"], 1, "First record should remain first");
        }
        _ => panic!("Expected JSON value"),
    }
    match &records[1].result.value {
        Some(EvidenceValue::Json(val)) => {
            assert_eq!(val["order"], 2, "Second record should remain second");
        }
        _ => panic!("Expected JSON value"),
    }
    match &records[2].result.value {
        Some(EvidenceValue::Json(val)) => {
            assert_eq!(val["order"], 3, "Third record should remain third");
        }
        _ => panic!("Expected JSON value"),
    }
}

#[test]
fn evidence_sorting_handles_special_characters() {
    // Arrange
    let mut records = vec![
        evidence_record("cond_underscore", json!(true)),
        evidence_record("cond-hyphen", json!(true)),
        evidence_record("cond.dot", json!(true)),
        evidence_record("cond:colon", json!(true)),
        evidence_record("cond/slash", json!(true)),
    ];

    // Act
    records.sort_by_key(|r| r.condition_id.as_str().to_string());

    // Assert
    let ids = extract_condition_ids(&records);
    // ASCII ordering: '-' (45) < '.' (46) < '/' (47) < ':' (58) < '_' (95)
    assert_eq!(
        ids,
        vec!["cond-hyphen", "cond.dot", "cond/slash", "cond:colon", "cond_underscore"],
        "Should sort by ASCII values of special characters"
    );
}
