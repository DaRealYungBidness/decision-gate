// crates/decision-gate-mcp/src/correlation/tests.rs
// ============================================================================
// Module: Correlation Policy Tests
// Description: Unit tests for correlation ID sanitization and generation.
// Purpose: Validate rejection reasons and generator formatting guarantees.
// Dependencies: decision-gate-mcp
// ============================================================================

//! ## Overview
//! Validates correlation ID sanitization rejects malformed inputs and that
//! server-generated correlation IDs follow stable formatting rules.
//!
//! Security posture: Tests cover rejection cases for untrusted correlation
//! headers; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test-only assertions use unwrap/expect for clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use super::CorrelationIdGenerator;
use super::CorrelationIdRejection;
use super::MAX_CLIENT_CORRELATION_ID_LENGTH;
use super::sanitize_client_correlation_id;

// ============================================================================
// SECTION: Sanitization Tests
// ============================================================================

#[test]
fn sanitize_rejects_empty_after_trim() {
    let err = sanitize_client_correlation_id(Some("   ")).expect_err("expected empty rejection");
    assert_eq!(err, CorrelationIdRejection::EmptyAfterTrim);
}

#[test]
fn sanitize_rejects_too_long() {
    let value = "a".repeat(MAX_CLIENT_CORRELATION_ID_LENGTH + 1);
    let err = sanitize_client_correlation_id(Some(&value)).expect_err("expected length rejection");
    assert_eq!(err, CorrelationIdRejection::TooLong);
}

#[test]
fn sanitize_rejects_whitespace() {
    let err =
        sanitize_client_correlation_id(Some("bad value")).expect_err("expected whitespace reject");
    assert_eq!(err, CorrelationIdRejection::ContainsWhitespace);
}

#[test]
fn sanitize_rejects_control_chars() {
    let err =
        sanitize_client_correlation_id(Some("bad\u{0007}")).expect_err("expected control reject");
    assert_eq!(err, CorrelationIdRejection::ContainsControlChar);
}

#[test]
fn sanitize_rejects_non_ascii() {
    let err = sanitize_client_correlation_id(Some("bad\u{00e9}")).expect_err("expected non-ascii");
    assert_eq!(err, CorrelationIdRejection::NonAscii);
}

#[test]
fn sanitize_rejects_disallowed_chars() {
    let err = sanitize_client_correlation_id(Some("bad@")).expect_err("expected tchar reject");
    assert_eq!(err, CorrelationIdRejection::ContainsDisallowedChar);
}

// ============================================================================
// SECTION: Generator Tests
// ============================================================================

#[test]
fn generator_issues_formatted_ids() {
    let generator = CorrelationIdGenerator::new("dg");
    let first = generator.issue();
    let second = generator.issue();
    assert_ne!(first, second);
    let parts: Vec<&str> = first.split('-').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "dg");
    assert_eq!(parts[1].len(), 16);
    assert_eq!(parts[2].len(), 16);
    assert!(parts[1].chars().all(|ch| ch.is_ascii_hexdigit()));
    assert!(parts[2].chars().all(|ch| ch.is_ascii_hexdigit()));
}
