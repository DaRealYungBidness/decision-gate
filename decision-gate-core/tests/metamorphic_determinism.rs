//! Metamorphic determinism tests for Decision Gate core.
// decision-gate-core/tests/metamorphic_determinism.rs
// ============================================================================
// Module: Metamorphic Determinism Tests
// Description: Ordering-insensitive determinism for gate evaluation logs.
// ============================================================================
//! ## Overview
//! Ensures gate evaluation evidence ordering is canonical regardless of
//! evaluation or provider call order.

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
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
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
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
        let value = match query.check_id.as_str() {
            "first" | "second" => json!(true),
            _ => json!(false),
        };
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(value)),
            lane: TrustLane::Verified,
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
        _target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<DispatchReceipt, decision_gate_core::DispatchError> {
        Ok(DispatchReceipt {
            dispatch_id: "dispatch-1".to_string(),
            target: DispatchTarget::Agent {
                agent_id: "agent-1".to_string(),
            },
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

#[test]
fn gate_eval_evidence_order_is_canonical() -> Result<(), Box<dyn std::error::Error>> {
    let scenario_id = ScenarioId::new("metamorphic-order");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let condition_a = ConditionId::new("first");
    let condition_b = ConditionId::new("second");

    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::and(vec![
                    ret_logic::Requirement::condition(condition_b.clone()),
                    ret_logic::Requirement::condition(condition_a.clone()),
                ]),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            ConditionSpec {
                condition_id: condition_b,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("test"),
                    check_id: "second".to_string(),
                    params: None,
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: condition_a,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("test"),
                    check_id: "first".to_string(),
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
    };

    let store = InMemoryRunStateStore::new();
    let config = ControlPlaneConfig::default();
    let control = ControlPlane::new(
        spec,
        TestEvidenceProvider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        config,
    )?;

    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id,
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };

    control.start_run(run_config.clone(), Timestamp::Logical(1), false)?;

    let trigger = TriggerEvent {
        run_id: run_config.run_id.clone(),
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(2),
        source_id: "metamorphic".to_string(),
        payload: None,
        correlation_id: None,
    };

    let _ = control.trigger(&trigger)?;
    let state = store
        .load(&run_config.tenant_id, &run_config.namespace_id, &run_config.run_id)?
        .ok_or("missing run state")?;

    let evidence = state
        .gate_evals
        .first()
        .ok_or("missing gate eval")?
        .evidence
        .iter()
        .map(|record| record.condition_id.as_str().to_string())
        .collect::<Vec<_>>();

    if evidence != vec!["first".to_string(), "second".to_string()] {
        return Err(format!("expected canonical evidence order, got {evidence:?}").into());
    }

    Ok(())
}

#[test]
fn gate_eval_evidence_order_canonical_across_requirement_permutations()
-> Result<(), Box<dyn std::error::Error>> {
    let permutations = [
        vec!["third", "first", "second"],
        vec!["second", "third", "first"],
        vec!["first", "third", "second"],
    ];
    for (index, order) in permutations.iter().enumerate() {
        let scenario_id = ScenarioId::new(format!("metamorphic-order-{index}"));
        let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
        let condition_ids =
            [ConditionId::new("first"), ConditionId::new("second"), ConditionId::new("third")];

        let mut conditions = vec![
            build_test_condition(&condition_ids[2], "third"),
            build_test_condition(&condition_ids[0], "first"),
            build_test_condition(&condition_ids[1], "second"),
        ];
        if index % 2 == 0 {
            conditions.swap(0, 1);
        }

        let requirement = ret_logic::Requirement::and(
            order
                .iter()
                .map(|id| ret_logic::Requirement::condition(ConditionId::new(*id)))
                .collect(),
        );

        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            namespace_id,
            spec_version: SpecVersion::new("1"),
            stages: vec![StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-1"),
                    requirement,
                    trust: None,
                }],
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            }],
            conditions,
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };

        assert_gate_eval_order(&spec, &["first", "second", "third"])?;
    }
    Ok(())
}

fn build_test_condition(condition_id: &ConditionId, check_id: &str) -> ConditionSpec {
    ConditionSpec {
        condition_id: condition_id.clone(),
        query: EvidenceQuery {
            provider_id: ProviderId::new("test"),
            check_id: check_id.to_string(),
            params: None,
        },
        comparator: Comparator::Equals,
        expected: Some(json!(true)),
        policy_tags: Vec::new(),
        trust: None,
    }
}

fn assert_gate_eval_order(
    spec: &ScenarioSpec,
    expected: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let store = InMemoryRunStateStore::new();
    let config = ControlPlaneConfig::default();
    let control = ControlPlane::new(
        spec.clone(),
        TestEvidenceProvider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        config,
    )?;
    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: spec.namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: spec.scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    control.start_run(run_config.clone(), Timestamp::Logical(1), false)?;
    let trigger = TriggerEvent {
        run_id: run_config.run_id.clone(),
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(2),
        source_id: "metamorphic".to_string(),
        payload: None,
        correlation_id: None,
    };
    let _ = control.trigger(&trigger)?;

    let state = store
        .load(&run_config.tenant_id, &run_config.namespace_id, &run_config.run_id)?
        .ok_or("missing run state")?;
    let evidence = state
        .gate_evals
        .first()
        .ok_or("missing gate eval")?
        .evidence
        .iter()
        .map(|record| record.condition_id.as_str().to_string())
        .collect::<Vec<_>>();
    let expected_vec = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
    if evidence != expected_vec {
        return Err(format!("expected canonical evidence order, got {evidence:?}").into());
    }
    Ok(())
}
