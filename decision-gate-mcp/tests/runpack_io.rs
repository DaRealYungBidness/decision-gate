// decision-gate-mcp/tests/runpack_io.rs
// ============================================================================
// Module: Runpack IO Tests
// Description: Tests for file-backed runpack artifact IO.
// Purpose: Validate path safety and IO round-trips for runpack artifacts.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================
//! ## Overview
//! Exercises filesystem-backed runpack artifact IO with adversarial paths.
//!
//! Security posture: Runpack paths are untrusted; all IO must fail closed.
//! Threat model: TM-RUNPACK-001 - Path traversal or path length abuse.

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

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_core::Artifact;
use decision_gate_core::ArtifactKind;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactSink;
use decision_gate_mcp::FileArtifactReader;
use decision_gate_mcp::FileArtifactSink;

// ========================================================================
// SECTION: Helpers
// ========================================================================

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-mcp-{label}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

fn sample_artifact(path: &str, bytes: &[u8]) -> Artifact {
    Artifact {
        kind: ArtifactKind::ScenarioSpec,
        path: path.to_string(),
        content_type: Some("application/json".to_string()),
        bytes: bytes.to_vec(),
        required: true,
    }
}

// ========================================================================
// SECTION: Round-Trip IO Tests
// ========================================================================

/// Verifies file-backed sinks and readers round-trip bytes successfully.
#[test]
fn file_artifact_sink_and_reader_round_trip() {
    let root = temp_root("roundtrip");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();
    let artifact = sample_artifact("evidence/log.json", b"hello");

    let reference = sink.write(&artifact).unwrap();
    let reference_path = PathBuf::from(&reference.uri);
    let expected = PathBuf::from("evidence").join("log.json");
    assert!(
        reference_path.ends_with(&expected),
        "unexpected artifact reference path: {}",
        reference.uri
    );

    let reader = FileArtifactReader::new(root.clone()).unwrap();
    let bytes = reader.read("evidence/log.json").unwrap();
    assert_eq!(bytes, b"hello");

    cleanup(&root);
}

// ========================================================================
// SECTION: Path Safety Tests
// ========================================================================

/// Verifies absolute artifact paths are rejected.
#[test]
fn file_artifact_sink_rejects_absolute_paths() {
    let root = temp_root("absolute");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    let absolute =
        if cfg!(windows) { "C:\\Windows\\System32\\drivers\\etc\\hosts" } else { "/etc/passwd" };

    let artifact = sample_artifact(absolute, b"nope");
    let err = sink.write(&artifact).unwrap_err();
    assert!(err.to_string().contains("absolute artifact path"), "unexpected error: {err}");

    cleanup(&root);
}

/// Verifies manifest path traversal is rejected.
#[test]
fn file_artifact_sink_rejects_manifest_traversal() {
    let root = temp_root("manifest-traversal");
    let result = FileArtifactSink::new(root.clone(), "../manifest.json");
    assert!(result.is_err());

    cleanup(&root);
}

/// Verifies parent traversal is rejected for readers.
#[test]
fn file_artifact_reader_rejects_parent_traversal() {
    let root = temp_root("traversal");
    fs::write(root.join("safe.json"), b"safe").unwrap();
    let reader = FileArtifactReader::new(root.clone()).unwrap();

    let result = reader.read("../escape.json");
    assert!(result.is_err());

    cleanup(&root);
}

// ========================================================================
// SECTION: Path Limit Tests
// ========================================================================

/// Verifies overly long path components are rejected.
#[test]
fn file_artifact_sink_rejects_overlong_component() {
    let mut root = std::env::temp_dir();
    root.push("a".repeat(256));
    let result = FileArtifactSink::new(root, "runpack.json");
    assert!(result.is_err());
}

/// Verifies overly long total paths are rejected.
#[test]
fn file_artifact_sink_rejects_overlong_total_path() {
    let too_long = "a".repeat(5000);
    let root = PathBuf::from(too_long);
    let result = FileArtifactSink::new(root, "runpack.json");
    assert!(result.is_err());
}

// ========================================================================
// SECTION: Path Edge Cases
// ========================================================================

/// Verifies that Windows UNC paths are rejected.
#[cfg(windows)]
#[test]
fn file_artifact_sink_rejects_unc_paths() {
    let root = temp_root("unc");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    // Test verbatim UNC path
    let verbatim_unc = r"\\?\C:\Windows\System32\config";
    let artifact = sample_artifact(verbatim_unc, b"nope");
    let err = sink.write(&artifact).unwrap_err();
    assert!(
        err.to_string().contains("absolute artifact path") || err.to_string().contains("artifact path"),
        "unexpected error for verbatim UNC path: {err}"
    );

    // Test server UNC path
    let server_unc = r"\\server\share\file.txt";
    let artifact2 = sample_artifact(server_unc, b"nope");
    let err2 = sink.write(&artifact2).unwrap_err();
    assert!(
        err2.to_string().contains("absolute artifact path") || err2.to_string().contains("artifact path"),
        "unexpected error for server UNC path: {err2}"
    );

    cleanup(&root);
}

/// Verifies that path traversal attempts are rejected even with normalization tricks.
#[test]
fn file_artifact_sink_rejects_traversal_with_normalization() {
    let root = temp_root("normalized");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    // Test path attempting traversal through relative components
    // This should be rejected because it contains .. component
    let traversal = "artifacts/../escape.json";
    let artifact = sample_artifact(traversal, b"test");
    let err = sink.write(&artifact).unwrap_err();
    assert!(
        err.to_string().contains("artifact path"),
        "expected path error for traversal, got: {err}"
    );

    cleanup(&root);
}

/// Verifies that certain non-normalized paths are safely handled.
#[test]
fn file_artifact_sink_normalizes_safe_paths() {
    let root = temp_root("safe-normalized");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    // These paths should be normalized and allowed (not rejected)
    // The implementation safely handles ./ and // by normalizing them

    // Path with ./ should be normalized to artifacts/log.json
    let dot_slash = "artifacts/./log.json";
    let artifact = sample_artifact(dot_slash, b"test-dot");
    let result = sink.write(&artifact);
    assert!(
        result.is_ok(),
        "safe path normalization should allow ./, got error: {:?}",
        result
    );

    cleanup(&root);
}

/// Verifies path component and total path length boundaries.
#[test]
fn file_artifact_sink_path_component_boundaries() {
    let root = temp_root("boundaries");

    // Test exact boundary for component length (255 chars should pass)
    let component_254 = "a".repeat(254);
    let result_254 = FileArtifactSink::new(root.join(&component_254), "runpack.json");
    assert!(result_254.is_ok(), "254-char component should be accepted");

    // Test exact boundary (255 chars should pass)
    let component_255 = "a".repeat(255);
    let result_255 = FileArtifactSink::new(root.join(&component_255), "runpack.json");
    assert!(result_255.is_ok(), "255-char component should be accepted");

    // Test over boundary (256 chars should fail)
    let component_256 = "a".repeat(256);
    let result_256 = FileArtifactSink::new(root.join(&component_256), "runpack.json");
    assert!(result_256.is_err(), "256-char component should be rejected");

    cleanup(&root);
}
