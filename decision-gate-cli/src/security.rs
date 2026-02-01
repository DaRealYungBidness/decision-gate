// decision-gate-cli/src/security.rs
// ============================================================================
// Module: CLI Security Helpers
// Description: Constant-time comparison utilities for secret material.
// Purpose: Provide reusable, side-channel resistant comparisons.
// Dependencies: subtle
// ============================================================================

//! ## Overview
//! Exposes constant-time equality helpers for secret values such as bearer
//! tokens or mTLS subject identifiers.
//!
//! Security posture: minimize timing side-channels when comparing secret inputs;
//! see `Docs/security/threat_model.md`.

use subtle::ConstantTimeEq;

// ============================================================================
// SECTION: Constant-Time Comparisons
// ============================================================================

/// Compares two byte slices in constant time.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

/// Compares two strings in constant time.
#[must_use]
pub fn constant_time_eq_str(a: &str, b: &str) -> bool {
    constant_time_eq(a.as_bytes(), b.as_bytes())
}
