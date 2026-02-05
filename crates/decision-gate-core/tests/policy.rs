// crates/decision-gate-core/tests/policy.rs
// ============================================================================
// Module: Policy Tests
// Description: Validate policy enforcement in the core control plane.
// Purpose: Ensure policy denial blocks dispatch targets deterministically.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

//! Policy behavior tests for dispatch authorization outcomes.

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
use decision_gate_core::NamespaceId;
use decision_gate_core::RunStateStore;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
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
use ret_logic::Requirement;
use serde_json::json;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Test keeps the full policy flow in one place for readability."
)]
fn policy_denies_dispatch_targets() -> Result<(), Box<dyn std::error::Error>> {
    let scenario_id = decision_gate_core::ScenarioId::new("policy-scenario");
    let stage1_id = decision_gate_core::StageId::new("stage-1");
    let stage2_id = decision_gate_core::StageId::new("stage-2");
    let condition_id = decision_gate_core::ConditionId::new("allow");
    let tenant_id = decision_gate_core::TenantId::from_raw(1)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "nonzero tenantid"))?;
    let namespace_id = NamespaceId::from_raw(1).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "nonzero namespaceid")
    })?;
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![
            decision_gate_core::StageSpec {
                stage_id: stage1_id,
                entry_packets: Vec::new(),
                gates: vec![decision_gate_core::GateSpec {
                    gate_id: decision_gate_core::GateId::new("gate-allow"),
                    requirement: Requirement::condition(condition_id.clone()),
                    trust: None,
                }],
                advance_to: decision_gate_core::AdvanceTo::Fixed {
                    stage_id: stage2_id.clone(),
                },
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            decision_gate_core::StageSpec {
                stage_id: stage2_id,
                entry_packets: vec![decision_gate_core::PacketSpec {
                    packet_id: decision_gate_core::PacketId::new("packet-1"),
                    schema_id: decision_gate_core::SchemaId::new("schema-1"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["internal".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: decision_gate_core::PacketPayload::Json {
                        value: json!({"hello": "world"}),
                    },
                }],
                gates: Vec::new(),
                advance_to: decision_gate_core::AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        conditions: vec![decision_gate_core::ConditionSpec {
            condition_id,
            query: decision_gate_core::EvidenceQuery {
                provider_id: decision_gate_core::ProviderId::new("stub"),
                check_id: "allow".to_string(),
                params: None,
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
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
        tenant_id,
        namespace_id,
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
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
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
            if reason != "policy denied disclosure" {
                return Err(format!("expected policy denial reason, got {reason}").into());
            }
        }
        other => {
            return Err(format!("unexpected decision outcome: {other:?}").into());
        }
    }

    let state = store
        .load(&run_config.tenant_id, &run_config.namespace_id, &run_config.run_id)?
        .ok_or("missing run state")?;
    if state.tool_calls.len() != 1 {
        return Err(format!("expected 1 tool call, got {}", state.tool_calls.len()).into());
    }
    if !state.packets.is_empty() {
        return Err("expected no packets after policy denial".into());
    }
    if dispatcher.dispatch_count() != 0 {
        return Err(format!("expected 0 dispatches, got {}", dispatcher.dispatch_count()).into());
    }

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
            lane: TrustLane::Verified,
            error: None,
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
        drop(guard);
        Ok(DispatchReceipt {
            dispatch_id: "dispatch-1".to_string(),
            target: target.clone(),
            receipt_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"dispatch"),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "counting-dispatcher".to_string(),
        })
    }
}
