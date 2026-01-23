// system-tests/tests/reliability.rs
// ============================================================================
// Module: Reliability Tests
// Description: Determinism and idempotency checks.
// Purpose: Validate trigger idempotency and replay-safe outputs.
// Dependencies: system-tests helpers
// ============================================================================

//! Reliability tests for Decision Gate system-tests.

mod helpers;

use decision_gate_core::DecisionOutcome;
use decision_gate_core::PacketPayload;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioStatus;
use decision_gate_core::StatusRequest;
use decision_gate_core::SubmissionRecord;
use decision_gate_core::SubmitRequest;
use decision_gate_core::SubmitResult;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioSubmitRequest;
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
        payload: None,
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

#[tokio::test(flavor = "multi_thread")]
async fn idempotent_submission() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("idempotent_submission")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("idempotent-submission", "run-1", 0);

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

    let submit = SubmitRequest {
        run_id: fixture.run_id.clone(),
        submission_id: "submission-1".to_string(),
        payload: PacketPayload::Json {
            value: serde_json::json!({"artifact": "alpha"}),
        },
        content_type: "application/json".to_string(),
        submitted_at: Timestamp::Logical(2),
        correlation_id: None,
    };
    let submit_request = ScenarioSubmitRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: submit.clone(),
    };
    let submit_input = serde_json::to_value(&submit_request)?;
    let first: SubmitResult = client.call_tool_typed("scenario_submit", submit_input).await?;

    let second_request = ScenarioSubmitRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: submit,
    };
    let second_input = serde_json::to_value(&second_request)?;
    let second: SubmitResult = client.call_tool_typed("scenario_submit", second_input).await?;

    assert_eq!(first.record, second.record);

    let conflict_request = ScenarioSubmitRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: SubmitRequest {
            run_id: fixture.run_id.clone(),
            submission_id: "submission-1".to_string(),
            payload: PacketPayload::Json {
                value: serde_json::json!({"artifact": "beta"}),
            },
            content_type: "application/json".to_string(),
            submitted_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let conflict_input = serde_json::to_value(&conflict_request)?;
    let conflict_error =
        client.call_tool("scenario_submit", conflict_input).await.err().unwrap_or_default();
    assert!(
        conflict_error.contains("submission_id conflict"),
        "unexpected conflict error: {conflict_error}"
    );

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id,
        run_id: fixture.run_id.clone(),
        output_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(4),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let submissions_path = runpack_dir.join("artifacts/submissions.json");
    let submissions_bytes = std::fs::read(&submissions_path)?;
    let submissions: Vec<SubmissionRecord> = serde_json::from_slice(&submissions_bytes)?;
    assert_eq!(submissions.len(), 1);
    assert_eq!(submissions[0].submission_id, "submission-1");
    assert_eq!(submissions[0].content_hash, first.record.content_hash);

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec![
            "idempotent submissions return the existing record".to_string(),
            "conflicting submission_id returns a conflict error".to_string(),
        ],
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
async fn timeout_policies() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("timeout_policies")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fail_fixture = ScenarioFixture::timeout_fail("timeout-fail", "run-fail", 5);
    let fail_define = ScenarioDefineRequest {
        spec: fail_fixture.spec.clone(),
    };
    let fail_define_input = serde_json::to_value(&fail_define)?;
    let fail_defined: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", fail_define_input).await?;

    let fail_start = ScenarioStartRequest {
        scenario_id: fail_defined.scenario_id.clone(),
        run_config: fail_fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let fail_start_input = serde_json::to_value(&fail_start)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", fail_start_input).await?;

    let fail_trigger = ScenarioTriggerRequest {
        scenario_id: fail_defined.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: fail_fixture.run_id.clone(),
            trigger_id: TriggerId::new("tick-1"),
            kind: TriggerKind::Tick,
            time: Timestamp::Logical(10),
            source_id: "scheduler".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let fail_trigger_input = serde_json::to_value(&fail_trigger)?;
    let fail_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", fail_trigger_input).await?;

    assert_eq!(fail_result.status, RunStatus::Failed);
    match fail_result.decision.outcome {
        DecisionOutcome::Fail {
            reason,
        } => assert_eq!(reason, "timeout"),
        outcome => {
            return Err(format!("unexpected timeout fail outcome: {outcome:?}").into());
        }
    }

    let advance_fixture = ScenarioFixture::timeout_advance("timeout-advance", "run-advance", 5);
    let advance_define = ScenarioDefineRequest {
        spec: advance_fixture.spec.clone(),
    };
    let advance_define_input = serde_json::to_value(&advance_define)?;
    let advance_defined: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", advance_define_input).await?;

    let advance_start = ScenarioStartRequest {
        scenario_id: advance_defined.scenario_id.clone(),
        run_config: advance_fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let advance_start_input = serde_json::to_value(&advance_start)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", advance_start_input).await?;

    let advance_trigger = ScenarioTriggerRequest {
        scenario_id: advance_defined.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: advance_fixture.run_id.clone(),
            trigger_id: TriggerId::new("tick-2"),
            kind: TriggerKind::Tick,
            time: Timestamp::Logical(10),
            source_id: "scheduler".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let advance_trigger_input = serde_json::to_value(&advance_trigger)?;
    let advance_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", advance_trigger_input).await?;

    assert_eq!(advance_result.status, RunStatus::Active);
    match advance_result.decision.outcome {
        DecisionOutcome::Advance {
            from_stage,
            to_stage,
            timeout,
        } => {
            assert!(timeout);
            assert_eq!(from_stage.as_str(), "stage-1");
            assert_eq!(to_stage.as_str(), "stage-2");
        }
        outcome => {
            return Err(format!("unexpected advance timeout outcome: {outcome:?}").into());
        }
    }

    let advance_status = ScenarioStatusRequest {
        scenario_id: advance_defined.scenario_id.clone(),
        request: StatusRequest {
            run_id: advance_fixture.run_id.clone(),
            requested_at: Timestamp::Logical(11),
            correlation_id: None,
        },
    };
    let advance_status_input = serde_json::to_value(&advance_status)?;
    let advance_snapshot: ScenarioStatus =
        client.call_tool_typed("scenario_status", advance_status_input).await?;
    assert_eq!(advance_snapshot.current_stage_id.as_str(), "stage-2");

    let branch_fixture =
        ScenarioFixture::timeout_alternate_branch("timeout-branch", "run-branch", 5);
    let branch_define = ScenarioDefineRequest {
        spec: branch_fixture.spec.clone(),
    };
    let branch_define_input = serde_json::to_value(&branch_define)?;
    let branch_defined: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", branch_define_input).await?;

    let branch_start = ScenarioStartRequest {
        scenario_id: branch_defined.scenario_id.clone(),
        run_config: branch_fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let branch_start_input = serde_json::to_value(&branch_start)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", branch_start_input).await?;

    let branch_trigger = ScenarioTriggerRequest {
        scenario_id: branch_defined.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: branch_fixture.run_id.clone(),
            trigger_id: TriggerId::new("tick-3"),
            kind: TriggerKind::Tick,
            time: Timestamp::Logical(10),
            source_id: "scheduler".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let branch_trigger_input = serde_json::to_value(&branch_trigger)?;
    let branch_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", branch_trigger_input).await?;

    assert_eq!(branch_result.status, RunStatus::Active);
    match branch_result.decision.outcome {
        DecisionOutcome::Advance {
            from_stage,
            to_stage,
            timeout,
        } => {
            assert!(timeout);
            assert_eq!(from_stage.as_str(), "stage-1");
            assert_eq!(to_stage.as_str(), "stage-alt");
        }
        outcome => {
            return Err(format!("unexpected alternate branch outcome: {outcome:?}").into());
        }
    }

    let branch_status = ScenarioStatusRequest {
        scenario_id: branch_defined.scenario_id,
        request: StatusRequest {
            run_id: branch_fixture.run_id,
            requested_at: Timestamp::Logical(11),
            correlation_id: None,
        },
    };
    let branch_status_input = serde_json::to_value(&branch_status)?;
    let branch_snapshot: ScenarioStatus =
        client.call_tool_typed("scenario_status", branch_status_input).await?;
    assert_eq!(branch_snapshot.current_stage_id.as_str(), "stage-alt");

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec![
            "timeout fail policy marks the run failed".to_string(),
            "advance_with_flag advances with timeout=true".to_string(),
            "alternate_branch routes to the unknown branch".to_string(),
        ],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
