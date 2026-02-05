// crates/decision-gate-core/tests/provider_orchestration_unit.rs
// ============================================================================
// Module: Provider Orchestration Unit Tests
// Description: Multi-provider coordination and trust override behavior.
// Purpose: Ensure provider failures do not corrupt evaluation and ordering is deterministic.
// Threat Models: TM-PROV-001 (provider DoS), TM-PROV-002 (provider confusion)
// ============================================================================

//! Provider orchestration tests for multi-provider evaluation and ordering.

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
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;

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

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

#[derive(Clone)]
struct MultiProvider {
    responses: BTreeMap<String, EvidenceResult>,
    errors: BTreeSet<String>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl MultiProvider {
    fn new(responses: BTreeMap<String, EvidenceResult>) -> Self {
        Self {
            responses,
            errors: BTreeSet::new(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_errors(mut self, errors: &[&str]) -> Self {
        for err in errors {
            self.errors.insert((*err).to_string());
        }
        self
    }

    fn calls(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.calls)
    }
}

impl EvidenceProvider for MultiProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let mut guard = self.calls.lock().unwrap();
        guard.push(query.provider_id.as_str().to_string());
        drop(guard);

        if self.errors.contains(query.provider_id.as_str()) {
            return Err(EvidenceError::Provider("provider error".to_string()));
        }
        self.responses
            .get(query.provider_id.as_str())
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

fn result_bool(value: bool, lane: TrustLane) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Json(json!(value))),
        lane,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: Some("application/json".to_string()),
    }
}

fn condition(provider_id: &str, condition_id: &str) -> ConditionSpec {
    ConditionSpec {
        condition_id: condition_id.into(),
        query: EvidenceQuery {
            provider_id: ProviderId::new(provider_id),
            check_id: condition_id.to_string(),
            params: Some(json!({})),
        },
        comparator: Comparator::Equals,
        expected: Some(json!(true)),
        policy_tags: Vec::new(),
        trust: None,
    }
}

fn spec_with_conditions(
    conditions: Vec<ConditionSpec>,
    requirement: ret_logic::Requirement<decision_gate_core::ConditionId>,
) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
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
fn provider_orchestration_calls_all_providers() {
    let conditions = vec![condition("alpha", "cond-a"), condition("beta", "cond-b")];
    let requirement = ret_logic::Requirement::and(vec![
        ret_logic::Requirement::condition("cond-a".into()),
        ret_logic::Requirement::condition("cond-b".into()),
    ]);
    let spec = spec_with_conditions(conditions, requirement);

    let mut responses = BTreeMap::new();
    responses.insert("alpha".to_string(), result_bool(true, TrustLane::Verified));
    responses.insert("beta".to_string(), result_bool(true, TrustLane::Verified));
    let provider = MultiProvider::new(responses);
    let calls = provider.calls();

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
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }

    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert!(calls.contains(&"alpha".to_string()));
    assert!(calls.contains(&"beta".to_string()));
}

#[test]
fn provider_orchestration_or_requirement_still_calls_all_providers() {
    let conditions = vec![condition("alpha", "cond-a"), condition("beta", "cond-b")];
    let requirement = ret_logic::Requirement::or(vec![
        ret_logic::Requirement::condition("cond-a".into()),
        ret_logic::Requirement::condition("cond-b".into()),
    ]);
    let spec = spec_with_conditions(conditions, requirement);

    let mut responses = BTreeMap::new();
    responses.insert("alpha".to_string(), result_bool(true, TrustLane::Verified));
    responses.insert("beta".to_string(), result_bool(false, TrustLane::Verified));
    let provider = MultiProvider::new(responses);
    let calls = provider.calls();

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
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }

    let calls = {
        let calls = calls.lock().unwrap();
        calls.clone()
    };
    assert!(calls.contains(&"alpha".to_string()));
    assert!(calls.contains(&"beta".to_string()));
    assert_eq!(calls.len(), 2);
}

#[test]
fn provider_orchestration_failure_does_not_skip_other_calls() {
    let conditions = vec![condition("alpha", "cond-a"), condition("beta", "cond-b")];
    let requirement = ret_logic::Requirement::and(vec![
        ret_logic::Requirement::condition("cond-a".into()),
        ret_logic::Requirement::condition("cond-b".into()),
    ]);
    let spec = spec_with_conditions(conditions, requirement);

    let mut responses = BTreeMap::new();
    responses.insert("alpha".to_string(), result_bool(true, TrustLane::Verified));
    responses.insert("beta".to_string(), result_bool(true, TrustLane::Verified));
    let provider = MultiProvider::new(responses).with_errors(&["alpha"]);
    let calls = provider.calls();

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
        DecisionOutcome::Hold {
            ..
        } => {}
        other => panic!("unexpected outcome: {other:?}"),
    }

    let calls = {
        let calls = calls.lock().unwrap();
        calls.clone()
    };
    assert_eq!(calls.len(), 2, "both providers should be queried");
}

#[test]
fn provider_orchestration_trust_override_enforces_stricter_lane() {
    let conditions = vec![condition("alpha", "cond-a")];
    let requirement = ret_logic::Requirement::condition("cond-a".into());
    let spec = spec_with_conditions(conditions, requirement);

    let mut responses = BTreeMap::new();
    responses.insert("alpha".to_string(), result_bool(true, TrustLane::Asserted));
    let provider = MultiProvider::new(responses);

    let mut config = ControlPlaneConfig {
        trust_requirement: TrustRequirement {
            min_lane: TrustLane::Asserted,
        },
        ..ControlPlaneConfig::default()
    };
    config.provider_trust_overrides.insert(
        "alpha".to_string(),
        TrustRequirement {
            min_lane: TrustLane::Verified,
        },
    );

    let store = InMemoryRunStateStore::new();
    let engine =
        ControlPlane::new(spec, provider, NoopDispatcher, store, Some(PermitAllPolicy), config)
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
fn provider_orchestration_deterministic_order_from_requirement() {
    let conditions = vec![condition("alpha", "cond-a"), condition("beta", "cond-b")];
    let requirement = ret_logic::Requirement::and(vec![
        ret_logic::Requirement::condition("cond-b".into()),
        ret_logic::Requirement::condition("cond-a".into()),
    ]);
    let spec = spec_with_conditions(conditions, requirement);

    let mut responses = BTreeMap::new();
    responses.insert("alpha".to_string(), result_bool(true, TrustLane::Verified));
    responses.insert("beta".to_string(), result_bool(true, TrustLane::Verified));
    let provider = MultiProvider::new(responses);
    let calls = provider.calls();

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

    let _ = engine.trigger(&trigger).unwrap();
    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls, vec!["beta".to_string(), "alpha".to_string()]);
}
