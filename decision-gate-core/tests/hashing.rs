// decision-gate-core/tests/hashing.rs
// ============================================================================
// Module: Hashing Tests
// Description: Tests for canonical JSON hashing.
// ============================================================================
//! ## Overview
//! Validates deterministic hashing using RFC 8785 canonicalization.

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

use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_canonical_json;
use serde_json::json;

// ============================================================================
// SECTION: Canonical Hashing
// ============================================================================

/// Tests canonical json hash is stable.
#[test]
fn test_canonical_json_hash_is_stable() {
    let value_a = json!({"b": 1, "a": 2});
    let value_b = json!({"a": 2, "b": 1});

    let hash_a = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &value_a).unwrap();
    let hash_b = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &value_b).unwrap();

    assert_eq!(hash_a, hash_b);
}
