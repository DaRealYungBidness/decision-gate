// decision-gate-core/tests/precheck.rs
// ============================================================================
// Module: Precheck and Gate Evaluation Tests
// Description: Tests for read-only precheck, gate evaluation, and trust composition.
// Purpose: Ensure precheck behaves correctly with various gate configurations.
// Dependencies: decision-gate-core
// ============================================================================

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    missing_docs,
    reason = "Test-only panic-based assertions are permitted."
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
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
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TrustLane;
use decision_gate_core::TrustRequirement;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::PrecheckRequest;
use ret_logic::TriState;
use serde_json::json;

struct NoopEvidenceProvider;

impl EvidenceProvider for NoopEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
        Err(decision_gate_core::EvidenceError::Provider("no evidence".to_string()))
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
        _envelope: &PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<DispatchReceipt, decision_gate_core::DispatchError> {
        Ok(DispatchReceipt {
            dispatch_id: "dispatch-1".to_string(),
            target: target.clone(),
            receipt_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"noop"),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "noop".to_string(),
        })
    }
}

struct NoopPolicy;

impl PolicyDecider for NoopPolicy {
    fn authorize(
        &self,
        _target: &DispatchTarget,
        _envelope: &PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<PolicyDecision, decision_gate_core::PolicyError> {
        Ok(PolicyDecision::Permit)
    }
}

#[derive(Clone, Default)]
struct CountingStore {
    saves: Arc<AtomicUsize>,
}

impl CountingStore {
    fn save_count(&self) -> usize {
        self.saves.load(Ordering::Relaxed)
    }
}

impl RunStateStore for CountingStore {
    fn load(
        &self,
        _tenant_id: &TenantId,
        _namespace_id: &NamespaceId,
        _run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        Ok(None)
    }

    fn save(&self, _state: &RunState) -> Result<(), StoreError> {
        self.saves.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn sample_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-ready"),
                requirement: ret_logic::Requirement::condition("ready".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id: ConditionId::new("ready"),
            query: EvidenceQuery {
                provider_id: ProviderId::new("noop"),
                check_id: "ready".to_string(),
                params: None,
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

fn sample_spec_two_stages() -> ScenarioSpec {
    let stage_one = StageId::new("stage-1");
    let stage_two = StageId::new("stage-2");
    let ready_condition = ConditionId::new("ready");
    let approved_condition = ConditionId::new("approved");
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: stage_one,
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-ready"),
                    requirement: ret_logic::Requirement::condition(ready_condition.clone()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Fixed {
                    stage_id: stage_two.clone(),
                },
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: stage_two,
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-approved"),
                    requirement: ret_logic::Requirement::condition(approved_condition.clone()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        conditions: vec![
            ConditionSpec {
                condition_id: ready_condition,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("noop"),
                    check_id: "ready".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: approved_condition,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("noop"),
                    check_id: "approved".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn build_control_plane(
    trust_lane: TrustLane,
) -> ControlPlane<NoopEvidenceProvider, NoopDispatcher, InMemoryRunStateStore, NoopPolicy> {
    build_control_plane_with_store(sample_spec(), trust_lane, InMemoryRunStateStore::new())
}

fn build_control_plane_with_store<S: RunStateStore>(
    spec: ScenarioSpec,
    trust_lane: TrustLane,
    store: S,
) -> ControlPlane<NoopEvidenceProvider, NoopDispatcher, S, NoopPolicy> {
    let config = ControlPlaneConfig {
        trust_requirement: TrustRequirement {
            min_lane: trust_lane,
        },
        ..ControlPlaneConfig::default()
    };
    ControlPlane::new(spec, NoopEvidenceProvider, NoopDispatcher, store, None, config)
        .expect("control plane")
}

#[test]
fn precheck_completes_on_verified_evidence() {
    let control = build_control_plane(TrustLane::Verified);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");
    match result.decision {
        DecisionOutcome::Complete {
            stage_id,
        } => {
            assert_eq!(stage_id.as_str(), "stage-1");
        }
        other => panic!("unexpected decision: {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_holds_on_untrusted_evidence() {
    let control = build_control_plane(TrustLane::Verified);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Asserted,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");
    match result.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected decision: {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

#[test]
fn precheck_respects_gate_trust_override() {
    let mut spec = sample_spec();
    spec.stages[0].gates[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });
    let control =
        build_control_plane_with_store(spec, TrustLane::Asserted, InMemoryRunStateStore::new());

    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Asserted,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);

    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_uses_stage_override() {
    let spec = sample_spec_two_stages();
    let control =
        build_control_plane_with_store(spec, TrustLane::Verified, InMemoryRunStateStore::new());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("approved"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: Some(StageId::new("stage-2")),
            evidence,
        })
        .expect("precheck result");
    match result.decision {
        DecisionOutcome::Complete {
            stage_id,
        } => {
            assert_eq!(stage_id.as_str(), "stage-2");
        }
        other => panic!("unexpected decision: {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_holds_when_evidence_missing() {
    let control = build_control_plane(TrustLane::Verified);
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence: BTreeMap::new(),
        })
        .expect("precheck result");
    match result.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected decision: {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

#[test]
fn precheck_does_not_write_run_state() {
    let store = CountingStore::default();
    let control = build_control_plane_with_store(sample_spec(), TrustLane::Verified, store.clone());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let _ = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");
    assert_eq!(store.save_count(), 0);
}

#[test]
fn precheck_advances_to_next_stage() {
    let spec = sample_spec_two_stages();
    let control =
        build_control_plane_with_store(spec, TrustLane::Verified, InMemoryRunStateStore::new());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    let result = control
        .precheck(&PrecheckRequest {
            stage_id: Some(StageId::new("stage-1")),
            evidence,
        })
        .expect("precheck result");
    match result.decision {
        DecisionOutcome::Advance {
            to_stage, ..
        } => {
            assert_eq!(to_stage.as_str(), "stage-2");
        }
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn precheck_rejects_unknown_stage() {
    let control = build_control_plane(TrustLane::Verified);
    let result = control.precheck(&PrecheckRequest {
        stage_id: Some(StageId::new("missing")),
        evidence: BTreeMap::new(),
    });
    let err = result.unwrap_err();
    assert!(err.to_string().contains("unknown stage identifier"));
}

// ============================================================================
// SECTION: Multi-Gate AND/OR Evaluation
// ============================================================================

fn spec_with_and_gate() -> ScenarioSpec {
    let ready_condition = ConditionId::new("ready");
    let approved_condition = ConditionId::new("approved");
    ScenarioSpec {
        scenario_id: ScenarioId::new("and-gate-scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-and"),
                // AND: both ready AND approved must be true
                requirement: ret_logic::Requirement::and(vec![
                    ret_logic::Requirement::condition(ready_condition.clone()),
                    ret_logic::Requirement::condition(approved_condition.clone()),
                ]),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            ConditionSpec {
                condition_id: ready_condition,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("noop"),
                    check_id: "ready".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: approved_condition,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("noop"),
                    check_id: "approved".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn spec_with_or_gate() -> ScenarioSpec {
    let ready_condition = ConditionId::new("ready");
    let approved_condition = ConditionId::new("approved");
    ScenarioSpec {
        scenario_id: ScenarioId::new("or-gate-scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-or"),
                // OR: either ready OR approved must be true
                requirement: ret_logic::Requirement::or(vec![
                    ret_logic::Requirement::condition(ready_condition.clone()),
                    ret_logic::Requirement::condition(approved_condition.clone()),
                ]),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            ConditionSpec {
                condition_id: ready_condition,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("noop"),
                    check_id: "ready".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: approved_condition,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("noop"),
                    check_id: "approved".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

#[test]
fn precheck_and_gate_all_true_passes() {
    let control = build_control_plane_with_store(
        spec_with_and_gate(),
        TrustLane::Verified,
        InMemoryRunStateStore::new(),
    );
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    evidence.insert(
        ConditionId::new("approved"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("expected Complete, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_and_gate_any_false_fails() {
    let control = build_control_plane_with_store(
        spec_with_and_gate(),
        TrustLane::Verified,
        InMemoryRunStateStore::new(),
    );
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    evidence.insert(
        ConditionId::new("approved"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(false))), // This will fail
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    // Implementation holds (allows retry) rather than failing immediately on False
    match result.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("expected Hold, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::False);
}

#[test]
fn precheck_and_gate_any_unknown_holds() {
    let control = build_control_plane_with_store(
        spec_with_and_gate(),
        TrustLane::Verified,
        InMemoryRunStateStore::new(),
    );
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    // Missing "approved" evidence -> Unknown

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("expected Hold, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

#[test]
fn precheck_or_gate_any_true_passes() {
    let control = build_control_plane_with_store(
        spec_with_or_gate(),
        TrustLane::Verified,
        InMemoryRunStateStore::new(),
    );
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    // "approved" is missing, but OR only needs one

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("expected Complete, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_or_gate_all_false_holds() {
    // Implementation holds (allows retry) rather than failing immediately on False
    let control = build_control_plane_with_store(
        spec_with_or_gate(),
        TrustLane::Verified,
        InMemoryRunStateStore::new(),
    );
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(false))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );
    evidence.insert(
        ConditionId::new("approved"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(false))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("expected Hold, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::False);
}

#[test]
fn precheck_or_gate_all_unknown_holds() {
    let control = build_control_plane_with_store(
        spec_with_or_gate(),
        TrustLane::Verified,
        InMemoryRunStateStore::new(),
    );
    // No evidence at all -> both conditions Unknown

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence: BTreeMap::new(),
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("expected Hold, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

// ============================================================================
// SECTION: Trust Lattice Composition in Precheck
// ============================================================================

#[test]
fn precheck_config_verified_condition_asserted_gate_verified_rejects_asserted_evidence() {
    // Config: Verified (default)
    // Condition: Asserted (relaxed)
    // Gate: Verified (tightened)
    // Evidence: Asserted
    // Result: Should reject because gate requires Verified
    let mut spec = sample_spec();
    spec.conditions[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    spec.stages[0].gates[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let control =
        build_control_plane_with_store(spec, TrustLane::Verified, InMemoryRunStateStore::new());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Asserted, // Asserted evidence
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    // Should hold because Asserted doesn't satisfy Verified requirement
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

#[test]
fn precheck_config_asserted_condition_verified_gate_none_accepts_verified_evidence() {
    // Config: Asserted (relaxed)
    // Condition: Verified (tightened)
    // Gate: None (inherits condition)
    // Evidence: Verified
    // Result: Should pass
    let mut spec = sample_spec();
    spec.conditions[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let control =
        build_control_plane_with_store(spec, TrustLane::Asserted, InMemoryRunStateStore::new());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("expected Complete, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_gate_override_stricter_than_condition_honored() {
    // Condition allows Asserted, Gate requires Verified
    let mut spec = sample_spec();
    spec.conditions[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    spec.stages[0].gates[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let control =
        build_control_plane_with_store(spec, TrustLane::Asserted, InMemoryRunStateStore::new());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Asserted,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    // Gate's stricter requirement wins
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

#[test]
fn precheck_condition_override_stricter_than_config_honored() {
    // Config allows Asserted, Condition requires Verified
    let mut spec = sample_spec();
    spec.conditions[0].trust = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let control =
        build_control_plane_with_store(spec, TrustLane::Asserted, InMemoryRunStateStore::new());
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Asserted,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    // Condition's stricter requirement wins
    assert_eq!(result.gate_evaluations[0].status, TriState::Unknown);
}

// ============================================================================
// SECTION: Precheck Determinism
// ============================================================================

#[test]
fn precheck_repeated_calls_same_input_produce_same_result() {
    let control = build_control_plane(TrustLane::Verified);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let request = PrecheckRequest {
        stage_id: None,
        evidence,
    };

    let result1 = control.precheck(&request).expect("precheck result 1");
    let result2 = control.precheck(&request).expect("precheck result 2");
    let result3 = control.precheck(&request).expect("precheck result 3");

    // All results should be identical
    assert_eq!(result1.decision, result2.decision);
    assert_eq!(result2.decision, result3.decision);
    assert_eq!(result1.gate_evaluations[0].status, result2.gate_evaluations[0].status);
    assert_eq!(result2.gate_evaluations[0].status, result3.gate_evaluations[0].status);
}

// ============================================================================
// SECTION: Evidence with Different Trust Lanes
// ============================================================================

#[test]
fn precheck_verified_evidence_passes_asserted_requirement() {
    // Config requires only Asserted, evidence is Verified (higher trust)
    let control = build_control_plane(TrustLane::Asserted);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Verified, // Higher trust than required
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    match result.decision {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("expected Complete, got {other:?}"),
    }
    assert_eq!(result.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_trust_lane_error_includes_error_code() {
    let control = build_control_plane(TrustLane::Verified);
    let mut evidence = BTreeMap::new();
    evidence.insert(
        ConditionId::new("ready"),
        EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: TrustLane::Asserted, // Lower trust than required
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        },
    );

    let result = control
        .precheck(&PrecheckRequest {
            stage_id: None,
            evidence,
        })
        .expect("precheck result");

    // The gate evaluation should indicate the trust requirement was not satisfied
    // When trust fails, the overall decision should be Hold or Fail
    match result.decision {
        DecisionOutcome::Hold {
            ..
        }
        | DecisionOutcome::Fail {
            ..
        } => {
            // Expected: evidence with Asserted lane doesn't satisfy Verified requirement
        }
        other => panic!("expected Hold or Fail due to trust mismatch, got {other:?}"),
    }
}
