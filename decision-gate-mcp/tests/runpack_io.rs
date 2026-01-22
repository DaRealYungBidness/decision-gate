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
    assert!(reference.uri.contains("evidence/log.json"));

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
