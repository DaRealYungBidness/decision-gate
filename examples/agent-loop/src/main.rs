// examples/agent-loop/src/main.rs
// ============================================================================
// Module: Decision Gate Agent Loop Example
// Description: Scenario gating that simulates an agent satisfying conditions.
// Purpose: Demonstrate multi-step gate satisfaction with an in-memory provider.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

//! ## Overview
//! This example models an agent loop where conditions are satisfied over time.
//! It updates in-memory signals between `scenario_next` calls to show staged
//! gate progression.

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

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
use decision_gate_core::TriggerId;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use serde_json::json;

/// Error type for example preconditions.
#[derive(Debug)]
struct ExampleError(&'static str);

impl std::fmt::Display for ExampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ExampleError {}

/// Shared agent signals updated by the example.
struct AgentSignals {
    /// Flag indicating the file was written.
    file_written: AtomicBool,
    /// Flag indicating tests passed.
    tests_pass: AtomicBool,
    /// Flag indicating review approval.
    review_approved: AtomicBool,
}

impl AgentSignals {
    /// Creates a new signal set with default values.
    const fn new() -> Self {
        Self {
            file_written: AtomicBool::new(false),
            tests_pass: AtomicBool::new(false),
            review_approved: AtomicBool::new(false),
        }
    }
}

/// Evidence provider backed by the agent signals.
struct AgentEvidenceProvider {
    /// Shared signal state used to answer queries.
    signals: Arc<AgentSignals>,
}

impl AgentEvidenceProvider {
    /// Creates a new provider from the shared signals.
    const fn new(signals: Arc<AgentSignals>) -> Self {
        Self {
            signals,
        }
    }
}

impl EvidenceProvider for AgentEvidenceProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let value = match query.check_id.as_str() {
            "file_exists" => self.signals.file_written.load(Ordering::Relaxed),
            "tests_pass" => self.signals.tests_pass.load(Ordering::Relaxed),
            "review_approved" => self.signals.review_approved.load(Ordering::Relaxed),
            _ => {
                return Err(EvidenceError::Provider(format!("unknown check: {}", query.check_id)));
            }
        };
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!(value))),
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
            dispatcher: "agent-loop".to_string(),
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

/// Builds the agent loop scenario spec.
fn build_spec(namespace_id: NamespaceId) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("agent-loop"),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("main"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("requirements-met"),
                requirement: ret_logic::Requirement::and(vec![
                    ret_logic::Requirement::condition("file_exists".into()),
                    ret_logic::Requirement::condition("tests_pass".into()),
                    ret_logic::Requirement::condition("review_approved".into()),
                ]),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            ConditionSpec {
                condition_id: "file_exists".into(),
                query: EvidenceQuery {
                    provider_id: ProviderId::new("agent"),
                    check_id: "file_exists".to_string(),
                    params: Some(json!({})),
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: "tests_pass".into(),
                query: EvidenceQuery {
                    provider_id: ProviderId::new("agent"),
                    check_id: "tests_pass".to_string(),
                    params: Some(json!({})),
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: "review_approved".into(),
                query: EvidenceQuery {
                    provider_id: ProviderId::new("agent"),
                    check_id: "review_approved".to_string(),
                    params: Some(json!({})),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::from_raw(1).ok_or(ExampleError("tenant id must be nonzero"))?;
    let namespace_id =
        NamespaceId::from_raw(1).ok_or(ExampleError("namespace id must be nonzero"))?;
    let signals = Arc::new(AgentSignals::new());
    let provider = AgentEvidenceProvider::new(signals.clone());
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        build_spec(namespace_id),
        provider,
        ExampleDispatcher,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )?;

    let run_config = RunConfig {
        tenant_id,
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("agent-loop"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false)?;

    let first = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id,
        namespace_id,
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let first_result = engine.scenario_next(&first)?;
    let first_outcome = outcome_summary(&first_result.decision.outcome);
    write_line("First decision", &first_outcome)?;

    signals.file_written.store(true, Ordering::Relaxed);
    signals.tests_pass.store(true, Ordering::Relaxed);
    signals.review_approved.store(true, Ordering::Relaxed);

    let second = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id,
        namespace_id,
        trigger_id: TriggerId::new("trigger-2"),
        agent_id: "agent-1".to_string(),
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
