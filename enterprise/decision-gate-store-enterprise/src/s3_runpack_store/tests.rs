// enterprise/decision-gate-store-enterprise/src/s3_runpack_store/tests.rs
// ============================================================================
// Module: S3 Runpack Store Unit Tests
// Description: Unit tests for S3 prefix normalization.
// Purpose: Validate prefix normalization and rejection rules.
// ============================================================================

#![allow(clippy::expect_used, reason = "Unit tests use expect for setup clarity.")]

use super::normalize_prefix;

#[test]
fn normalize_prefix_none_is_empty() {
    let normalized = normalize_prefix(None).expect("normalize");
    assert_eq!(normalized, "");
}

#[test]
fn normalize_prefix_trims_and_appends_slash() {
    let normalized = normalize_prefix(Some("/runs/prefix/")).expect("normalize");
    assert_eq!(normalized, "runs/prefix/");
}

#[test]
fn normalize_prefix_empty_or_root_is_empty() {
    let normalized = normalize_prefix(Some("///")).expect("normalize");
    assert_eq!(normalized, "");
    let normalized = normalize_prefix(Some("")).expect("normalize");
    assert_eq!(normalized, "");
}

#[test]
fn normalize_prefix_rejects_invalid_segments() {
    assert!(normalize_prefix(Some("bad/../prefix")).is_err());
    assert!(normalize_prefix(Some("bad//prefix")).is_err());
    assert!(normalize_prefix(Some("bad\\prefix")).is_err());
}

#[test]
fn normalize_prefix_rejects_overlength_segment() {
    let long_segment = "a".repeat(256);
    let input = format!("{long_segment}/good");
    assert!(normalize_prefix(Some(&input)).is_err());
}
