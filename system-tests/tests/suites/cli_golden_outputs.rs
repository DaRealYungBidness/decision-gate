// system-tests/tests/suites/cli_golden_outputs.rs
// ============================================================================
// Module: CLI Golden Output Tests
// Description: Snapshot tests for canonical CLI JSON output.
// Purpose: Ensure CLI JSON responses remain deterministic.
// Dependencies: system-tests helpers, decision-gate-cli
// ============================================================================

//! ## Overview
//! Snapshot tests for canonical CLI JSON output.
//! Purpose: Ensure CLI JSON responses remain deterministic.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::helpers::artifacts::TestReporter;
use crate::helpers::cli::cli_binary;
use crate::helpers::cli::run_cli;

#[derive(Serialize)]
struct CliGoldenEntry {
    scenario: String,
    status: i32,
    stdout: String,
    stderr: String,
}

fn canonicalize_json(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|err| format!("parse json: {err}"))?;
    serde_jcs::to_vec(&value).map_err(|err| format!("canonicalize json: {err}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_golden_provider_list() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_golden_provider_list")?;
    let Some(cli) = cli_binary() else {
        reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "tool_transcript.json".to_string(),
            ],
        )?;
        drop(reporter);
        return Ok(());
    };

    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[server]
transport = "http"
mode = "strict"
bind = "127.0.0.1:0"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"
"#;
    fs::write(&config_path, config_contents)?;

    let output = run_cli(
        &cli,
        &[
            "provider",
            "list",
            "--config",
            config_path.to_str().unwrap_or_default(),
            "--format",
            "json",
        ],
    )?;
    let transcript = vec![CliGoldenEntry {
        scenario: "provider_list".to_string(),
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }];
    if !output.status.success() {
        return Err("cli provider list failed".into());
    }

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("cli")
        .join("provider_list.json");
    let expected_bytes = fs::read(&fixture_path)
        .map_err(|err| format!("read fixture {}: {err}", fixture_path.display()))?;

    let expected_canonical = canonicalize_json(&expected_bytes)?;
    let actual_canonical = canonicalize_json(&output.stdout)?;

    if expected_canonical != actual_canonical {
        return Err("provider list output does not match golden fixture".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI provider list matches golden output".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
