// decision-gate-cli/tests/runpack_commands.rs
// ============================================================================
// Module: CLI Runpack Command Tests
// Description: Integration tests for CLI runpack export and verify workflows.
// Purpose: Validate CLI command wiring and offline verification outputs.
// Dependencies: decision-gate-cli binary, decision-gate-core, serde_json
// ============================================================================
//! ## Overview
//! Runs the CLI binary for runpack export and verification using temporary
//! artifacts. These tests ensure the CLI executes deterministic workflows and
//! emits expected status text.
//!
//! Security posture: CLI inputs are untrusted and must fail closed.
//! Threat model: TM-CLI-001 - Unsafe runpack output or verification bypass.

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

use decision_gate_core::AdvanceTo;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::runtime::VerificationReport;
use decision_gate_core::runtime::VerificationStatus;

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

fn minimal_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn minimal_state(spec: &ScenarioSpec) -> RunState {
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");
    RunState {
        tenant_id: TenantId::new("tenant"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        status: RunStatus::Active,
        dispatch_targets: Vec::new(),
        triggers: Vec::new(),
        gate_evals: Vec::new(),
        decisions: Vec::new(),
        packets: Vec::new(),
        submissions: Vec::new(),
        tool_calls: Vec::new(),
    }
}

fn export_runpack(root: &Path) -> PathBuf {
    let spec = minimal_spec();
    let state = minimal_state(&spec);
    let spec_path = root.join("spec.json");
    let state_path = root.join("state.json");
    write_json(&spec_path, &spec);
    write_json(&state_path, &state);

    let manifest_path = root.join("runpack.json");
    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "export",
            "--spec",
            spec_path.to_string_lossy().as_ref(),
            "--state",
            state_path.to_string_lossy().as_ref(),
            "--output-dir",
            root.to_string_lossy().as_ref(),
            "--manifest-name",
            "runpack.json",
            "--generated-at-unix-ms",
            "1700000000000",
        ])
        .output()
        .expect("runpack export");

    assert!(output.status.success(), "export failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Runpack manifest written"),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(manifest_path.exists(), "manifest not written");
    manifest_path
}

// ============================================================================
// SECTION: Version Tests
// ============================================================================

/// Verifies the version flag prints a version string.
#[test]
fn cli_version_flag_prints_version() {
    let output = Command::new(decision_gate_bin())
        .arg("--version")
        .output()
        .expect("run decision-gate --version");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("decision-gate"));
}

// ============================================================================
// SECTION: Runpack Export Tests
// ============================================================================

/// Verifies runpack export writes a manifest to disk.
#[test]
fn cli_runpack_export_writes_manifest() {
    let root = temp_root("export");
    let manifest_path = export_runpack(&root);
    assert!(manifest_path.exists());
    cleanup(&root);
}

// ============================================================================
// SECTION: Runpack Verify Tests
// ============================================================================

/// Verifies runpack verification succeeds with JSON output.
#[test]
fn cli_runpack_verify_outputs_json_report() {
    let root = temp_root("verify-json");
    let manifest = export_runpack(&root);

    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "verify",
            "--manifest",
            manifest.to_string_lossy().as_ref(),
            "--format",
            "json",
        ])
        .output()
        .expect("runpack verify");

    assert!(output.status.success());
    let report: VerificationReport = serde_json::from_slice(&output.stdout).expect("parse report");
    assert_eq!(report.status, VerificationStatus::Pass);

    cleanup(&root);
}

/// Verifies runpack verification renders markdown summaries.
#[test]
fn cli_runpack_verify_outputs_markdown_report() {
    let root = temp_root("verify-markdown");
    let manifest = export_runpack(&root);

    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "verify",
            "--manifest",
            manifest.to_string_lossy().as_ref(),
            "--format",
            "markdown",
        ])
        .output()
        .expect("runpack verify markdown");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Decision Gate Runpack Verification"));
    assert!(stdout.contains("Status: pass"));

    cleanup(&root);
}
