// system-tests/tests/suites/security.rs
// ============================================================================
// Module: Security Tests
// Description: Evidence redaction and disclosure metadata validation.
// Purpose: Confirm security posture defaults and visibility propagation.
// Dependencies: system-tests helpers
// ============================================================================

//! Security posture tests for Decision Gate system-tests.

use std::num::NonZeroU64;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DispatchTarget;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerKind;
use decision_gate_mcp::config::ServerMode;
use decision_gate_mcp::policy::PolicyEffect;
use decision_gate_mcp::policy::PolicyEngine;
use decision_gate_mcp::policy::PolicyRule;
use decision_gate_mcp::policy::StaticPolicyConfig;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::EvidenceQueryResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::TranscriptEntry;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use reqwest::Client;
use reqwest::StatusCode;
use ret_logic::Requirement;
use serde_json::json;

use crate::helpers;

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
            check_id: "now".to_string(),
            params: None,
        },
        context: fixture.evidence_context("trigger-ctx", Timestamp::Logical(10)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;

    if response.result.value.is_some() {
        return Err("expected evidence value to be redacted".into());
    }
    if response.result.content_type.is_some() {
        return Err("expected evidence content type to be redacted".into());
    }
    if response.result.evidence_hash.is_none() {
        return Err("expected evidence hash to be present".into());
    }

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
    drop(reporter);
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

    let mut fixture = ScenarioFixture::with_visibility_packet(
        "visibility-scenario",
        "run-1",
        vec!["confidential".to_string(), "restricted".to_string()],
        vec!["policy-alpha".to_string()],
    );
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
        issue_entry_packets: true,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    if state.packets.len() != 1 {
        return Err(format!("expected 1 packet, got {}", state.packets.len()).into());
    }
    let envelope =
        &state.packets.first().ok_or_else(|| "missing packet envelope".to_string())?.envelope;
    if envelope.visibility.labels != vec!["confidential", "restricted"] {
        return Err(format!("unexpected labels: {:?}", envelope.visibility.labels).into());
    }
    if envelope.visibility.policy_tags != vec!["policy-alpha"] {
        return Err(format!("unexpected policy tags: {:?}", envelope.visibility.policy_tags).into());
    }

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
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn strict_mode_rejects_default_namespace() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("strict_mode_rejects_default_namespace")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.server.mode = ServerMode::Strict;
    config.namespace.allow_default = false;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("strict-default", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let Err(error) = client.call_tool("scenario_define", define_input).await else {
        return Err("expected scenario_define to be rejected".into());
    };
    if !error.contains("unauthorized") {
        return Err(format!("unexpected error: {error}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["default namespace rejected in strict mode".to_string()],
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
async fn invalid_correlation_id_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("invalid_correlation_id_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let http_client = Client::builder().timeout(std::time::Duration::from_secs(5)).build()?;
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
    });
    let response = http_client
        .post(server.base_url())
        .header("x-correlation-id", "bad value")
        .json(&request)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let payload: serde_json::Value = response.json().await?;

    if status != StatusCode::BAD_REQUEST {
        return Err(format!("expected 400, got {status}").into());
    }
    let code = payload
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(serde_json::Value::as_i64);
    if code != Some(-32073) {
        return Err(format!("expected error code -32073, got {code:?}").into());
    }
    let server_corr = headers.get("x-server-correlation-id").and_then(|value| value.to_str().ok());
    if server_corr.is_none() {
        return Err("expected x-server-correlation-id response header".into());
    }
    if headers.get("x-correlation-id").is_some() {
        return Err("invalid x-correlation-id should not be echoed".into());
    }

    let error_message = payload
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let transcript = vec![TranscriptEntry {
        sequence: 1,
        method: "tools/list".to_string(),
        request: request.clone(),
        response: payload,
        error: error_message,
    }];
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["invalid correlation id rejected with server correlation header".to_string()],
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
#[allow(
    clippy::too_many_lines,
    reason = "End-to-end policy denial coverage is clearer in one test."
)]
async fn policy_denies_dispatch_targets() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("policy_denies_dispatch_targets")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.policy.engine = PolicyEngine::Static;
    config.policy.static_policy = Some(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Deny,
            error_message: None,
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: vec!["internal".to_string()],
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let scenario_id = ScenarioId::new("policy-scenario");
    let namespace_id = NamespaceId::new(NonZeroU64::MIN);
    let stage1_id = StageId::new("stage-1");
    let stage2_id = StageId::new("stage-2");
    let condition_id = ConditionId::new("after");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: stage1_id.clone(),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-allow"),
                    requirement: Requirement::condition(condition_id.clone()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Fixed {
                    stage_id: stage2_id.clone(),
                },
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: stage2_id.clone(),
                entry_packets: vec![PacketSpec {
                    packet_id: PacketId::new("packet-1"),
                    schema_id: decision_gate_core::SchemaId::new("schema-1"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["internal".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: PacketPayload::Json {
                        value: json!({"hello": "world"}),
                    },
                }],
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        conditions: vec![ConditionSpec {
            condition_id,
            query: decision_gate_core::EvidenceQuery {
                provider_id: ProviderId::new("time"),
                check_id: "after".to_string(),
                params: Some(json!({"timestamp": 0})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(TenantId::new(NonZeroU64::MIN)),
    };

    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let mut run_config = decision_gate_core::RunConfig {
        tenant_id: TenantId::new(NonZeroU64::MIN),
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: define_output.scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    run_config.dispatch_targets = vec![DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    }];

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: TriggerEvent {
            run_id: decision_gate_core::RunId::new("run-1"),
            tenant_id: TenantId::new(NonZeroU64::MIN),
            namespace_id: NamespaceId::new(NonZeroU64::MIN),
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "policy".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match &trigger_result.decision.outcome {
        DecisionOutcome::Fail {
            reason,
        } => {
            if reason != "policy denied disclosure" {
                return Err(format!("expected policy denial reason, got {reason}").into());
            }
        }
        other => return Err(format!("unexpected decision outcome: {other:?}").into()),
    }
    if trigger_result.status != RunStatus::Failed {
        return Err(format!("expected failed status, got {:?}", trigger_result.status).into());
    }
    if !trigger_result.packets.is_empty() {
        return Err("expected no packets after policy denial".into());
    }

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: decision_gate_core::runtime::StatusRequest {
            run_id: decision_gate_core::RunId::new("run-1"),
            tenant_id: TenantId::new(NonZeroU64::MIN),
            namespace_id: NamespaceId::new(NonZeroU64::MIN),
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;
    if status.status != RunStatus::Failed {
        return Err(format!("expected failed run status, got {:?}", status.status).into());
    }

    let policy_state = serde_json::json!({
        "trigger_result": trigger_result,
        "status": status,
    });

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.artifacts().write_json("policy_state.json", &policy_state)?;
    reporter.finish(
        "pass",
        vec!["policy denial blocks dispatch targets".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "policy_state.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "End-to-end policy error coverage is clearer in one test."
)]
async fn policy_error_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("policy_error_fails_closed")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.policy.engine = PolicyEngine::Static;
    config.policy.static_policy = Some(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Error,
            error_message: Some("policy error".to_string()),
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: vec!["internal".to_string()],
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let scenario_id = ScenarioId::new("policy-error-scenario");
    let namespace_id = NamespaceId::new(NonZeroU64::MIN);
    let stage1_id = StageId::new("stage-1");
    let stage2_id = StageId::new("stage-2");
    let condition_id = ConditionId::new("after");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: stage1_id.clone(),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-allow"),
                    requirement: Requirement::condition(condition_id.clone()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Fixed {
                    stage_id: stage2_id.clone(),
                },
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: stage2_id.clone(),
                entry_packets: vec![PacketSpec {
                    packet_id: PacketId::new("packet-1"),
                    schema_id: decision_gate_core::SchemaId::new("schema-1"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["internal".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: PacketPayload::Json {
                        value: json!({"hello": "world"}),
                    },
                }],
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        conditions: vec![ConditionSpec {
            condition_id,
            query: decision_gate_core::EvidenceQuery {
                provider_id: ProviderId::new("time"),
                check_id: "after".to_string(),
                params: Some(json!({"timestamp": 0})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(TenantId::new(NonZeroU64::MIN)),
    };

    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let mut run_config = decision_gate_core::RunConfig {
        tenant_id: TenantId::new(NonZeroU64::MIN),
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: define_output.scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    run_config.dispatch_targets = vec![DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    }];

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: TriggerEvent {
            run_id: decision_gate_core::RunId::new("run-1"),
            tenant_id: TenantId::new(NonZeroU64::MIN),
            namespace_id: NamespaceId::new(NonZeroU64::MIN),
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "policy".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match &trigger_result.decision.outcome {
        DecisionOutcome::Fail {
            reason,
        } => {
            if !reason.contains("policy decision error") {
                return Err(format!("expected policy error reason, got {reason}").into());
            }
        }
        other => return Err(format!("unexpected decision outcome: {other:?}").into()),
    }
    if trigger_result.status != RunStatus::Failed {
        return Err(format!("expected failed status, got {:?}", trigger_result.status).into());
    }
    if !trigger_result.packets.is_empty() {
        return Err("expected no packets after policy error".into());
    }

    let policy_state = serde_json::json!({
        "trigger_result": trigger_result,
    });

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.artifacts().write_json("policy_state.json", &policy_state)?;
    reporter.finish(
        "pass",
        vec!["policy error fails closed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "policy_state.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
