// system-tests/tests/suites/mcp_transport.rs
// ============================================================================
// Module: MCP Transport Tests
// Description: Transport validation for HTTP JSON-RPC.
// Purpose: Ensure MCP server is reachable and responds to tools/list and tools/call.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Transport validation for HTTP JSON-RPC.
//! Purpose: Ensure MCP server is reachable and responds to tools/list and tools/call.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::path::PathBuf;
use std::time::Duration;

use decision_gate_contract::ToolName;
use decision_gate_mcp::docs::RESOURCE_URI_PREFIX;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::base_http_config_with_bearer;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::readiness::wait_for_stdio_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::stdio_client::StdioMcpClient;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn http_transport_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_transport_end_to_end")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if !tools.iter().any(|tool| tool.name == ToolName::ScenarioDefine) {
        return Err("tools/list missing scenario_define".into());
    }

    let mut fixture = ScenarioFixture::time_after("transport-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let _output: decision_gate_mcp::tools::ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http transport responded to tools/list and tools/call".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_transport_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("stdio_transport_end_to_end")?;
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[server]
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
"#;
    std::fs::write(&config_path, config_contents)?;

    let stderr_path = reporter.artifacts().root().join("mcp.stderr.log");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
    wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("stdio-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output = client.call_tool("scenario_define", define_input).await?;
    let define_response: ScenarioDefineResponse = serde_json::from_value(define_output)?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_response.scenario_id,
        run_config: fixture.run_config(),
        started_at: decision_gate_core::Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        serde_json::from_value(client.call_tool("scenario_start", start_input).await?)?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["stdio transport responded to tools/list and tools/call".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "mcp.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn docs_resources_http_list_read() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("docs_resources_http_list_read")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config_with_bearer(&bind, "docs-token");
    let server = spawn_mcp_server(config).await?;

    let unauthorized = server.client(Duration::from_secs(5))?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token("docs-token".to_string());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let Err(err) = unauthorized.list_resources().await else {
        return Err("expected unauthenticated error for resources/list".into());
    };
    if !err.contains("unauthenticated") {
        return Err(format!("unexpected unauthenticated error: {err}").into());
    }

    let resources = client.list_resources().await?;
    if resources.is_empty() {
        return Err("resources/list returned empty list".into());
    }
    let evidence = resources
        .iter()
        .find(|entry| entry.uri == "decision-gate://docs/evidence-flow")
        .ok_or_else(|| "missing evidence-flow resource".to_string())?;
    if !evidence.uri.starts_with(RESOURCE_URI_PREFIX) {
        return Err("resource uri missing decision-gate://docs/ prefix".into());
    }
    if evidence.name.trim().is_empty() || evidence.description.trim().is_empty() {
        return Err("resource metadata missing name or description".into());
    }
    if evidence.mime_type != "text/markdown" {
        return Err(format!("unexpected resource mime type: {}", evidence.mime_type).into());
    }

    let content = client.read_resource(&evidence.uri).await?;
    if !content.text.contains("# Evidence Flow + Execution Model") {
        return Err("resource body missing expected heading".into());
    }

    let Err(err) = client.read_resource("decision-gate://docs/missing").await else {
        return Err("expected error for missing resource".into());
    };
    if !err.contains("unknown resource uri") {
        return Err(format!("unexpected missing resource error: {err}").into());
    }

    let mut transcript = unauthorized.transcript();
    transcript.extend(client.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["resources/list and resources/read succeed with auth".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}
