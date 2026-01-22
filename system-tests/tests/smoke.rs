// system-tests/tests/smoke.rs
// ============================================================================
// Module: Smoke Tests
// Description: Fast end-to-end sanity checks for MCP tooling.
// Purpose: Validate basic scenario lifecycle over HTTP.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Smoke tests for the Decision Gate MCP HTTP surface.

mod helpers;

use decision_gate_core::RunStatus;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;

#[tokio::test(flavor = "multi_thread")]
async fn smoke_define_start_next_status() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("smoke_define_start_next_status")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("smoke-scenario", "run-1", 0);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let next_request = ScenarioNextRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: NextRequest {
            run_id: fixture.run_id.clone(),
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "agent-1".to_string(),
            time: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let next_input = serde_json::to_value(&next_request)?;
    let _next_result: decision_gate_core::runtime::NextResult =
        client.call_tool_typed("scenario_next", next_input).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: StatusRequest {
            run_id: fixture.run_id,
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    assert_eq!(status.status, RunStatus::Completed);

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["scenario lifecycle completed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
