// decision-gate-core/tests/limits.rs
// ============================================================================
// Module: Size Limit Tests
// Description: Tests for evidence and payload size caps.
// ============================================================================
//! ## Overview
//! Ensures evidence and payload hashing enforces hard byte caps and fails closed.

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
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::ControlPlaneError;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::MAX_EVIDENCE_VALUE_BYTES;
use decision_gate_core::runtime::MAX_PAYLOAD_BYTES;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

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

struct LargeEvidenceProvider {
    size: usize,
}

impl EvidenceProvider for LargeEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Bytes(vec![0u8; self.size])),
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: Some("application/octet-stream".to_string()),
        })
    }

    fn validate_providers(
        &self,
        _spec: &ScenarioSpec,
    ) -> Result<(), decision_gate_core::ProviderMissingError> {
        Ok(())
    }
}

fn minimal_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::predicate("ready".into()),
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
fn evidence_payload_size_limit_is_enforced() {
    let store = InMemoryRunStateStore::new();
    let provider = LargeEvidenceProvider {
        size: MAX_EVIDENCE_VALUE_BYTES + 1,
    };
    let engine = ControlPlane::new(
        minimal_spec(),
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .expect("control plane");

    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: vec![],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false).expect("start run");

    let request = decision_gate_core::runtime::NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let err = engine.scenario_next(&request).expect_err("expected size limit error");
    assert!(matches!(err, ControlPlaneError::EvidenceTooLarge { .. }));
}

#[test]
fn payload_size_limit_is_enforced() {
    let store = InMemoryRunStateStore::new();
    let provider = LargeEvidenceProvider {
        size: 1,
    };
    let engine = ControlPlane::new(
        minimal_spec(),
        provider,
        NoopDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .expect("control plane");

    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        run_id: decision_gate_core::RunId::new("run-2"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: vec![],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false).expect("start run");

    let request = decision_gate_core::runtime::SubmitRequest {
        run_id: decision_gate_core::RunId::new("run-2"),
        submission_id: "submission-1".to_string(),
        payload: PacketPayload::Bytes {
            bytes: vec![0u8; MAX_PAYLOAD_BYTES + 1],
        },
        content_type: "application/octet-stream".to_string(),
        submitted_at: Timestamp::Logical(1),
        correlation_id: None,
    };

    let err = engine.scenario_submit(&request).expect_err("expected payload limit error");
    assert!(matches!(err, ControlPlaneError::PayloadTooLarge { .. }));
}
