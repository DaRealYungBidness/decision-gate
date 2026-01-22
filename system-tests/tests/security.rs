// system-tests/tests/security.rs
// ============================================================================
// Module: Security Tests
// Description: Evidence redaction and disclosure metadata validation.
// Purpose: Confirm security posture defaults and visibility propagation.
// Dependencies: system-tests helpers
// ============================================================================

//! Security posture tests for Decision Gate system-tests.

mod helpers;

use decision_gate_core::Timestamp;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::EvidenceQueryResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;

#[tokio::test(flavor = "multi_thread")]
async fn evidence_redaction_default() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("evidence_redaction_default")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("redaction-scenario", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: decision_gate_core::EvidenceQuery {
            provider_id: decision_gate_core::ProviderId::new("time"),
            predicate: "now".to_string(),
            params: None,
        },
        context: fixture.evidence_context("trigger-ctx", Timestamp::Logical(10)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;

    assert!(response.result.value.is_none());
    assert!(response.result.content_type.is_none());
    assert!(response.result.evidence_hash.is_some());

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["raw evidence redacted by default".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn packet_disclosure_visibility() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("packet_disclosure_visibility")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::with_visibility_packet(
        "visibility-scenario",
        "run-1",
        vec!["confidential".to_string(), "restricted".to_string()],
        vec!["policy-alpha".to_string()],
    );

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
        issue_entry_packets: true,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    assert_eq!(state.packets.len(), 1);
    let envelope = &state.packets[0].envelope;
    assert_eq!(envelope.visibility.labels, vec!["confidential", "restricted"]);
    assert_eq!(envelope.visibility.policy_tags, vec!["policy-alpha"]);

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["packet visibility metadata persisted".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
