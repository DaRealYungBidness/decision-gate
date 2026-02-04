// system-tests/tests/suites/log_leak_scan.rs
// ============================================================================
// Module: Log Leak Scans
// Description: Scan logs for secret leakage across error paths.
// Purpose: Ensure secrets never appear in stderr or audit logs.
// Dependencies: system-tests helpers
// ============================================================================

//! Log leakage scanning tests for Decision Gate system-tests.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use helpers::artifacts::TestReporter;
use helpers::readiness::wait_for_stdio_ready;
use helpers::stdio_client::StdioMcpClient;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn log_leak_scan_redacts_secrets() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("log_leak_scan_redacts_secrets")?;
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let audit_path = reporter.artifacts().root().join("audit.log");
    let secret = "SECRET_TOKEN_12345";

    let audit_path_toml = toml_escape_path(&audit_path);
    let config_contents = format!(
        r#"[server]
transport = "stdio"
mode = "strict"

[server.audit]
enabled = true
path = "{audit_path_toml}"
log_precheck_payloads = false

[[providers]]
name = "time"
type = "builtin"
"#
    );
    std::fs::write(&config_path, config_contents)?;

    let stderr_path = reporter.artifacts().root().join("mcp.stderr.log");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
    wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;

    let _ = client.list_tools().await?;
    let _ = client
        .call_tool(
            "evidence_query",
            json!({
                "query": {
                    "provider_id": "time",
                    "check_id": "after",
                    "params": { "timestamp": secret }
                },
                "context": {
                    "tenant_id": 1,
                    "namespace_id": 1,
                    "run_id": "run-1",
                    "trigger_id": "trigger-1",
                    "trigger_time": { "kind": "logical", "value": 1 }
                }
            }),
        )
        .await;

    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    if stderr.contains(secret) {
        return Err("secret leaked into stderr logs".into());
    }
    let audit = std::fs::read_to_string(&audit_path).unwrap_or_default();
    if audit.contains(secret) {
        return Err("secret leaked into audit logs".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["no secrets leaked into stderr or audit logs".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "mcp.stderr.log".to_string(),
            "audit.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

fn toml_escape_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}
