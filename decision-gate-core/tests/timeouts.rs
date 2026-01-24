// decision-gate-core/tests/timeouts.rs
// ============================================================================
// Module: Timeout Handling Tests
// Description: Ensures timeout policies are enforced deterministically.
// ============================================================================
//! ## Overview
//! Validates timeout evaluation on tick triggers and policy outcomes.

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
    reason = "Test-only output and panic-based assertions are permitted."
)]

use decision_gate_core::AdvanceTo;
use decision_gate_core::BranchRule;
use decision_gate_core::Comparator;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateId;
use decision_gate_core::GateOutcome;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStateStore;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SchemaId;
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

struct TestEvidenceProvider;

impl EvidenceProvider for TestEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
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

struct TestDispatcher;

impl Dispatcher for TestDispatcher {
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
            dispatcher: "test".to_string(),
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

fn base_predicate() -> PredicateSpec {
    PredicateSpec {
        predicate: "ready".into(),
        query: EvidenceQuery {
            provider_id: ProviderId::new("test"),
            predicate: "ready".to_string(),
            params: Some(json!({})),
        },
        comparator: Comparator::Equals,
        expected: Some(json!(true)),
        policy_tags: Vec::new(),
        trust: None,
    }
}

fn base_gate() -> GateSpec {
    GateSpec {
        gate_id: GateId::new("gate-1"),
        requirement: ret_logic::Requirement::predicate("ready".into()),
        trust: None,
    }
}

fn start_run(
    engine: &ControlPlane<
        TestEvidenceProvider,
        TestDispatcher,
        InMemoryRunStateStore,
        PermitAllPolicy,
    >,
) {
    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
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
fn timeout_fail_triggers_fail_decision() {
    let store = InMemoryRunStateStore::new();
    let spec = ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![base_gate()],
            advance_to: AdvanceTo::Terminal,
            timeout: Some(TimeoutSpec {
                timeout_ms: 5,
                policy_tags: Vec::new(),
            }),
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: vec![base_predicate()],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };

    let engine = ControlPlane::new(
        spec,
        TestEvidenceProvider,
        TestDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("tick-1"),
        kind: TriggerKind::Tick,
        time: Timestamp::Logical(10),
        source_id: "scheduler".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    match result.decision.outcome {
        DecisionOutcome::Fail {
            reason,
        } => assert_eq!(reason, "timeout"),
        other => panic!("unexpected decision outcome: {other:?}"),
    }
}

#[test]
fn timeout_advance_with_flag_advances_stage() {
    let store = InMemoryRunStateStore::new();
    let spec = ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: Vec::new(),
                advance_to: AdvanceTo::Linear,
                timeout: Some(TimeoutSpec {
                    timeout_ms: 5,
                    policy_tags: Vec::new(),
                }),
                on_timeout: TimeoutPolicy::AdvanceWithFlag,
            },
            StageSpec {
                stage_id: StageId::new("stage-2"),
                entry_packets: vec![PacketSpec {
                    packet_id: decision_gate_core::PacketId::new("packet-1"),
                    schema_id: SchemaId::new("schema-1"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["public".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: PacketPayload::Json {
                        value: json!({"ok": true}),
                    },
                }],
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: TimeoutPolicy::Fail,
            },
        ],
        predicates: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };

    let engine = ControlPlane::new(
        spec,
        TestEvidenceProvider,
        TestDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("tick-1"),
        kind: TriggerKind::Tick,
        time: Timestamp::Logical(10),
        source_id: "scheduler".to_string(),
        payload: None,
        correlation_id: None,
    };

    let result = engine.trigger(&trigger).unwrap();
    match result.decision.outcome {
        DecisionOutcome::Advance {
            from_stage,
            to_stage,
            timeout,
        } => {
            assert_eq!(from_stage, StageId::new("stage-1"));
            assert_eq!(to_stage, StageId::new("stage-2"));
            assert!(timeout);
        }
        other => panic!("unexpected decision outcome: {other:?}"),
    }

    let state = store
        .load(
            &TenantId::new("tenant"),
            &NamespaceId::new("default"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("run state");
    assert_eq!(state.current_stage_id, StageId::new("stage-2"));
    assert_eq!(state.stage_entered_at, Timestamp::Logical(10));
}

#[test]
fn timeout_alternate_branch_routes_unknown() {
    let store = InMemoryRunStateStore::new();
    let spec = ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: vec![base_gate()],
                advance_to: AdvanceTo::Branch {
                    branches: vec![BranchRule {
                        gate_id: GateId::new("gate-1"),
                        outcome: GateOutcome::Unknown,
                        next_stage_id: StageId::new("stage-timeout"),
                    }],
                    default: Some(StageId::new("stage-default")),
                },
                timeout: Some(TimeoutSpec {
                    timeout_ms: 5,
                    policy_tags: Vec::new(),
                }),
                on_timeout: TimeoutPolicy::AlternateBranch,
            },
            StageSpec {
                stage_id: StageId::new("stage-timeout"),
                entry_packets: Vec::new(),
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: StageId::new("stage-default"),
                entry_packets: Vec::new(),
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: TimeoutPolicy::Fail,
            },
        ],
        predicates: vec![base_predicate()],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };

    let engine = ControlPlane::new(
        spec,
        TestEvidenceProvider,
        TestDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("tick-1"),
        kind: TriggerKind::Tick,
        time: Timestamp::Logical(10),
        source_id: "scheduler".to_string(),
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
            assert_eq!(to_stage, StageId::new("stage-timeout"));
            assert!(timeout);
        }
        other => panic!("unexpected decision outcome: {other:?}"),
    }

    let state = store
        .load(
            &TenantId::new("tenant"),
            &NamespaceId::new("default"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("run state");
    assert_eq!(state.current_stage_id, StageId::new("stage-timeout"));
}

#[test]
fn tick_before_timeout_evaluates_normally() {
    let store = InMemoryRunStateStore::new();
    let spec = ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: Vec::new(),
                advance_to: AdvanceTo::Linear,
                timeout: Some(TimeoutSpec {
                    timeout_ms: 10,
                    policy_tags: Vec::new(),
                }),
                on_timeout: TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: StageId::new("stage-2"),
                entry_packets: Vec::new(),
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: TimeoutPolicy::Fail,
            },
        ],
        predicates: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };

    let engine = ControlPlane::new(
        spec,
        TestEvidenceProvider,
        TestDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    start_run(&engine);

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("tick-1"),
        kind: TriggerKind::Tick,
        time: Timestamp::Logical(5),
        source_id: "scheduler".to_string(),
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
            assert_eq!(to_stage, StageId::new("stage-2"));
            assert!(!timeout);
        }
        other => panic!("unexpected decision outcome: {other:?}"),
    }
}
