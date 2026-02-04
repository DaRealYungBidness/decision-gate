// system-tests/tests/suites/reliability.rs
// ============================================================================
// Module: Reliability Tests
// Description: Determinism and idempotency checks.
// Purpose: Validate trigger idempotency and replay-safe outputs.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Determinism and idempotency checks.
//! Purpose: Validate trigger idempotency and replay-safe outputs.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

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

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn idempotent_trigger() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("idempotent_trigger")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("idempotent-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
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

    require_eq(
        &first.decision.decision_id,
        &second.decision.decision_id,
        "decision ids differ on replay",
    )?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id,
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
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

    require(triggers.len() == 1, format!("expected 1 trigger, got {}", triggers.len()))?;
    require(decisions.len() == 1, format!("expected 1 decision, got {}", decisions.len()))?;
    let decision = decisions.first().ok_or_else(|| "missing decision record".to_string())?;
    require_eq(&decision.trigger_id.as_str(), &"trigger-1", "decision trigger id mismatch")?;

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
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Test exercises multi-step submission flow.")]
async fn idempotent_submission() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("idempotent_submission")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("idempotent-submission", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
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

    require_eq(&first.record, &second.record, "submission record mismatch on replay")?;

    let conflict_request = ScenarioSubmitRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: SubmitRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
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
    let Err(conflict_error) = client.call_tool("scenario_submit", conflict_input).await else {
        return Err("expected submission conflict".into());
    };
    require(
        conflict_error.contains("submission_id conflict"),
        format!("unexpected conflict error: {conflict_error}"),
    )?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id,
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
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
    require(submissions.len() == 1, format!("expected 1 submission, got {}", submissions.len()))?;
    let submission = submissions.first().ok_or_else(|| "missing submission record".to_string())?;
    require_eq(&submission.submission_id.as_str(), &"submission-1", "submission id mismatch")?;
    require_eq(
        &submission.content_hash,
        &first.record.content_hash,
        "submission content hash mismatch",
    )?;

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
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Test covers multiple timeout policies in one flow.")]
async fn timeout_policies() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("timeout_policies")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fail_fixture = ScenarioFixture::timeout_fail("timeout-fail", "run-fail", 5);
    fail_fixture.spec.default_tenant_id = Some(fail_fixture.tenant_id);
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
            tenant_id: fail_fixture.tenant_id,
            namespace_id: fail_fixture.namespace_id,
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

    require_eq(&fail_result.status, &RunStatus::Failed, "timeout fail status mismatch")?;
    match fail_result.decision.outcome {
        DecisionOutcome::Fail {
            reason,
        } => {
            if reason != "timeout" {
                return Err(format!("expected timeout reason, got {reason}").into());
            }
        }
        outcome => {
            return Err(format!("unexpected timeout fail outcome: {outcome:?}").into());
        }
    }

    let mut advance_fixture = ScenarioFixture::timeout_advance("timeout-advance", "run-advance", 5);
    advance_fixture.spec.default_tenant_id = Some(advance_fixture.tenant_id);
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
            tenant_id: advance_fixture.tenant_id,
            namespace_id: advance_fixture.namespace_id,
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

    require_eq(&advance_result.status, &RunStatus::Active, "advance timeout status mismatch")?;
    match advance_result.decision.outcome {
        DecisionOutcome::Advance {
            from_stage,
            to_stage,
            timeout,
        } => {
            if !timeout {
                return Err("expected timeout flag for advance".into());
            }
            require_eq(&from_stage.as_str(), &"stage-1", "advance from stage mismatch")?;
            require_eq(&to_stage.as_str(), &"stage-2", "advance to stage mismatch")?;
        }
        outcome => {
            return Err(format!("unexpected advance timeout outcome: {outcome:?}").into());
        }
    }

    let advance_status = ScenarioStatusRequest {
        scenario_id: advance_defined.scenario_id.clone(),
        request: StatusRequest {
            run_id: advance_fixture.run_id.clone(),
            tenant_id: advance_fixture.tenant_id,
            namespace_id: advance_fixture.namespace_id,
            requested_at: Timestamp::Logical(11),
            correlation_id: None,
        },
    };
    let advance_status_input = serde_json::to_value(&advance_status)?;
    let advance_snapshot: ScenarioStatus =
        client.call_tool_typed("scenario_status", advance_status_input).await?;
    require_eq(
        &advance_snapshot.current_stage_id.as_str(),
        &"stage-2",
        "advance snapshot stage mismatch",
    )?;

    let mut branch_fixture =
        ScenarioFixture::timeout_alternate_branch("timeout-branch", "run-branch", 5);
    branch_fixture.spec.default_tenant_id = Some(branch_fixture.tenant_id);
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
            tenant_id: branch_fixture.tenant_id,
            namespace_id: branch_fixture.namespace_id,
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

    require_eq(&branch_result.status, &RunStatus::Active, "alternate branch status mismatch")?;
    match branch_result.decision.outcome {
        DecisionOutcome::Advance {
            from_stage,
            to_stage,
            timeout,
        } => {
            if !timeout {
                return Err("expected timeout flag for alternate branch".into());
            }
            require_eq(&from_stage.as_str(), &"stage-1", "branch from stage mismatch")?;
            require_eq(&to_stage.as_str(), &"stage-alt", "branch to stage mismatch")?;
        }
        outcome => {
            return Err(format!("unexpected alternate branch outcome: {outcome:?}").into());
        }
    }

    let branch_status = ScenarioStatusRequest {
        scenario_id: branch_defined.scenario_id,
        request: StatusRequest {
            run_id: branch_fixture.run_id,
            tenant_id: branch_fixture.tenant_id,
            namespace_id: branch_fixture.namespace_id,
            requested_at: Timestamp::Logical(11),
            correlation_id: None,
        },
    };
    let branch_status_input = serde_json::to_value(&branch_status)?;
    let branch_snapshot: ScenarioStatus =
        client.call_tool_typed("scenario_status", branch_status_input).await?;
    require_eq(
        &branch_snapshot.current_stage_id.as_str(),
        &"stage-alt",
        "branch snapshot stage mismatch",
    )?;

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
    drop(reporter);
    Ok(())
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
    if condition { Ok(()) } else { Err(message.into().into()) }
}

fn require_eq<T: PartialEq + std::fmt::Debug>(
    left: &T,
    right: &T,
    context: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if left == right {
        Ok(())
    } else {
        Err(format!("{context}: left={left:?} right={right:?}").into())
    }
}
