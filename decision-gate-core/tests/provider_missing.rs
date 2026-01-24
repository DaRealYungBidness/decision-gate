// decision-gate-core/tests/provider_missing.rs
// ============================================================================
// Module: Provider Missing Tests
// Description: Validate preflight failures for missing evidence providers.
// Purpose: Ensure provider validation fails fast and logs tool-call errors.
// Dependencies: decision-gate-core, ret-logic, serde_json
// ============================================================================
//! ## Overview
//! Ensures provider validation fails fast and logs tool-call errors when providers
//! are missing or blocked.

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
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::ToolCallErrorDetails;
use decision_gate_core::TriggerId;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::ControlPlaneError;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use serde_json::json;

struct MissingProvider;

impl EvidenceProvider for MissingProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &decision_gate_core::EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
        Err(decision_gate_core::EvidenceError::Provider("unexpected query".to_string()))
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Err(ProviderMissingError {
            missing_providers: vec!["missing".to_string()],
            required_capabilities: vec!["predicate".to_string()],
            blocked_by_policy: false,
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

fn missing_provider_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::predicate("needs_provider".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
            predicate: "needs_provider".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("missing"),
                predicate: "exists".to_string(),
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

struct NoopDispatcher;

impl Dispatcher for NoopDispatcher {
    fn dispatch(
        &self,
        _target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<decision_gate_core::DispatchReceipt, decision_gate_core::DispatchError> {
        Err(decision_gate_core::DispatchError::DispatchFailed(
            "dispatch should not be called".to_string(),
        ))
    }
}

/// Tests scenario next logs missing provider error.
#[test]
fn scenario_next_logs_missing_provider_error() {
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        missing_provider_spec(),
        MissingProvider,
        NoopDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )
    .unwrap();

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

    let request = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let error = engine.scenario_next(&request).expect_err("expected provider missing error");
    match error {
        ControlPlaneError::ProviderMissing(err) => {
            assert_eq!(err.missing_providers, vec!["missing".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let state = store
        .load(
            &TenantId::new("tenant"),
            &NamespaceId::new("default"),
            &decision_gate_core::RunId::new("run-1"),
        )
        .unwrap()
        .expect("run state");
    assert_eq!(state.tool_calls.len(), 1);
    let call = &state.tool_calls[0];
    let details = call.error.as_ref().expect("tool error").details.as_ref().expect("details");
    match details {
        ToolCallErrorDetails::ProviderMissing(missing) => {
            assert_eq!(missing.missing_providers, vec!["missing".to_string()]);
            assert!(!missing.blocked_by_policy);
        }
        ToolCallErrorDetails::Message {
            ..
        } => panic!("unexpected details"),
    }
}
