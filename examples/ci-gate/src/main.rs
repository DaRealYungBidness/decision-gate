// examples/ci-gate/src/main.rs
// ============================================================================
// Module: Decision Gate CI Gate Example
// Description: Scenario gating based on CI status and review approvals.
// Purpose: Demonstrate evidence comparisons for CI/CD workflows.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

//! ## Overview
//! This example models a CI gate that requires both a passing CI status and a
//! minimum number of approvals before advancing the scenario.

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
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
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use serde_json::json;

/// Shared CI signal state updated by the example.
struct CiSignals {
    /// CI pass/fail flag.
    ci_passed: AtomicBool,
    /// Approval count for the change.
    approvals: AtomicUsize,
}

impl CiSignals {
    /// Creates a new signal set with default values.
    const fn new() -> Self {
        Self {
            ci_passed: AtomicBool::new(false),
            approvals: AtomicUsize::new(0),
        }
    }
}

/// Evidence provider backed by the shared CI signals.
struct CiEvidenceProvider {
    /// Shared signal state used to answer queries.
    signals: Arc<CiSignals>,
}

impl CiEvidenceProvider {
    /// Creates a new provider from the shared signals.
    const fn new(signals: Arc<CiSignals>) -> Self {
        Self {
            signals,
        }
    }
}

impl EvidenceProvider for CiEvidenceProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        match query.predicate.as_str() {
            "ci_status" => {
                let status = if self.signals.ci_passed.load(Ordering::Relaxed) {
                    "passed"
                } else {
                    "failed"
                };
                Ok(EvidenceResult {
                    value: Some(EvidenceValue::Json(json!(status))),
                    lane: TrustLane::Verified,
                    error: None,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: None,
                    signature: None,
                    content_type: Some("application/json".to_string()),
                })
            }
            "approvals" => {
                let approvals = i64::try_from(self.signals.approvals.load(Ordering::Relaxed))
                    .map_err(|_| EvidenceError::Provider("approval count overflow".to_string()))?;
                Ok(EvidenceResult {
                    value: Some(EvidenceValue::Json(json!(approvals))),
                    lane: TrustLane::Verified,
                    error: None,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: None,
                    signature: None,
                    content_type: Some("application/json".to_string()),
                })
            }
            _ => Err(EvidenceError::Provider(format!("unknown predicate: {}", query.predicate))),
        }
    }

    fn validate_providers(
        &self,
        _spec: &ScenarioSpec,
    ) -> Result<(), decision_gate_core::ProviderMissingError> {
        Ok(())
    }
}

/// Dispatcher that returns a deterministic receipt without delivery.
struct ExampleDispatcher;

impl Dispatcher for ExampleDispatcher {
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
            dispatcher: "ci-gate".to_string(),
        })
    }
}

/// Policy decider that permits all disclosures.
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

/// Builds the CI gate scenario spec.
fn build_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("ci-gate"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("review"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("ci-approved"),
                requirement: ret_logic::Requirement::and(vec![
                    ret_logic::Requirement::predicate("ci_status".into()),
                    ret_logic::Requirement::predicate("approvals".into()),
                ]),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: vec![
            PredicateSpec {
                predicate: "ci_status".into(),
                query: EvidenceQuery {
                    provider_id: ProviderId::new("ci"),
                    predicate: "ci_status".to_string(),
                    params: Some(json!({})),
                },
                comparator: Comparator::Equals,
                expected: Some(json!("passed")),
                policy_tags: Vec::new(),
                trust: None,
            },
            PredicateSpec {
                predicate: "approvals".into(),
                query: EvidenceQuery {
                    provider_id: ProviderId::new("ci"),
                    predicate: "approvals".to_string(),
                    params: Some(json!({})),
                },
                comparator: Comparator::GreaterThanOrEqual,
                expected: Some(json!(2)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signals = Arc::new(CiSignals::new());
    let provider = CiEvidenceProvider::new(signals.clone());
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        build_spec(),
        provider,
        ExampleDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )?;

    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("ci-gate"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "ci-bot".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false)?;

    let first = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "ci-bot".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let first_result = engine.scenario_next(&first)?;
    let first_outcome = outcome_summary(&first_result.decision.outcome);
    write_line("First decision", &first_outcome)?;

    signals.ci_passed.store(true, Ordering::Relaxed);
    signals.approvals.store(2, Ordering::Relaxed);

    let second = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        trigger_id: TriggerId::new("trigger-2"),
        agent_id: "ci-bot".to_string(),
        time: Timestamp::Logical(2),
        correlation_id: None,
    };
    let second_result = engine.scenario_next(&second)?;
    let second_outcome = outcome_summary(&second_result.decision.outcome);
    write_line("Second decision", &second_outcome)?;

    Ok(())
}

/// Formats a short summary for the decision outcome.
fn outcome_summary(outcome: &DecisionOutcome) -> String {
    match outcome {
        DecisionOutcome::Start {
            stage_id,
        } => format!("start:{stage_id}"),
        DecisionOutcome::Complete {
            stage_id,
        } => format!("complete:{stage_id}"),
        DecisionOutcome::Advance {
            from_stage,
            to_stage,
            timeout,
        } => {
            let reason = if *timeout { "timeout" } else { "gate" };
            format!("advance:{from_stage}->{to_stage} ({reason})")
        }
        DecisionOutcome::Hold {
            summary,
        } => format!("hold:{}", summary.status),
        DecisionOutcome::Fail {
            reason,
        } => format!("fail:{reason}"),
    }
}

/// Writes a labeled line to stdout.
fn write_line(label: &str, value: &str) -> Result<(), std::io::Error> {
    let mut out = std::io::stdout();
    writeln!(out, "{label}: {value}")?;
    Ok(())
}
