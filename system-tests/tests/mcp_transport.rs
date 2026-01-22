// system-tests/tests/mcp_transport.rs
// ============================================================================
// Module: MCP Transport Tests
// Description: Transport validation for HTTP JSON-RPC.
// Purpose: Ensure MCP server is reachable and responds to tools/list and tools/call.
// Dependencies: system-tests helpers
// ============================================================================

//! MCP transport tests for Decision Gate.

mod helpers;

use decision_gate_mcp::tools::ScenarioDefineRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;

#[tokio::test(flavor = "multi_thread")]
async fn http_transport_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_transport_end_to_end")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    let names: Vec<String> = tools.into_iter().map(|tool| tool.name).collect();
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
