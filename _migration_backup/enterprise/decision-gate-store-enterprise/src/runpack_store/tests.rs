// enterprise/decision-gate-store-enterprise/src/runpack_store/tests.rs
// ============================================================================
// Module: Runpack Store Unit Tests
// Description: Unit tests for runpack key/path validation helpers.
// Purpose: Validate segment rules and relative path handling.
// ============================================================================

#![allow(clippy::expect_used, reason = "Unit tests use expect for setup clarity.")]

use std::path::Path;

use super::validate_segment;

#[test]
fn validate_segment_accepts_normal_values() {
    validate_segment("tenant-1").expect("segment ok");
    validate_segment("namespace").expect("segment ok");
    validate_segment("run-123").expect("segment ok");
}

#[test]
fn validate_segment_rejects_empty_or_traversal() {
    assert!(validate_segment("").is_err());
    assert!(validate_segment(".").is_err());
    assert!(validate_segment("..").is_err());
}

#[test]
fn validate_segment_rejects_overlength_or_separators() {
    let long_value = "a".repeat(256);
    assert!(validate_segment(&long_value).is_err());
    assert!(validate_segment("bad/seg").is_err());
    assert!(validate_segment("bad\\seg").is_err());
}

#[cfg(feature = "s3")]
#[test]
fn validate_relative_path_rejects_traversal() {
    let result = super::validate_relative_path(Path::new("../escape"));
    assert!(result.is_err());
}

#[cfg(feature = "s3")]
#[test]
fn validate_relative_path_accepts_nested_paths() {
    super::validate_relative_path(Path::new("a/b/c")).expect("relative path ok");
}

#[cfg(feature = "s3")]
#[test]
fn validate_relative_path_rejects_absolute() {
    let result = super::validate_relative_path(Path::new("/absolute/path"));
    assert!(result.is_err());
}
