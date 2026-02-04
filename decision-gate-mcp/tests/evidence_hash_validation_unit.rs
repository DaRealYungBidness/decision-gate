//! Unit tests for evidence hash validation.
// decision-gate-mcp/tests/evidence_hash_validation_unit.rs
// ============================================================================
// Module: Evidence Hash Validation Unit Tests
// Description: Tests for ensure_evidence_hash function.
// Purpose: Validate hash computation and tamper detection for evidence integrity.
// Dependencies: decision-gate-mcp, decision-gate-core
// ============================================================================
//! ## Overview
//! Tests the enhanced `ensure_evidence_hash` function that validates evidence
//! integrity by computing and verifying hashes of evidence values.
//!
//! Security posture: hash validation prevents evidence tampering during
//! signature verification; see `Docs/security/threat_model.md`.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions use unwrap for clarity."
)]

use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::HashDigest;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_mcp::evidence::ensure_evidence_hash;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Creates an `EvidenceResult` with a JSON value.
fn evidence_with_json(value: serde_json::Value) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Json(value)),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: Some("application/json".to_string()),
    }
}

/// Creates an `EvidenceResult` with a Bytes value.
fn evidence_with_bytes(bytes: Vec<u8>) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Bytes(bytes)),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: Some("application/octet-stream".to_string()),
    }
}

/// Pre-computes the hash for a JSON value.
fn compute_json_hash(value: &serde_json::Value) -> HashDigest {
    let bytes = canonical_json_bytes(value).expect("canonical json");
    hash_bytes(HashAlgorithm::Sha256, &bytes)
}

/// Pre-computes the hash for bytes.
fn compute_bytes_hash(bytes: &[u8]) -> HashDigest {
    hash_bytes(HashAlgorithm::Sha256, bytes)
}

// ============================================================================
// SECTION: Positive Cases - Matching Hashes
// ============================================================================

#[test]
fn hash_validation_accepts_matching_json_hash() {
    // Arrange
    let value = json!({"status": "approved", "count": 42});
    let expected_hash = compute_json_hash(&value);
    let mut result = evidence_with_json(value);
    result.evidence_hash = Some(expected_hash.clone());

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should accept matching JSON hash");
    assert_eq!(hash.unwrap(), expected_hash, "Should return the hash");
}

#[test]
fn hash_validation_accepts_matching_bytes_hash() {
    // Arrange
    let bytes = b"test evidence data".to_vec();
    let expected_hash = compute_bytes_hash(&bytes);
    let mut result = evidence_with_bytes(bytes);
    result.evidence_hash = Some(expected_hash.clone());

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should accept matching Bytes hash");
    assert_eq!(hash.unwrap(), expected_hash, "Should return the hash");
}

#[test]
fn hash_validation_computes_missing_json_hash() {
    // Arrange
    let value = json!({"key": "value"});
    let expected_hash = compute_json_hash(&value);
    let mut result = evidence_with_json(value);
    // Note: evidence_hash is None

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should compute missing JSON hash");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute correct hash");
    assert_eq!(result.evidence_hash, Some(expected_hash), "Should set the hash on the result");
}

#[test]
fn hash_validation_computes_missing_bytes_hash() {
    // Arrange
    let bytes = b"binary evidence".to_vec();
    let expected_hash = compute_bytes_hash(&bytes);
    let mut result = evidence_with_bytes(bytes);
    // Note: evidence_hash is None

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should compute missing Bytes hash");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute correct hash");
    assert_eq!(result.evidence_hash, Some(expected_hash), "Should set the hash on the result");
}

// ============================================================================
// SECTION: Negative Cases - Mismatched Hashes
// ============================================================================

#[test]
fn hash_validation_rejects_tampered_json_hash() {
    // Arrange
    let actual_value = json!({"key": "value"});
    let tampered_value = json!({"key": "different"});
    let tampered_hash = compute_json_hash(&tampered_value);

    let mut result = evidence_with_json(actual_value);
    result.evidence_hash = Some(tampered_hash);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_err(), "Should reject tampered JSON hash");
    let err = hash.unwrap_err();
    let EvidenceError::Provider(msg) = err;
    assert!(
        msg.contains("evidence hash mismatch"),
        "Error should mention hash mismatch, got: {msg}"
    );
}

#[test]
fn hash_validation_rejects_tampered_bytes_hash() {
    // Arrange
    let actual_bytes = b"actual data".to_vec();
    let tampered_bytes = b"tampered data".to_vec();
    let tampered_hash = compute_bytes_hash(&tampered_bytes);

    let mut result = evidence_with_bytes(actual_bytes);
    result.evidence_hash = Some(tampered_hash);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_err(), "Should reject tampered Bytes hash");
    let err = hash.unwrap_err();
    let EvidenceError::Provider(msg) = err;
    assert!(
        msg.contains("evidence hash mismatch"),
        "Error should mention hash mismatch, got: {msg}"
    );
}

#[test]
fn hash_validation_rejects_missing_value_with_signature() {
    // Arrange - EvidenceResult with no value but a hash present
    let mut result = EvidenceResult {
        value: None,
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: Some(HashDigest {
            algorithm: HashAlgorithm::Sha256,
            value: "abc123".to_string(),
        }),
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_err(), "Should reject missing value");
    let err = hash.unwrap_err();
    let EvidenceError::Provider(msg) = err;
    assert!(
        msg.contains("missing evidence hash for signature verification"),
        "Error should mention missing evidence, got: {msg}"
    );
}

// ============================================================================
// SECTION: Edge Cases
// ============================================================================

#[test]
fn hash_validation_handles_empty_json_object() {
    // Arrange
    let value = json!({});
    let expected_hash = compute_json_hash(&value);
    let mut result = evidence_with_json(value);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should handle empty JSON object");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute valid hash");
}

#[test]
fn hash_validation_handles_empty_bytes() {
    // Arrange
    let bytes = Vec::new();
    let expected_hash = compute_bytes_hash(&bytes);
    let mut result = evidence_with_bytes(bytes);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should handle empty bytes");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute valid hash");
}

#[test]
fn hash_validation_uses_canonical_json_ordering() {
    // Arrange - Two JSON objects with same data but different key ordering
    let value_1 = json!({"b": 2, "a": 1, "c": 3});
    let value_2 = json!({"a": 1, "c": 3, "b": 2});

    let mut result_1 = evidence_with_json(value_1);
    let mut result_2 = evidence_with_json(value_2);

    // Act
    let hash_1 = ensure_evidence_hash(&mut result_1).unwrap();
    let hash_2 = ensure_evidence_hash(&mut result_2).unwrap();

    // Assert
    assert_eq!(
        hash_1, hash_2,
        "Same JSON data with different key order should produce same hash (canonical ordering)"
    );
}

#[test]
fn hash_validation_handles_complex_nested_json() {
    // Arrange
    let value = json!({
        "nested": {
            "array": [1, 2, 3, {"inner": "value"}],
            "null_field": null,
            "bool_field": true,
            "number": 123.456
        },
        "top_level": "data"
    });
    let expected_hash = compute_json_hash(&value);
    let mut result = evidence_with_json(value);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should handle complex nested JSON");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute correct hash for complex structure");
}

#[test]
fn hash_validation_handles_large_json_payloads() {
    // Arrange - Create a large JSON object (not quite MAX_EVIDENCE_VALUE_BYTES but substantial)
    let mut large_obj = serde_json::Map::new();
    for i in 0 .. 100 {
        large_obj.insert(
            format!("field_{i:04}"),
            json!({
                "index": i,
                "data": "x".repeat(100),
                "nested": {"a": i, "b": i * 2}
            }),
        );
    }
    let value = serde_json::Value::Object(large_obj);
    let expected_hash = compute_json_hash(&value);
    let mut result = evidence_with_json(value);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should handle large JSON payloads");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute correct hash for large payload");
}

#[test]
fn hash_validation_handles_large_byte_payloads() {
    // Arrange - Create a large byte array
    let bytes = vec![0xAB; 10_000]; // 10KB of data
    let expected_hash = compute_bytes_hash(&bytes);
    let mut result = evidence_with_bytes(bytes);

    // Act
    let hash = ensure_evidence_hash(&mut result);

    // Assert
    assert!(hash.is_ok(), "Should handle large byte payloads");
    assert_eq!(hash.unwrap(), expected_hash, "Should compute correct hash for large byte array");
}
