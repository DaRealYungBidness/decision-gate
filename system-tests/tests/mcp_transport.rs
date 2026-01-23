// system-tests/tests/mcp_transport.rs
// ============================================================================
// Module: MCP Transport Tests
// Description: Transport validation for HTTP JSON-RPC.
// Purpose: Ensure MCP server is reachable and responds to tools/list and tools/call.
// Dependencies: system-tests helpers
// ============================================================================

//! MCP transport tests for Decision Gate.

mod helpers;

use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::stdio_client::StdioMcpClient;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
async fn http_transport_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_transport_end_to_end")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    let names: Vec<String> = tools.into_iter().map(|tool| tool.name.as_str().to_string()).collect();
    assert!(names.contains(&"scenario_define".to_string()));

    let fixture = ScenarioFixture::time_after("transport-scenario", "run-1", 0);
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
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_transport_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("stdio_transport_end_to_end")?;
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    std::fs::write(&config_path, "[server]\ntransport = \"stdio\"\n")?;

    let stderr_path = reporter.artifacts().root().join("mcp.stderr.log");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;

    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        match client.list_tools().await {
            Ok(_) => break,
            Err(err) => {
                if start.elapsed() > timeout {
                    return Err(format!("stdio readiness timeout: {err}").into());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    let fixture = ScenarioFixture::time_after("stdio-scenario", "run-1", 0);
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
    Ok(())
}
