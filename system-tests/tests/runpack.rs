// system-tests/tests/runpack.rs
// ============================================================================
// Module: Runpack Tests
// Description: Runpack export and verification coverage.
// Purpose: Ensure runpack integrity checks are enforced.
// Dependencies: system-tests helpers
// ============================================================================

//! Runpack validation tests for Decision Gate system-tests.

mod helpers;

use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackVerifyRequest;
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
async fn runpack_export_verify_happy_path() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_export_verify_happy_path")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("runpack-scenario", "run-1", 0);

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

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        run_id: fixture.run_id.clone(),
        output_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verified: decision_gate_mcp::tools::RunpackVerifyResponse =
        client.call_tool_typed("runpack_verify", verify_input).await?;

    if verified.status != decision_gate_core::runtime::VerificationStatus::Pass {
        return Err(format!("expected verification pass, got {:?}", verified.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["runpack verification passed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn runpack_tamper_detection() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_tamper_detection")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("runpack-tamper", "run-1", 0);

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

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        run_id: fixture.run_id.clone(),
        output_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let tamper_path = runpack_dir.join("artifacts/triggers.json");
    let mut bytes = std::fs::read(&tamper_path)?;
    bytes.extend_from_slice(b"tampered");
    std::fs::write(&tamper_path, bytes)?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verified: decision_gate_mcp::tools::RunpackVerifyResponse =
        client.call_tool_typed("runpack_verify", verify_input).await?;

    if verified.status != decision_gate_core::runtime::VerificationStatus::Fail {
        return Err(format!("expected verification fail, got {:?}", verified.status).into());
    }
    if verified.report.errors.is_empty() {
        return Err("expected verification errors after tampering".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["tampered runpack rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    Ok(())
}
