// crates/decision-gate-cli/src/tests/timing.rs
// ============================================================================
// Module: Timing-Safe Comparison Tests
// Description: Unit tests for constant-time comparison helpers.
// Purpose: Ensure security helpers behave correctly for secrets.
// Dependencies: decision-gate-cli security module
// ============================================================================

//! ## Overview
//! Verifies constant-time equality helpers return correct results for
//! equal/unequal inputs of varying lengths.

use crate::security::constant_time_eq;
use crate::security::constant_time_eq_str;

#[test]
fn constant_time_eq_returns_true_for_equal_bytes() {
    let a = b"secret-token";
    let b = b"secret-token";
    assert!(constant_time_eq(a, b));
}

#[test]
fn constant_time_eq_returns_false_for_mismatch() {
    let a = b"secret-token";
    let b = b"secret-token-x";
    assert!(!constant_time_eq(a, b));
}

#[test]
fn constant_time_eq_handles_empty_inputs() {
    assert!(constant_time_eq(b"", b""));
    assert!(!constant_time_eq(b"", b"nonempty"));
}

#[test]
fn constant_time_eq_str_matches_equal_strings() {
    assert!(constant_time_eq_str("CN=test", "CN=test"));
    assert!(!constant_time_eq_str("CN=test", "CN=other"));
}
