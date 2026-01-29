// decision-gate-cli/src/main_tests.rs
// ============================================================================
// Module: CLI Main Helpers Tests
// Description: Unit tests for file read size enforcement in the CLI entry point.
// Purpose: Ensure bounded reads fail closed on oversized inputs.
// Dependencies: decision-gate-cli main helpers
// ============================================================================

//! ## Overview
//! Validates `read_bytes_with_limit` enforces size limits for CLI inputs.
//!
//! Security posture: CLI inputs are untrusted; size limits must fail closed.

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

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::ReadLimitError;
use super::read_bytes_with_limit;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn temp_file(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-cli-{label}-{nanos}.bin"));
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn read_bytes_with_limit_allows_small_file() {
    let path = temp_file("io-small");
    fs::write(&path, b"ok").expect("write small file");

    let bytes = read_bytes_with_limit(&path, 16).expect("read small file");
    assert_eq!(bytes, b"ok");

    cleanup(&path);
}

#[test]
fn read_bytes_with_limit_rejects_large_file() {
    let path = temp_file("io-large");
    let limit = 8_usize;
    let payload = vec![0_u8; limit + 1];
    fs::write(&path, payload).expect("write large file");

    let err = read_bytes_with_limit(&path, limit).expect_err("expected size limit failure");
    match err {
        ReadLimitError::TooLarge { size, limit: reported } => {
            let limit_u64 = u64::try_from(limit).expect("limit fits");
            assert!(size > limit_u64);
            assert_eq!(reported, limit);
        }
        ReadLimitError::Io(err) => panic!("unexpected IO error: {err}"),
    }

    cleanup(&path);
}
