// system-tests/tests/suites/cli_limits.rs
// ============================================================================
// Module: CLI Size Limit Tests
// Description: CLI input/output size limit enforcement.
// Purpose: Ensure CLI fails closed on oversized inputs and outputs.
// Dependencies: system-tests helpers, decision-gate-cli
// ============================================================================

//! CLI size limit coverage for Decision Gate.

use std::fs;

use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::helpers::artifacts::TestReporter;
use crate::helpers::cli::cli_binary;
use crate::helpers::cli::run_cli;

#[derive(Serialize)]
struct CliLimitEntry {
    scenario: String,
    status: i32,
    stdout: String,
    stderr: String,
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_size_limits_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_size_limits_enforced")?;
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
    let oversize_path = temp_dir.path().join("oversize.json");
    let oversize = vec![b'a'; MAX_RUNPACK_ARTIFACT_BYTES + 1];
    fs::write(&oversize_path, &oversize)?;

    let mut transcript: Vec<CliLimitEntry> = Vec::new();

    let input_fail = run_cli(
        &cli,
        &[
            "mcp",
            "tools",
            "call",
            "--tool",
            "providers_list",
            "--input",
            oversize_path.to_str().unwrap_or_default(),
            "--endpoint",
            "http://127.0.0.1:1/rpc",
        ],
    )?;
    transcript.push(CliLimitEntry {
        scenario: "mcp_tool_input_oversize".to_string(),
        status: input_fail.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&input_fail.stdout).to_string(),
        stderr: String::from_utf8_lossy(&input_fail.stderr).to_string(),
    });
    if input_fail.status.success() {
        return Err("expected oversized mcp tool input to fail".into());
    }
    if !String::from_utf8_lossy(&input_fail.stderr).contains("Refusing to read") {
        return Err("expected oversize input error message".into());
    }

    let tiny_config = temp_dir.path().join("decision-gate.toml");
    let tiny_contents = r#"[server]
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

[provider_discovery]
max_response_bytes = 1
"#;
    fs::write(&tiny_config, tiny_contents)?;

    let output_fail =
        run_cli(&cli, &["provider", "list", "--config", tiny_config.to_str().unwrap_or_default()])?;
    transcript.push(CliLimitEntry {
        scenario: "provider_list_output_oversize".to_string(),
        status: output_fail.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output_fail.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output_fail.stderr).to_string(),
    });
    if output_fail.status.success() {
        return Err("expected provider list to fail output size limit".into());
    }
    if !String::from_utf8_lossy(&output_fail.stderr).contains("size limit") {
        return Err("expected output size limit error message".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI input/output size limits enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
