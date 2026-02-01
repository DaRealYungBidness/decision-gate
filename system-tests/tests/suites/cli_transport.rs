// system-tests/tests/suites/cli_transport.rs
// ============================================================================
// Module: CLI Transport Matrix Tests
// Description: Cross-transport CLI MCP client parity checks.
// Purpose: Ensure CLI MCP client behaves consistently across HTTP, SSE, and stdio.
// Dependencies: system-tests helpers, decision-gate-cli
// ============================================================================

//! CLI transport matrix tests for Decision Gate.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::helpers::artifacts::TestReporter;
use crate::helpers::cli::cli_binary;
use crate::helpers::cli::run_cli;
use crate::helpers::harness::allocate_bind_addr;
use crate::helpers::harness::base_http_config;
use crate::helpers::harness::base_sse_config;
use crate::helpers::harness::spawn_mcp_server;
use crate::helpers::readiness::wait_for_ready;
use crate::helpers::readiness::wait_for_server_ready;

#[derive(Serialize)]
struct CliTransportEntry {
    transport: String,
    command: String,
    status: i32,
    stdout: String,
    stderr: String,
}

fn tools_from_output(output: &Value) -> Result<Vec<String>, String> {
    let mut tools = output
        .get("tools")
        .and_then(Value::as_array)
        .ok_or("tools list missing tools array")?
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect::<Vec<_>>();
    tools.sort();
    Ok(tools)
}

fn providers_from_output(output: &Value) -> Result<Vec<String>, String> {
    let mut providers = output
        .get("providers")
        .and_then(Value::as_array)
        .ok_or("providers list missing providers array")?
        .iter()
        .filter_map(|provider| {
            provider.get("provider_id").and_then(Value::as_str).map(str::to_string)
        })
        .collect::<Vec<_>>();
    providers.sort();
    Ok(providers)
}

fn run_cli_json(
    cli: &PathBuf,
    args: &[&str],
    transcript: &mut Vec<CliTransportEntry>,
    transport: &str,
    command_label: &str,
) -> Result<Value, String> {
    let output = run_cli(cli, args)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    transcript.push(CliTransportEntry {
        transport: transport.to_string(),
        command: command_label.to_string(),
        status: output.status.code().unwrap_or(-1),
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    });
    if !output.status.success() {
        return Err(format!("cli {command_label} failed: {stderr}"));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("invalid json output for {command_label}: {err}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_transport_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_transport_matrix")?;
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

    let http_bind = allocate_bind_addr()?.to_string();
    let http_server = spawn_mcp_server(base_http_config(&http_bind)).await?;
    let http_client = http_server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&http_client, Duration::from_secs(5)).await?;
    let http_url = http_server.base_url().to_string();

    let sse_bind = allocate_bind_addr()?.to_string();
    let sse_server = spawn_mcp_server(base_sse_config(&sse_bind)).await?;
    let sse_url = sse_server.base_url().to_string();

    wait_for_ready(
        || async {
            let output = run_cli(
                &cli,
                &["mcp", "tools", "list", "--transport", "sse", "--endpoint", &sse_url],
            )?;
            if output.status.success() { Ok(()) } else { Err("sse cli not ready".to_string()) }
        },
        Duration::from_secs(5),
        "sse cli",
    )
    .await?;

    let temp_dir = TempDir::new()?;
    let stdio_config = temp_dir.path().join("decision-gate-stdio.toml");
    let stdio_config_contents = r#"[server]
transport = "stdio"
mode = "strict"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "stdio"
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

[[providers]]
name = "json"
type = "builtin"

[[providers]]
name = "http"
type = "builtin"
"#;
    fs::write(&stdio_config, stdio_config_contents)?;
    let stdio_server = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));

    let mut transcript: Vec<CliTransportEntry> = Vec::new();

    let http_tools = run_cli_json(
        &cli,
        &["mcp", "tools", "list", "--endpoint", &http_url],
        &mut transcript,
        "http",
        "mcp tools list",
    )?;
    let sse_tools = run_cli_json(
        &cli,
        &["mcp", "tools", "list", "--transport", "sse", "--endpoint", &sse_url],
        &mut transcript,
        "sse",
        "mcp tools list",
    )?;
    let stdio_tools = run_cli_json(
        &cli,
        &[
            "mcp",
            "tools",
            "list",
            "--transport",
            "stdio",
            "--stdio-command",
            stdio_server.to_str().unwrap_or_default(),
            "--stdio-config",
            stdio_config.to_str().unwrap_or_default(),
        ],
        &mut transcript,
        "stdio",
        "mcp tools list",
    )?;

    let http_tool_names = tools_from_output(&http_tools)?;
    let sse_tool_names = tools_from_output(&sse_tools)?;
    let stdio_tool_names = tools_from_output(&stdio_tools)?;

    if http_tool_names != sse_tool_names || http_tool_names != stdio_tool_names {
        return Err("tool lists differ across transports".into());
    }

    let http_providers = run_cli_json(
        &cli,
        &[
            "mcp",
            "tools",
            "call",
            "--tool",
            "providers_list",
            "--json",
            "{}",
            "--endpoint",
            &http_url,
        ],
        &mut transcript,
        "http",
        "mcp tools call providers_list",
    )?;
    let sse_providers = run_cli_json(
        &cli,
        &[
            "mcp",
            "tools",
            "call",
            "--tool",
            "providers_list",
            "--json",
            "{}",
            "--transport",
            "sse",
            "--endpoint",
            &sse_url,
        ],
        &mut transcript,
        "sse",
        "mcp tools call providers_list",
    )?;
    let stdio_providers = run_cli_json(
        &cli,
        &[
            "mcp",
            "tools",
            "call",
            "--tool",
            "providers_list",
            "--json",
            "{}",
            "--transport",
            "stdio",
            "--stdio-command",
            stdio_server.to_str().unwrap_or_default(),
            "--stdio-config",
            stdio_config.to_str().unwrap_or_default(),
        ],
        &mut transcript,
        "stdio",
        "mcp tools call providers_list",
    )?;

    let http_provider_ids = providers_from_output(&http_providers)?;
    let sse_provider_ids = providers_from_output(&sse_providers)?;
    let stdio_provider_ids = providers_from_output(&stdio_providers)?;

    if http_provider_ids != sse_provider_ids || http_provider_ids != stdio_provider_ids {
        return Err("provider lists differ across transports".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI transport parity verified across HTTP/SSE/stdio".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    http_server.shutdown().await;
    sse_server.shutdown().await;
    drop(reporter);
    Ok(())
}
