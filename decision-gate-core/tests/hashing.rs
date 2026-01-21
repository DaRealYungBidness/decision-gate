// decision-gate-core/tests/hashing.rs
// ============================================================================
// Module: Hashing Tests
// Description: Tests for canonical JSON hashing.
// ============================================================================
//! ## Overview
//! Validates deterministic hashing using RFC 8785 canonicalization.

#![allow(clippy::unwrap_used, reason = "Tests use unwrap on deterministic hashing inputs.")]
#![allow(clippy::expect_used, reason = "Tests use expect for explicit failure messages.")]

use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_canonical_json;
use serde_json::json;

// ============================================================================
// SECTION: Canonical Hashing
// ============================================================================

#[test]
fn test_canonical_json_hash_is_stable() {
    let value_a = json!({"b": 1, "a": 2});
    let value_b = json!({"a": 2, "b": 1});

    let hash_a = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &value_a).unwrap();
    let hash_b = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &value_b).unwrap();

    assert_eq!(hash_a, hash_b);
}
