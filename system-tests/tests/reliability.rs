// system-tests/tests/reliability.rs
// ============================================================================
// Module: Reliability Tests
// Description: Determinism and idempotency checks.
// Purpose: Validate trigger idempotency and replay-safe outputs.
// Dependencies: system-tests helpers
// ============================================================================

//! Reliability tests for Decision Gate system-tests.

mod helpers;

use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;

#[tokio::test(flavor = "multi_thread")]
async fn idempotent_trigger() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("idempotent_trigger")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("idempotent-scenario", "run-1", 0);

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

    let trigger_event = decision_gate_core::TriggerEvent {
        run_id: fixture.run_id.clone(),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(2),
        source_id: "idempotent".to_string(),
        payload_ref: None,
        correlation_id: None,
    };
    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: trigger_event.clone(),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let first: TriggerResult = client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let second_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: trigger_event,
    };
    let second_input = serde_json::to_value(&second_request)?;
    let second: TriggerResult = client.call_tool_typed("scenario_trigger", second_input).await?;

    assert_eq!(first.decision.decision_id, second.decision.decision_id);

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id,
        run_id: fixture.run_id.clone(),
        output_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let triggers_path = runpack_dir.join("artifacts/triggers.json");
    let decisions_path = runpack_dir.join("artifacts/decisions.json");
    let triggers_bytes = std::fs::read(&triggers_path)?;
    let decisions_bytes = std::fs::read(&decisions_path)?;
    let triggers: Vec<decision_gate_core::TriggerRecord> = serde_json::from_slice(&triggers_bytes)?;
    let decisions: Vec<decision_gate_core::DecisionRecord> =
        serde_json::from_slice(&decisions_bytes)?;

    assert_eq!(triggers.len(), 1);
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].trigger_id.as_str(), "trigger-1");

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["idempotent trigger did not duplicate decisions".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    Ok(())
}
