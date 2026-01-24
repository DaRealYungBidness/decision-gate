// examples/data-disclosure/src/main.rs
// ============================================================================
// Module: Decision Gate Data Disclosure Example
// Description: Scenario gating that issues disclosure packets on approval.
// Purpose: Demonstrate stage advancement with packet dispatch.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

//! ## Overview
//! This example models a data disclosure workflow where a policy approval gate
//! unlocks a disclosure stage that emits a packet payload.

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SchemaId;
use decision_gate_core::SchemaRef;
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

/// Shared disclosure signals updated by the example.
struct DisclosureSignals {
    /// Policy approval flag.
    policy_approved: AtomicBool,
}

impl DisclosureSignals {
    /// Creates a new signal set with default values.
    const fn new() -> Self {
        Self {
            policy_approved: AtomicBool::new(false),
        }
    }
}

/// Evidence provider backed by the disclosure signals.
struct DisclosureEvidenceProvider {
    /// Shared signal state used to answer queries.
    signals: Arc<DisclosureSignals>,
}

impl DisclosureEvidenceProvider {
    /// Creates a new provider from the shared signals.
    const fn new(signals: Arc<DisclosureSignals>) -> Self {
        Self {
            signals,
        }
    }
}

impl EvidenceProvider for DisclosureEvidenceProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        if query.predicate.as_str() != "policy_approved" {
            return Err(EvidenceError::Provider(format!("unknown predicate: {}", query.predicate)));
        }
        let approved = self.signals.policy_approved.load(Ordering::Relaxed);
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!(approved))),
            lane: TrustLane::Verified,
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
            dispatcher: "data-disclosure".to_string(),
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

/// Builds the disclosure scenario spec.
fn build_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("data-disclosure"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("review"),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("policy-approved"),
                    requirement: ret_logic::Requirement::predicate("policy_approved".into()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Fixed {
                    stage_id: StageId::new("disclosure"),
                },
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: StageId::new("disclosure"),
                entry_packets: vec![PacketSpec {
                    packet_id: PacketId::new("disclosure-packet"),
                    schema_id: SchemaId::new("document"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["restricted".to_string()],
                    policy_tags: vec!["disclosure".to_string()],
                    expiry: None,
                    payload: PacketPayload::Json {
                        value: json!({
                            "document_id": "doc-42",
                            "classification": "confidential"
                        }),
                    },
                }],
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        predicates: vec![PredicateSpec {
            predicate: "policy_approved".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("policy"),
                predicate: "policy_approved".to_string(),
                params: Some(json!({})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: vec![SchemaRef {
            schema_id: SchemaId::new("document"),
            version: Some("1".to_string()),
            uri: None,
        }],
        default_tenant_id: None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signals = Arc::new(DisclosureSignals::new());
    let provider = DisclosureEvidenceProvider::new(signals.clone());
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
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("data-disclosure"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false)?;

    let first = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let first_result = engine.scenario_next(&first)?;
    let first_outcome = outcome_summary(&first_result.decision.outcome);
    write_line("First decision", &first_outcome)?;

    signals.policy_approved.store(true, Ordering::Relaxed);

    let second = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("trigger-2"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(2),
        correlation_id: None,
    };
    let second_result = engine.scenario_next(&second)?;
    let second_outcome = outcome_summary(&second_result.decision.outcome);
    write_line("Second decision", &second_outcome)?;
    write_line("Packets dispatched", &second_result.packets.len().to_string())?;

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
