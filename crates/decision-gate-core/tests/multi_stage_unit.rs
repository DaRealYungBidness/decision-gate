// crates/decision-gate-core/tests/multi_stage_unit.rs
// ============================================================================
// Module: Multi-Stage Scenario Unit Tests
// Description: Stage transitions, branch precedence, and timeout interactions.
// Purpose: Validate multi-stage behavior under adversarial and edge conditions.
// Threat Models: TM-STAGE-001 (stage bypass), TM-STAGE-002 (timeout manipulation)
// ============================================================================

//! Multi-stage scenario tests for stage transitions and branch behavior.

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

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_core::AdvanceTo;
use decision_gate_core::BranchRule;
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
use decision_gate_core::GateOutcome;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStateStore;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::TimeoutSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use serde_json::json;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

#[derive(Clone)]
struct MapProvider {
    responses: BTreeMap<String, EvidenceResult>,
    contexts: Arc<Mutex<Vec<EvidenceContext>>>,
}

impl MapProvider {
    fn new(responses: BTreeMap<String, EvidenceResult>) -> Self {
        Self {
            responses,
            contexts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl EvidenceProvider for MapProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let mut guard = self.contexts.lock().unwrap();
        guard.push(ctx.clone());
        drop(guard);

        self.responses
            .get(query.check_id.as_str())
            .cloned()
            .ok_or_else(|| EvidenceError::Provider("missing response".to_string()))
    }

    fn validate_providers(
        &self,
        _spec: &ScenarioSpec,
    ) -> Result<(), decision_gate_core::ProviderMissingError> {
        Ok(())
    }
}

struct ErrorProvider;

impl EvidenceProvider for ErrorProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Err(EvidenceError::Provider("boom".to_string()))
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

fn result_bool(value: bool) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Json(json!(value))),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: Some("application/json".to_string()),
    }
}

fn condition(check_id: &str) -> ConditionSpec {
    ConditionSpec {
        condition_id: check_id.into(),
        query: EvidenceQuery {
            provider_id: ProviderId::new("test"),
            check_id: check_id.to_string(),
            params: Some(json!({})),
        },
        comparator: Comparator::Equals,
        expected: Some(json!(true)),
        policy_tags: Vec::new(),
        trust: None,
    }
}

fn stage_with_gate(
    stage_id: &str,
    gate_id: &str,
    condition_id: &str,
    advance_to: AdvanceTo,
) -> StageSpec {
    StageSpec {
        stage_id: StageId::new(stage_id),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new(gate_id),
            requirement: ret_logic::Requirement::condition(condition_id.into()),
            trust: None,
        }],
        advance_to,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    }
}

fn base_spec(stages: Vec<StageSpec>, conditions: Vec<ConditionSpec>) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages,
        conditions,
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

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn multi_stage_linear_advances_and_completes() {
    let stage1 = stage_with_gate("stage-1", "gate-1", "ready", AdvanceTo::Linear);
    let stage2 = StageSpec {
        stage_id: StageId::new("stage-2"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let spec = base_spec(vec![stage1, stage2], vec![condition("ready")]);

    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    let provider = MapProvider::new(responses);

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
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
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Advance {
            ref to_stage,
            timeout,
            ..
        } => {
            assert_eq!(to_stage.as_str(), "stage-2");
            assert!(!timeout);
        }
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
    assert_eq!(state.current_stage_id.as_str(), "stage-2");
    assert_eq!(state.stage_entered_at, Timestamp::Logical(1));
    assert_eq!(state.status, RunStatus::Active);

    let trigger2 = TriggerEvent {
        trigger_id: TriggerId::new("trigger-2"),
        time: Timestamp::Logical(2),
        ..trigger
    };
    let result2 = engine.trigger(&trigger2).unwrap();
    match result2.decision.outcome {
        DecisionOutcome::Complete {
            stage_id,
        } => {
            assert_eq!(stage_id.as_str(), "stage-2");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn multi_stage_hold_on_unknown_evidence() {
    let stage1 = stage_with_gate("stage-1", "gate-1", "ready", AdvanceTo::Terminal);
    let spec = base_spec(vec![stage1], vec![condition("ready")]);

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        ErrorProvider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
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
fn multi_stage_branch_precedence_first_match() {
    let stage1 = StageSpec {
        stage_id: StageId::new("stage-1"),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new("gate-1"),
            requirement: ret_logic::Requirement::condition("ready".into()),
            trust: None,
        }],
        advance_to: AdvanceTo::Branch {
            branches: vec![
                BranchRule {
                    gate_id: GateId::new("gate-1"),
                    outcome: GateOutcome::True,
                    next_stage_id: StageId::new("stage-a"),
                },
                BranchRule {
                    gate_id: GateId::new("gate-1"),
                    outcome: GateOutcome::True,
                    next_stage_id: StageId::new("stage-b"),
                },
            ],
            default: Some(StageId::new("stage-default")),
        },
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage_a = StageSpec {
        stage_id: StageId::new("stage-a"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage_b = StageSpec {
        stage_id: StageId::new("stage-b"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage_default = StageSpec {
        stage_id: StageId::new("stage-default"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };

    let spec = base_spec(vec![stage1, stage_a, stage_b, stage_default], vec![condition("ready")]);

    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    let provider = MapProvider::new(responses);

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Advance {
            to_stage, ..
        } => {
            assert_eq!(to_stage.as_str(), "stage-a");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn multi_stage_branch_default_used_when_no_match() {
    let stage1 = StageSpec {
        stage_id: StageId::new("stage-1"),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new("gate-1"),
            requirement: ret_logic::Requirement::condition("ready".into()),
            trust: None,
        }],
        advance_to: AdvanceTo::Branch {
            branches: vec![BranchRule {
                gate_id: GateId::new("gate-1"),
                outcome: GateOutcome::False,
                next_stage_id: StageId::new("stage-true"),
            }],
            default: Some(StageId::new("stage-default")),
        },
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage_default = StageSpec {
        stage_id: StageId::new("stage-default"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage_true = StageSpec {
        stage_id: StageId::new("stage-true"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };

    let spec = base_spec(vec![stage1, stage_true, stage_default], vec![condition("ready")]);

    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    let provider = MapProvider::new(responses);

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Advance {
            to_stage, ..
        } => {
            assert_eq!(to_stage.as_str(), "stage-default");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn multi_stage_branch_no_default_returns_error() {
    let stage1 = StageSpec {
        stage_id: StageId::new("stage-1"),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new("gate-1"),
            requirement: ret_logic::Requirement::condition("ready".into()),
            trust: None,
        }],
        advance_to: AdvanceTo::Branch {
            branches: vec![BranchRule {
                gate_id: GateId::new("gate-1"),
                outcome: GateOutcome::False,
                next_stage_id: StageId::new("stage-2"),
            }],
            default: None,
        },
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage2 = stage_with_gate("stage-2", "gate-2", "other", AdvanceTo::Terminal);

    let spec = base_spec(vec![stage1, stage2], vec![condition("ready"), condition("other")]);
    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    responses.insert("other".to_string(), result_bool(true));
    let provider = MapProvider::new(responses);

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();
    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger);
    assert!(result.is_err(), "branch with no default should error");
}

#[test]
fn multi_stage_no_gates_advances_linear() {
    let stage1 = StageSpec {
        stage_id: StageId::new("stage-1"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Linear,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let stage2 = StageSpec {
        stage_id: StageId::new("stage-2"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let spec = base_spec(vec![stage1, stage2], Vec::new());

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        ErrorProvider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Advance {
            to_stage, ..
        } => {
            assert_eq!(to_stage.as_str(), "stage-2");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn multi_stage_timeout_alternate_branch_advances() {
    let stage1 = StageSpec {
        stage_id: StageId::new("stage-1"),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new("gate-1"),
            requirement: ret_logic::Requirement::condition("ready".into()),
            trust: None,
        }],
        advance_to: AdvanceTo::Branch {
            branches: vec![BranchRule {
                gate_id: GateId::new("gate-1"),
                outcome: GateOutcome::Unknown,
                next_stage_id: StageId::new("stage-timeout"),
            }],
            default: None,
        },
        timeout: Some(TimeoutSpec {
            timeout_ms: 5,
            policy_tags: Vec::new(),
        }),
        on_timeout: TimeoutPolicy::AlternateBranch,
    };
    let timeout_stage = StageSpec {
        stage_id: StageId::new("stage-timeout"),
        entry_packets: Vec::new(),
        gates: Vec::new(),
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    };
    let spec = base_spec(vec![stage1, timeout_stage], vec![condition("ready")]);

    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(false));
    let provider = MapProvider::new(responses);

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec,
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-timeout"),
        kind: TriggerKind::Tick,
        time: Timestamp::Logical(10),
        source_id: "tick".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Advance {
            to_stage,
            timeout,
            ..
        } => {
            assert_eq!(to_stage.as_str(), "stage-timeout");
            assert!(timeout);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}
