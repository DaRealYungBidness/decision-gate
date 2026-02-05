// crates/decision-gate-core/tests/control_plane.rs
// ============================================================================
// Module: Control Plane Tests
// Description: Tests for trigger idempotency and stage advancement.
// ============================================================================
//! ## Overview
//! Validates that duplicate triggers do not double-advance or duplicate packets.

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
use decision_gate_core::ConditionSpec;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SchemaId;
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
use decision_gate_core::runtime::NextRequest;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

struct TestEvidenceProvider;

impl EvidenceProvider for TestEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &decision_gate_core::EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
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

fn sample_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-ready"),
                    requirement: ret_logic::Requirement::condition("ready".into()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Linear,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
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
                        value: json!({"hello": "world"}),
                    },
                }],
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
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

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Tests trigger idempotency.
#[test]
fn test_trigger_idempotency() {
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        sample_spec(),
        TestEvidenceProvider,
        TestDispatcher,
        store,
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

    let request = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let result_a = engine.scenario_next(&request).unwrap();
    let result_b = engine.scenario_next(&request).unwrap();

    assert_eq!(result_a.decision, result_b.decision);
    assert_eq!(result_a.packets.len(), 1);
    assert_eq!(result_b.packets.len(), 1);

    let status_request = decision_gate_core::runtime::StatusRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        requested_at: Timestamp::Logical(2),
        correlation_id: None,
    };
    let status = engine.scenario_status(&status_request).unwrap();
    assert_eq!(status.issued_packet_ids.len(), 1);
}

/// Tests scenario next completes after terminal stage evaluation.
#[test]
fn scenario_next_completes_after_terminal_stage_evaluation() {
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        sample_spec(),
        TestEvidenceProvider,
        TestDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: decision_gate_core::RunId::new("run-2"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false).unwrap();

    let first = NextRequest {
        run_id: decision_gate_core::RunId::new("run-2"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let result_first = engine.scenario_next(&first).unwrap();
    assert_eq!(result_first.status, RunStatus::Active);

    let second = NextRequest {
        run_id: decision_gate_core::RunId::new("run-2"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-2"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(2),
        correlation_id: None,
    };
    let result_second = engine.scenario_next(&second).unwrap();
    assert_eq!(result_second.status, RunStatus::Completed);
}
