// system-tests/tests/security.rs
// ============================================================================
// Module: Security Tests
// Description: Evidence redaction and disclosure metadata validation.
// Purpose: Confirm security posture defaults and visibility propagation.
// Dependencies: system-tests helpers
// ============================================================================

//! Security posture tests for Decision Gate system-tests.

mod helpers;

use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::RunStateStore;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::interfaces::DispatchError;
use decision_gate_core::interfaces::Dispatcher;
use decision_gate_core::interfaces::PolicyDecider;
use decision_gate_core::interfaces::PolicyDecision;
use decision_gate_core::interfaces::PolicyError;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::TriggerResult;
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

#[test]
fn policy_denies_dispatch_targets() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("policy_denies_dispatch_targets")?;
    let scenario_id = decision_gate_core::ScenarioId::new("policy-scenario");
    let stage1_id = decision_gate_core::StageId::new("stage-1");
    let stage2_id = decision_gate_core::StageId::new("stage-2");
    let predicate_key = decision_gate_core::PredicateKey::new("allow");
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: scenario_id.clone(),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![
            decision_gate_core::StageSpec {
                stage_id: stage1_id.clone(),
                entry_packets: Vec::new(),
                gates: vec![decision_gate_core::GateSpec {
                    gate_id: decision_gate_core::GateId::new("gate-allow"),
                    requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
                }],
                advance_to: decision_gate_core::AdvanceTo::Fixed {
                    stage_id: stage2_id.clone(),
                },
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            decision_gate_core::StageSpec {
                stage_id: stage2_id.clone(),
                entry_packets: vec![decision_gate_core::PacketSpec {
                    packet_id: decision_gate_core::PacketId::new("packet-1"),
                    schema_id: decision_gate_core::SchemaId::new("schema-1"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["internal".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: decision_gate_core::PacketPayload::Json {
                        value: serde_json::json!({"hello": "world"}),
                    },
                }],
                gates: Vec::new(),
                advance_to: decision_gate_core::AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        predicates: vec![decision_gate_core::PredicateSpec {
            predicate: predicate_key,
            query: decision_gate_core::EvidenceQuery {
                provider_id: decision_gate_core::ProviderId::new("stub"),
                predicate: "allow".to_string(),
                params: None,
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(serde_json::json!(true)),
            policy_tags: Vec::new(),
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };

    let evidence = StubEvidence;
    let dispatcher = CountingDispatcher::new();
    let deny_policy = DenyPolicy;
    let store = InMemoryRunStateStore::new();
    let control = ControlPlane::new(
        spec,
        evidence,
        dispatcher.clone(),
        store.clone(),
        Some(deny_policy),
        ControlPlaneConfig::default(),
    )?;

    let mut run_config = decision_gate_core::RunConfig {
        tenant_id: decision_gate_core::TenantId::new("tenant-1"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id,
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    run_config.dispatch_targets = vec![DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    }];

    control.start_run(run_config.clone(), Timestamp::Logical(1), false)?;

    let trigger = TriggerEvent {
        run_id: run_config.run_id.clone(),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(2),
        source_id: "policy".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result: TriggerResult = control.trigger(&trigger)?;
    match result.decision.outcome {
        decision_gate_core::DecisionOutcome::Fail {
            reason,
        } => {
            assert_eq!(reason, "policy denied disclosure");
        }
        other => panic!("unexpected decision outcome: {other:?}"),
    }

    let state = store.load(&run_config.run_id)?.ok_or("missing run state")?;
    assert_eq!(state.tool_calls.len(), 1);
    assert!(state.packets.is_empty());
    assert_eq!(dispatcher.dispatch_count(), 0);

    reporter.artifacts().write_json("tool_transcript.json", &state.tool_calls)?;
    reporter.artifacts().write_json("policy_state.json", &state)?;
    reporter.finish(
        "pass",
        vec!["policy denied disclosure as expected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "policy_state.json".to_string(),
        ],
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct DenyPolicy;

impl PolicyDecider for DenyPolicy {
    fn authorize(
        &self,
        _target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &decision_gate_core::PacketPayload,
    ) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Deny)
    }
}

#[derive(Clone, Debug)]
struct StubEvidence;

impl EvidenceProvider for StubEvidence {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(serde_json::Value::Bool(true))),
            evidence_hash: Some(hash_bytes(DEFAULT_HASH_ALGORITHM, b"stub")),
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: Some("application/json".to_string()),
        })
    }

    fn validate_providers(
        &self,
        _spec: &decision_gate_core::ScenarioSpec,
    ) -> Result<(), decision_gate_core::ProviderMissingError> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct CountingDispatcher {
    count: Arc<Mutex<u64>>,
}

impl CountingDispatcher {
    fn new() -> Self {
        Self {
            count: Arc::new(Mutex::new(0)),
        }
    }

    fn dispatch_count(&self) -> u64 {
        self.count.lock().map_or(0, |count| *count)
    }
}

impl Dispatcher for CountingDispatcher {
    fn dispatch(
        &self,
        target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &decision_gate_core::PacketPayload,
    ) -> Result<DispatchReceipt, DispatchError> {
        let mut guard = self.count.lock().map_err(|_| {
            DispatchError::DispatchFailed("dispatch count lock poisoned".to_string())
        })?;
        *guard = guard.saturating_add(1);
        Ok(DispatchReceipt {
            dispatch_id: "dispatch-1".to_string(),
            target: target.clone(),
            receipt_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"dispatch"),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "counting-dispatcher".to_string(),
        })
    }
}
