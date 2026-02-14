// crates/decision-gate-cli/tests/serve_command.rs
// ============================================================================
// Module: CLI Serve Command Tests
// Description: Integration tests for the CLI serve command safety checks.
// Purpose: Ensure non-loopback binds fail closed before server startup.
// Dependencies: decision-gate-cli binary
// ============================================================================
//! ## Overview
//! Validates that the CLI refuses to bind MCP servers to non-loopback
//! addresses unless explicit auth/policy support exists.
//!
//! Security posture: local-only is a hard requirement; fail closed.
//! Threat model: TM-CLI-002 - accidental network exposure of MCP.

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

/// Verifies non-loopback binds are rejected before server startup.
#[test]
fn cli_serve_rejects_non_loopback_bind() {
    let root = temp_root("serve");
    let config_path = root.join("decision-gate.toml");

    let config = r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"
"#;
    fs::write(&config_path, config.trim()).expect("write config");

    let output = Command::new(decision_gate_bin())
        .args(["serve", "--config", config_path.to_string_lossy().as_ref()])
        .output()
        .expect("run decision-gate serve");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("non-loopback"), "unexpected stderr: {stderr}");

    cleanup(&root);
}

/// Verifies `--json-root-id` cannot be used without `--json-root`.
#[test]
fn cli_serve_rejects_json_root_id_without_json_root() {
    let root = temp_root("json-root-id");
    let config_path = root.join("decision-gate.toml");

    let config = r#"
[server]
transport = "stdio"
"#;
    fs::write(&config_path, config.trim()).expect("write config");

    let output = Command::new(decision_gate_bin())
        .args([
            "serve",
            "--config",
            config_path.to_string_lossy().as_ref(),
            "--json-root-id",
            "arxi-evidence-root",
        ])
        .output()
        .expect("run decision-gate serve");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--json-root-id requires --json-root"), "unexpected stderr: {stderr}");

    cleanup(&root);
}

/// Verifies `--json-root` fails closed when no built-in `json` provider exists.
#[test]
fn cli_serve_rejects_json_root_without_builtin_json_provider() {
    let root = temp_root("json-root-missing-provider");
    let config_path = root.join("decision-gate.toml");
    let evidence_root = root.join("evidence-root");
    fs::create_dir_all(&evidence_root).expect("create evidence root");

    let config = r#"
[server]
transport = "stdio"

[[providers]]
name = "env"
type = "builtin"
"#;
    fs::write(&config_path, config.trim()).expect("write config");

    let output = Command::new(decision_gate_bin())
        .args([
            "serve",
            "--config",
            config_path.to_string_lossy().as_ref(),
            "--json-root",
            evidence_root.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run decision-gate serve");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("built-in provider 'json'"), "unexpected stderr: {stderr}");

    cleanup(&root);
}
