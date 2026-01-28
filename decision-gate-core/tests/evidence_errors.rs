// decision-gate-core/tests/evidence_errors.rs
// ============================================================================
// Module: Evidence Error Tests
// Description: Tests for provider error capture in run state.
// ============================================================================
//! ## Overview
//! Ensures provider query errors are recorded for auditability.

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
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceAnchorPolicy;
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
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderAnchorPolicy;
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
use decision_gate_core::TriggerId;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use ret_logic::TriState;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

struct ErroringEvidenceProvider;

impl EvidenceProvider for ErroringEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<decision_gate_core::EvidenceResult, EvidenceError> {
        Err(EvidenceError::Provider("provider unavailable".to_string()))
    }

    fn validate_providers(
        &self,
        _spec: &ScenarioSpec,
    ) -> Result<(), decision_gate_core::ProviderMissingError> {
        Ok(())
    }
}

struct AnchorlessEvidenceProvider;

impl EvidenceProvider for AnchorlessEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
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

fn minimal_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::predicate("ready".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
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
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn provider_errors_are_recorded_in_run_state() {
    let store = InMemoryRunStateStore::new();
    let store_clone = store.clone();
    let engine = ControlPlane::new(
        minimal_spec(),
        ErroringEvidenceProvider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .expect("control plane");

    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: vec![],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false).expect("start run");

    let request = decision_gate_core::runtime::NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let result = engine.scenario_next(&request).expect("scenario next");
    assert_eq!(result.status, decision_gate_core::RunStatus::Active);

    let state = store_clone
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .expect("load state")
        .expect("missing state");
    let evidence = &state.gate_evals[0].evidence[0];
    assert_eq!(evidence.status, TriState::Unknown);
    assert!(evidence.result.value.is_none());
    assert!(evidence.result.evidence_hash.is_none());
    assert!(evidence.result.content_type.is_none());
    let error = evidence.result.error.as_ref().expect("missing error");
    assert_eq!(error.code, "provider_error");
    assert!(error.message.contains("provider unavailable"));
}

#[test]
fn missing_anchors_are_recorded_as_errors() {
    let store = InMemoryRunStateStore::new();
    let store_clone = store.clone();
    let anchor_policy = EvidenceAnchorPolicy {
        providers: vec![ProviderAnchorPolicy {
            provider_id: ProviderId::new("test"),
            requirement: decision_gate_core::AnchorRequirement {
                anchor_type: "assetcore.anchor_set".to_string(),
                required_fields: vec!["assetcore.namespace_id".to_string()],
            },
        }],
    };
    let engine = ControlPlane::new(
        minimal_spec(),
        AnchorlessEvidenceProvider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig {
            anchor_policy,
            ..ControlPlaneConfig::default()
        },
    )
    .expect("control plane");

    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: decision_gate_core::RunId::new("run-anchor"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: vec![],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false).expect("start run");

    let request = decision_gate_core::runtime::NextRequest {
        run_id: decision_gate_core::RunId::new("run-anchor"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-anchor"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let _result = engine.scenario_next(&request).expect("scenario next");

    let state = store_clone
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &decision_gate_core::RunId::new("run-anchor"),
        )
        .expect("load state")
        .expect("missing state");
    let evidence = &state.gate_evals[0].evidence[0];
    assert_eq!(evidence.status, TriState::Unknown);
    assert!(evidence.result.value.is_none());
    assert!(evidence.result.evidence_hash.is_none());
    assert!(evidence.result.content_type.is_none());
    let error = evidence.result.error.as_ref().expect("missing error");
    assert_eq!(error.code, "anchor_invalid");
    assert!(error.message.contains("missing evidence_anchor"));
}
