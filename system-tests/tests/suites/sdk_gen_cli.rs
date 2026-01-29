// system-tests/tests/suites/sdk_gen_cli.rs
// ============================================================================
// Module: SDK Generator CLI Tests
// Description: End-to-end decision-gate-sdk-gen CLI coverage.
// Purpose: Ensure generate/check workflows succeed and fail closed on drift/invalid input.
// Dependencies: system-tests helpers
// ============================================================================

//! SDK generator CLI coverage for system-tests.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use helpers::artifacts::TestReporter;
use tempfile::TempDir;

use crate::helpers;

fn sdk_gen_binary() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_decision_gate_sdk_gen").map(PathBuf::from)
}

fn run_sdk_gen(binary: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new(binary)
        .args(args)
        .output()
        .map_err(|err| format!("run decision-gate-sdk-gen failed: {err}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn sdk_gen_cli_generate_and_check() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sdk_gen_cli_generate_and_check")?;
    let Some(binary) = sdk_gen_binary() else {
        reporter.finish(
            "skip",
            vec!["decision-gate-sdk-gen binary unavailable".to_string()],
            vec!["summary.json".to_string(), "summary.md".to_string()],
        )?;
        return Ok(());
    };
    let temp_dir = TempDir::new()?;
    let tooling_src = PathBuf::from("Docs/generated/decision-gate/tooling.json");
    let tooling_path = temp_dir.path().join("tooling.json");
    fs::copy(&tooling_src, &tooling_path)?;

    let python_out = temp_dir.path().join("python/_generated.py");
    let ts_out = temp_dir.path().join("typescript/_generated.ts");
    let openapi_out = temp_dir.path().join("openapi/decision-gate.json");

    let generate = run_sdk_gen(
        &binary,
        &[
            "generate",
            "--tooling",
            tooling_path.to_str().unwrap_or_default(),
            "--python-out",
            python_out.to_str().unwrap_or_default(),
            "--typescript-out",
            ts_out.to_str().unwrap_or_default(),
            "--openapi-out",
            openapi_out.to_str().unwrap_or_default(),
        ],
    )?;
    reporter
        .artifacts()
        .write_text("sdk_gen.generate.stdout.log", &String::from_utf8_lossy(&generate.stdout))?;
    reporter
        .artifacts()
        .write_text("sdk_gen.generate.stderr.log", &String::from_utf8_lossy(&generate.stderr))?;
    if !generate.status.success() {
        return Err("sdk-gen generate failed".into());
    }
    if !python_out.exists() || !ts_out.exists() || !openapi_out.exists() {
        return Err("sdk-gen outputs missing after generate".into());
    }

    let check = run_sdk_gen(
        &binary,
        &[
            "check",
            "--tooling",
            tooling_path.to_str().unwrap_or_default(),
            "--python-out",
            python_out.to_str().unwrap_or_default(),
            "--typescript-out",
            ts_out.to_str().unwrap_or_default(),
            "--openapi-out",
            openapi_out.to_str().unwrap_or_default(),
        ],
    )?;
    reporter
        .artifacts()
        .write_text("sdk_gen.check.stdout.log", &String::from_utf8_lossy(&check.stdout))?;
    reporter
        .artifacts()
        .write_text("sdk_gen.check.stderr.log", &String::from_utf8_lossy(&check.stderr))?;
    if !check.status.success() {
        return Err("sdk-gen check failed".into());
    }

    fs::write(&python_out, "tampered output")?;
    let drift = run_sdk_gen(
        &binary,
        &[
            "check",
            "--tooling",
            tooling_path.to_str().unwrap_or_default(),
            "--python-out",
            python_out.to_str().unwrap_or_default(),
            "--typescript-out",
            ts_out.to_str().unwrap_or_default(),
            "--openapi-out",
            openapi_out.to_str().unwrap_or_default(),
        ],
    )?;
    reporter
        .artifacts()
        .write_text("sdk_gen.drift.stdout.log", &String::from_utf8_lossy(&drift.stdout))?;
    reporter
        .artifacts()
        .write_text("sdk_gen.drift.stderr.log", &String::from_utf8_lossy(&drift.stderr))?;
    if drift.status.success() {
        return Err("sdk-gen check should fail on drift".into());
    }

    let invalid_tooling = temp_dir.path().join("invalid_tooling.json");
    fs::write(&invalid_tooling, "{ invalid")?;
    let invalid = run_sdk_gen(
        &binary,
        &[
            "generate",
            "--tooling",
            invalid_tooling.to_str().unwrap_or_default(),
            "--python-out",
            python_out.to_str().unwrap_or_default(),
            "--typescript-out",
            ts_out.to_str().unwrap_or_default(),
            "--openapi-out",
            openapi_out.to_str().unwrap_or_default(),
        ],
    )?;
    reporter
        .artifacts()
        .write_text("sdk_gen.invalid.stdout.log", &String::from_utf8_lossy(&invalid.stdout))?;
    reporter
        .artifacts()
        .write_text("sdk_gen.invalid.stderr.log", &String::from_utf8_lossy(&invalid.stderr))?;
    if invalid.status.success() {
        return Err("sdk-gen generate should fail on invalid tooling".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["sdk-gen generate/check workflows validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "sdk_gen.generate.stdout.log".to_string(),
            "sdk_gen.generate.stderr.log".to_string(),
            "sdk_gen.check.stdout.log".to_string(),
            "sdk_gen.check.stderr.log".to_string(),
            "sdk_gen.drift.stdout.log".to_string(),
            "sdk_gen.drift.stderr.log".to_string(),
            "sdk_gen.invalid.stdout.log".to_string(),
            "sdk_gen.invalid.stderr.log".to_string(),
        ],
    )?;
    Ok(())
}
