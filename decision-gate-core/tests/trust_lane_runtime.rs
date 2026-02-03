// decision-gate-core/tests/trust_lane_runtime.rs
// ============================================================================
// Module: Trust Lane Runtime Tests
// Description: Runtime trust enforcement for evidence lanes.
// Purpose: Ensure runtime evaluation enforces trust requirements and errors.
// Threat Models: TM-TRUST-001 (lane bypass), TM-TRUST-002 (signature forgery)
// ============================================================================

//! Runtime trust lane enforcement tests for evidence evaluation.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions and helpers are permitted."
)]

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
use decision_gate_core::TrustRequirement;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use serde_json::json;

struct LaneProvider {
    lane: TrustLane,
}

impl EvidenceProvider for LaneProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: self.lane,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: Some("application/json".to_string()),
        })
    }

    fn validate_providers(
        &self,
        _spec: &ScenarioSpec,
    ) -> Result<(), decision_gate_core::ProviderMissingError> {
        Ok(())
    }
}

struct NoopDispatcher;

impl Dispatcher for NoopDispatcher {
    fn dispatch(
        &self,
        target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<DispatchReceipt, decision_gate_core::DispatchError> {
        Ok(DispatchReceipt {
            dispatch_id: "dispatch-1".to_string(),
            target: target.clone(),
            receipt_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"receipt"),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "noop".to_string(),
        })
    }
}

struct PermitAllPolicy;

impl PolicyDecider for PermitAllPolicy {
    fn authorize(
        &self,
        _target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<PolicyDecision, decision_gate_core::PolicyError> {
        Ok(PolicyDecision::Permit)
    }
}

fn base_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::condition("ready".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id: "ready".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("test"),
                check_id: "ready".to_string(),
                params: Some(json!({})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn start_run<P: EvidenceProvider>(
    engine: &ControlPlane<P, NoopDispatcher, InMemoryRunStateStore, PermitAllPolicy>,
) {
    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };
    engine.start_run(run_config, Timestamp::Logical(0), false).unwrap();
}

#[test]
fn runtime_trust_lane_violation_sets_error() {
    let provider = LaneProvider {
        lane: TrustLane::Asserted,
    };
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        base_spec(),
        provider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("tenant"),
        namespace_id: NamespaceId::from_raw(1).expect("namespace"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }

    let state = store
        .load(
            &TenantId::from_raw(1).expect("tenant"),
            &NamespaceId::from_raw(1).expect("namespace"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("state");
    let record = &state.gate_evals[0].evidence[0];
    let error = record.result.error.as_ref().expect("trust error");
    assert_eq!(error.code, "trust_lane");
}

#[test]
fn runtime_condition_trust_override_is_stricter() {
    let mut spec = base_spec();
    spec.conditions[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let provider = LaneProvider {
        lane: TrustLane::Asserted,
    };
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig {
            trust_requirement: TrustRequirement {
                min_lane: TrustLane::Asserted,
            },
            ..ControlPlaneConfig::default()
        },
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("tenant"),
        namespace_id: NamespaceId::from_raw(1).expect("namespace"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn runtime_gate_trust_override_is_stricter() {
    let mut spec = base_spec();
    spec.stages[0].gates[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let provider = LaneProvider {
        lane: TrustLane::Asserted,
    };
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        provider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig {
            trust_requirement: TrustRequirement {
                min_lane: TrustLane::Asserted,
            },
            ..ControlPlaneConfig::default()
        },
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("tenant"),
        namespace_id: NamespaceId::from_raw(1).expect("namespace"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }

    let state = store
        .load(
            &TenantId::from_raw(1).expect("tenant"),
            &NamespaceId::from_raw(1).expect("namespace"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("state");
    let record = &state.gate_evals[0].evidence[0];
    let error = record.result.error.as_ref().expect("trust error");
    assert_eq!(error.code, "trust_lane");
}

#[test]
fn runtime_provider_trust_override_cannot_relax_global_requirement() {
    let provider = LaneProvider {
        lane: TrustLane::Asserted,
    };
    let store = InMemoryRunStateStore::new();
    let mut config = ControlPlaneConfig::default();
    config.provider_trust_overrides.insert(
        "test".to_string(),
        TrustRequirement {
            min_lane: TrustLane::Asserted,
        },
    );
    let engine = ControlPlane::new(
        base_spec(),
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        config,
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("tenant"),
        namespace_id: NamespaceId::from_raw(1).expect("namespace"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }
}
