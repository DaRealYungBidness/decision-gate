// crates/decision-gate-core/tests/evidence_correlation_unit.rs
// ============================================================================
// Module: Evidence Correlation Unit Tests
// Description: Validate evidence context propagation and run isolation.
// Purpose: Ensure evidence queries are bound to the correct run/stage/trigger context.
// Threat Models: TM-EVID-001 (correlation attack), TM-EVID-002 (replay)
// ============================================================================

//! Evidence correlation tests for context propagation and run isolation.

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
use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
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
struct RecordingProvider {
    responses: BTreeMap<String, EvidenceResult>,
    contexts: Arc<Mutex<Vec<EvidenceContext>>>,
}

impl RecordingProvider {
    fn new(responses: BTreeMap<String, EvidenceResult>) -> Self {
        Self {
            responses,
            contexts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn contexts(&self) -> Arc<Mutex<Vec<EvidenceContext>>> {
        Arc::clone(&self.contexts)
    }
}

impl EvidenceProvider for RecordingProvider {
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

fn spec_two_stages() -> ScenarioSpec {
    let stage1 = StageSpec {
        stage_id: StageId::new("stage-1"),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new("gate-1"),
            requirement: ret_logic::Requirement::condition("ready".into()),
            trust: None,
        }],
        advance_to: AdvanceTo::Linear,
        timeout: None,
        on_timeout: decision_gate_core::TimeoutPolicy::Fail,
    };
    let stage2 = StageSpec {
        stage_id: StageId::new("stage-2"),
        entry_packets: Vec::new(),
        gates: vec![GateSpec {
            gate_id: GateId::new("gate-2"),
            requirement: ret_logic::Requirement::condition("ready-2".into()),
            trust: None,
        }],
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: decision_gate_core::TimeoutPolicy::Fail,
    };
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![stage1, stage2],
        conditions: vec![condition("ready"), condition("ready-2")],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn start_run<P: EvidenceProvider>(
    engine: &ControlPlane<P, NoopDispatcher, InMemoryRunStateStore, PermitAllPolicy>,
    run_id: &str,
) {
    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: decision_gate_core::RunId::new(run_id),
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
fn evidence_context_propagates_correlation_id() {
    let spec = spec_two_stages();
    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    responses.insert("ready-2".to_string(), result_bool(true));
    let provider = RecordingProvider::new(responses);
    let contexts = provider.contexts();

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

    start_run(&engine, "run-1");

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("tenant"),
        namespace_id: NamespaceId::from_raw(1).expect("namespace"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: Some(decision_gate_core::CorrelationId::new("corr-1")),
    };

    let _ = engine.trigger(&trigger).unwrap();

    let ctx = {
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        contexts[0].clone()
    };
    assert_eq!(ctx.run_id.as_str(), "run-1");
    assert_eq!(ctx.stage_id.as_str(), "stage-1");
    assert_eq!(ctx.trigger_id.as_str(), "trigger-1");
    assert_eq!(ctx.correlation_id.as_ref().unwrap().as_str(), "corr-1");
}

#[test]
fn evidence_context_includes_run_and_scenario_metadata() {
    let spec = spec_two_stages();
    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    responses.insert("ready-2".to_string(), result_bool(true));
    let provider = RecordingProvider::new(responses);
    let contexts = provider.contexts();

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

    start_run(&engine, "run-1");

    let trigger = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("tenant"),
        namespace_id: NamespaceId::from_raw(1).expect("namespace"),
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(42),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    };

    let _ = engine.trigger(&trigger).unwrap();

    let ctx = {
        let contexts = contexts.lock().unwrap();
        contexts[0].clone()
    };
    assert_eq!(ctx.tenant_id.get(), 1);
    assert_eq!(ctx.namespace_id.get(), 1);
    assert_eq!(ctx.scenario_id.as_str(), "scenario");
    assert_eq!(ctx.trigger_time, Timestamp::Logical(42));
}

#[test]
fn evidence_context_updates_across_stage_transitions() {
    let spec = spec_two_stages();
    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    responses.insert("ready-2".to_string(), result_bool(true));
    let provider = RecordingProvider::new(responses);
    let contexts = provider.contexts();

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

    start_run(&engine, "run-1");

    let trigger1 = TriggerEvent {
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

    let _ = engine.trigger(&trigger1).unwrap();

    let trigger2 = TriggerEvent {
        trigger_id: TriggerId::new("trigger-2"),
        time: Timestamp::Logical(2),
        ..trigger1
    };
    let _ = engine.trigger(&trigger2).unwrap();

    let (first_stage, second_stage) = {
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 2);
        (contexts[0].stage_id.as_str().to_string(), contexts[1].stage_id.as_str().to_string())
    };
    assert_eq!(first_stage, "stage-1");
    assert_eq!(second_stage, "stage-2");
}

#[test]
fn evidence_context_isolated_across_runs() {
    let spec = spec_two_stages();
    let mut responses = BTreeMap::new();
    responses.insert("ready".to_string(), result_bool(true));
    responses.insert("ready-2".to_string(), result_bool(true));
    let provider = RecordingProvider::new(responses);
    let contexts = provider.contexts();

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

    start_run(&engine, "run-1");
    start_run(&engine, "run-2");

    let trigger1 = TriggerEvent {
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
    let trigger2 = TriggerEvent {
        run_id: decision_gate_core::RunId::new("run-2"),
        trigger_id: TriggerId::new("trigger-2"),
        time: Timestamp::Logical(2),
        ..trigger1.clone()
    };

    let _ = engine.trigger(&trigger1).unwrap();
    let _ = engine.trigger(&trigger2).unwrap();

    let (first_run, second_run) = {
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 2);
        (contexts[0].run_id.as_str().to_string(), contexts[1].run_id.as_str().to_string())
    };
    assert_eq!(first_run, "run-1");
    assert_eq!(second_run, "run-2");
}
