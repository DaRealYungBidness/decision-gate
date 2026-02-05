// crates/decision-gate-core/tests/submissions.rs
// ============================================================================
// Module: Submission Idempotency Tests
// Description: Ensures scenario_submit is idempotent and detects conflicts.
// ============================================================================
//! ## Overview
//! Validates submission idempotency semantics for deterministic run state.

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
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::ControlPlaneError;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::SubmitRequest;
use serde_json::json;

struct NoopEvidenceProvider;

impl EvidenceProvider for NoopEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
        Err(decision_gate_core::EvidenceError::Provider("unexpected evidence query".to_string()))
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

fn submission_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: vec![PacketSpec {
                packet_id: decision_gate_core::PacketId::new("packet-1"),
                schema_id: decision_gate_core::SchemaId::new("schema-1"),
                content_type: "application/json".to_string(),
                visibility_labels: vec!["public".to_string()],
                policy_tags: Vec::new(),
                expiry: None,
                payload: PacketPayload::Json {
                    value: json!({"message": "hello"}),
                },
            }],
            gates: Vec::new(),
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

#[test]
fn submission_idempotent_returns_existing_record() {
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        submission_spec(),
        NoopEvidenceProvider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

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

    let request = SubmitRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        submission_id: "submission-1".to_string(),
        payload: PacketPayload::Json {
            value: json!({"artifact": "attestation"}),
        },
        content_type: "application/json".to_string(),
        submitted_at: Timestamp::Logical(1),
        correlation_id: None,
    };

    let first = engine.scenario_submit(&request).unwrap();
    let second = engine.scenario_submit(&request).unwrap();

    assert_eq!(first.record, second.record);

    let state = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("run state");
    assert_eq!(state.submissions.len(), 1);
    assert_eq!(state.tool_calls.len(), 2);
}

#[test]
fn submission_idempotent_conflict_returns_error() {
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        submission_spec(),
        NoopEvidenceProvider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

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

    let first = SubmitRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        submission_id: "submission-1".to_string(),
        payload: PacketPayload::Json {
            value: json!({"artifact": "attestation"}),
        },
        content_type: "application/json".to_string(),
        submitted_at: Timestamp::Logical(1),
        correlation_id: None,
    };
    engine.scenario_submit(&first).unwrap();

    let conflicting = SubmitRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        submission_id: "submission-1".to_string(),
        payload: PacketPayload::Json {
            value: json!({"artifact": "different"}),
        },
        content_type: "application/json".to_string(),
        submitted_at: Timestamp::Logical(2),
        correlation_id: None,
    };

    let err = engine.scenario_submit(&conflicting).unwrap_err();
    match err {
        ControlPlaneError::SubmissionConflict(submission_id) => {
            assert_eq!(submission_id, "submission-1");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let state = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("run state");
    assert_eq!(state.submissions.len(), 1);
    assert_eq!(state.tool_calls.len(), 2);
}
