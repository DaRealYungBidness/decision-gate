// decision-gate-cli/tests/authoring_commands.rs
// ============================================================================
// Module: CLI Authoring Command Tests
// Description: Integration tests for CLI authoring validation and normalization.
// Purpose: Ensure authoring commands validate inputs and emit canonical JSON.
// Dependencies: decision-gate-cli binary, decision-gate-contract, serde_json
// ============================================================================

//! CLI authoring command integration tests.

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
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_contract::examples;
use serde_json::Value;

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

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let bytes = serde_json::to_vec(value).expect("serialize");
    fs::write(path, bytes).expect("write json");
}

fn write_text(path: &Path, value: &str) {
    fs::write(path, value).expect("write text");
}

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Verifies authoring validate succeeds for canonical JSON input.
#[test]
fn cli_authoring_validate_json_succeeds() {
    let root = temp_root("authoring-validate");
    let input_path = root.join("scenario.json");
    let spec = examples::scenario_example();
    write_json(&input_path, &spec);

    let output = Command::new(decision_gate_bin())
        .args(["authoring", "validate", "--input", input_path.to_string_lossy().as_ref()])
        .output()
        .expect("authoring validate");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ScenarioSpec valid"), "unexpected stdout: {stdout}");

    cleanup(&root);
}

/// Verifies authoring validate fails for invalid input.
#[test]
fn cli_authoring_validate_rejects_invalid_input() {
    let root = temp_root("authoring-invalid");
    let input_path = root.join("scenario.json");
    write_text(&input_path, "{}");

    let output = Command::new(decision_gate_bin())
        .args(["authoring", "validate", "--input", input_path.to_string_lossy().as_ref()])
        .output()
        .expect("authoring validate");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Schema validation failed"), "unexpected stderr: {stderr}");

    cleanup(&root);
}

/// Verifies authoring normalize writes canonical JSON output for RON input.
#[test]
fn cli_authoring_normalize_writes_output() {
    let root = temp_root("authoring-normalize");
    let input_path = root.join("scenario.ron");
    let output_path = root.join("scenario.json");
    let ron = examples::scenario_example_ron().expect("ron example");
    write_text(&input_path, &ron);

    let output = Command::new(decision_gate_bin())
        .args([
            "authoring",
            "normalize",
            "--input",
            input_path.to_string_lossy().as_ref(),
            "--format",
            "ron",
            "--output",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("authoring normalize");

    assert!(output.status.success());
    assert!(output_path.exists(), "normalized output missing");
    let normalized: Value = serde_json::from_slice(&fs::read(&output_path).expect("read output"))
        .expect("parse output json");
    assert_eq!(normalized.get("scenario_id").and_then(Value::as_str), Some("example-scenario"));

    cleanup(&root);
}
