// system-tests/tests/suites/contract_cli.rs
// ============================================================================
// Module: Contract CLI Tests
// Description: End-to-end decision-gate-contract CLI coverage.
// Purpose: Ensure generate/check workflows succeed and fail closed on drift.
// Dependencies: system-tests helpers
// ============================================================================

//! Contract CLI coverage for system-tests.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use helpers::artifacts::TestReporter;
use tempfile::TempDir;

use crate::helpers;

fn contract_binary() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_decision_gate_contract").map(PathBuf::from)
}

fn run_contract(binary: &Path, cwd: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|err| format!("run decision-gate-contract failed: {err}"))
}

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "Contract CLI flow kept in one sequence for auditability."
)]
async fn contract_cli_generate_and_check() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("contract_cli_generate_and_check")?;
    let Some(binary) = contract_binary() else {
        reporter.finish(
            "skip",
            vec!["decision-gate-contract binary unavailable".to_string()],
            vec!["summary.json".to_string(), "summary.md".to_string()],
        )?;
        drop(reporter);
        return Ok(());
    };

    let temp_dir = TempDir::new()?;
    let docs_dir = temp_dir.path().join("Docs/configuration");
    fs::create_dir_all(&docs_dir)?;

    let output_dir = temp_dir.path().join("contract_out");
    let generate = run_contract(
        &binary,
        temp_dir.path(),
        &["generate", "--out", output_dir.to_str().unwrap_or_default()],
    )?;
    reporter
        .artifacts()
        .write_text("contract.generate.stdout.log", &String::from_utf8_lossy(&generate.stdout))?;
    reporter
        .artifacts()
        .write_text("contract.generate.stderr.log", &String::from_utf8_lossy(&generate.stderr))?;
    if !generate.status.success() {
        return Err("contract generate failed".into());
    }

    let index_path = output_dir.join("index.json");
    let tooling_path = output_dir.join("tooling.json");
    let providers_path = output_dir.join("providers.json");
    if !index_path.exists() || !tooling_path.exists() || !providers_path.exists() {
        return Err("contract outputs missing after generate".into());
    }

    let check = run_contract(
        &binary,
        temp_dir.path(),
        &["check", "--out", output_dir.to_str().unwrap_or_default()],
    )?;
    reporter
        .artifacts()
        .write_text("contract.check.stdout.log", &String::from_utf8_lossy(&check.stdout))?;
    reporter
        .artifacts()
        .write_text("contract.check.stderr.log", &String::from_utf8_lossy(&check.stderr))?;
    if !check.status.success() {
        return Err("contract check failed".into());
    }

    fs::write(&tooling_path, "tampered")?;
    let drift = run_contract(
        &binary,
        temp_dir.path(),
        &["check", "--out", output_dir.to_str().unwrap_or_default()],
    )?;
    reporter
        .artifacts()
        .write_text("contract.drift.stdout.log", &String::from_utf8_lossy(&drift.stdout))?;
    reporter
        .artifacts()
        .write_text("contract.drift.stderr.log", &String::from_utf8_lossy(&drift.stderr))?;
    if drift.status.success() {
        return Err("contract check should fail on drift".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["contract CLI generate/check workflows validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "contract.generate.stdout.log".to_string(),
            "contract.generate.stderr.log".to_string(),
            "contract.check.stdout.log".to_string(),
            "contract.check.stderr.log".to_string(),
            "contract.drift.stdout.log".to_string(),
            "contract.drift.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
