// decision-gate-cli/tests/config_commands.rs
// ============================================================================
// Module: CLI Config Command Tests
// Description: Integration tests for CLI config validation workflows.
// Purpose: Ensure config validation reports success and fails closed on errors.
// Dependencies: decision-gate-cli binary
// ============================================================================

//! ## Overview
//! Runs the CLI binary for config validation and ensures invalid configuration
//! fails closed with explicit errors.
//!
//! Security posture: configuration inputs are untrusted; validation must fail closed.

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
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn decision_gate_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_decision-gate"))
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-cli-{label}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Verifies config validation succeeds for a loopback HTTP bind.
#[test]
fn cli_config_validate_accepts_valid_config() {
    let root = temp_root("config-validate-ok");
    let config_path = root.join("decision-gate.toml");
    let config = r#"
[server]
transport = "http"
bind = "127.0.0.1:0"
"#;
    fs::write(&config_path, config.trim()).expect("write config");

    let output = Command::new(decision_gate_bin())
        .args(["config", "validate", "--config", config_path.to_string_lossy().as_ref()])
        .output()
        .expect("config validate");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config valid"), "unexpected stdout: {stdout}");

    cleanup(&root);
}

/// Verifies config validation fails closed on non-loopback binds.
#[test]
fn cli_config_validate_rejects_non_loopback() {
    let root = temp_root("config-validate-bad");
    let config_path = root.join("decision-gate.toml");
    let config = r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"
"#;
    fs::write(&config_path, config.trim()).expect("write config");

    let output = Command::new(decision_gate_bin())
        .args(["config", "validate", "--config", config_path.to_string_lossy().as_ref()])
        .output()
        .expect("config validate");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to load config"), "unexpected stderr: {stderr}");

    cleanup(&root);
}
