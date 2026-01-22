#![cfg_attr(
    test,
    allow(
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
    )
)]
// examples/llm-scenario/src/main.rs
// ============================================================================
// Module: Decision Gate LLM Scenario Example
// Description: Demonstrates a Decision Gate disclosure flow into an LLM handler.
// Purpose: Show broker integration with a callback sink and model submissions.
// Dependencies: decision-gate-core, decision-gate-broker, ret-logic
// ============================================================================

//! ## Overview
//! Runs a Decision Gate scenario that dispatches a prompt payload to a callback
//! sink (simulating an LLM integration) and records a model submission.

use std::io::Write;

use decision_gate_broker::CallbackSink;
use decision_gate_broker::CompositeBroker;
use decision_gate_broker::PayloadBody;
use decision_gate_broker::SinkError;
use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
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
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::SubmitRequest;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use serde_json::json;

/// Evidence provider that always returns `true`.
struct ExampleEvidenceProvider;

impl EvidenceProvider for ExampleEvidenceProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, decision_gate_core::EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
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

/// Builds the LLM scenario spec.
fn build_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("llm-scenario"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-ready"),
                    requirement: ret_logic::Requirement::predicate("ready".into()),
                }],
                advance_to: AdvanceTo::Linear,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: StageId::new("stage-2"),
                entry_packets: vec![PacketSpec {
                    packet_id: decision_gate_core::PacketId::new("packet-prompt"),
                    schema_id: SchemaId::new("schema-prompt"),
                    content_type: "application/json".to_string(),
                    visibility_labels: vec!["internal".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: PacketPayload::Json {
                        value: json!({"prompt": "Summarize the last runpack."}),
                    },
                }],
                gates: Vec::new(),
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        predicates: vec![PredicateSpec {
            predicate: "ready".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("example"),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sink = CallbackSink::new(|target, payload| {
        let target_label = target_label(target);
        let payload_label = payload_summary(&payload.body)?;
        write_line("Dispatching to LLM target", &target_label)?;
        write_line("Payload", &payload_label)?;
        Ok(DispatchReceipt {
            dispatch_id: "llm-1".to_string(),
            target: target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "llm-callback".to_string(),
        })
    });

    let broker = CompositeBroker::builder().sink(sink).build()?;

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        build_spec(),
        ExampleEvidenceProvider,
        broker,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )?;

    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("llm-scenario"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false)?;

    let request = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let result = engine.scenario_next(&request)?;
    let outcome = outcome_summary(&result.decision.outcome);
    write_line("Decision", &outcome)?;

    let submission = SubmitRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        submission_id: "submission-1".to_string(),
        payload: PacketPayload::Json {
            value: json!({"response": "Summary goes here."}),
        },
        content_type: "application/json".to_string(),
        submitted_at: Timestamp::Logical(2),
        correlation_id: None,
    };
    let submit_result = engine.scenario_submit(&submission)?;
    write_line("Recorded submission", &submit_result.record.submission_id)?;

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

/// Formats a short label for the dispatch target.
fn target_label(target: &DispatchTarget) -> String {
    match target {
        DispatchTarget::Agent {
            agent_id,
        } => format!("agent:{agent_id}"),
        DispatchTarget::Session {
            session_id,
        } => format!("session:{session_id}"),
        DispatchTarget::External {
            system,
            target,
        } => format!("external:{system}:{target}"),
        DispatchTarget::Channel {
            channel,
        } => format!("channel:{channel}"),
    }
}

/// Formats a short summary for the payload body.
fn payload_summary(body: &PayloadBody) -> Result<String, SinkError> {
    match body {
        PayloadBody::Json(value) => {
            let encoded = serde_json::to_string(value)
                .map_err(|err| SinkError::DeliveryFailed(err.to_string()))?;
            Ok(format!("json:{encoded}"))
        }
        PayloadBody::Bytes(bytes) => Ok(format!("bytes:{} bytes", bytes.len())),
    }
}

/// Writes a labeled line to stdout.
fn write_line(label: &str, value: &str) -> Result<(), SinkError> {
    let mut out = std::io::stdout();
    writeln!(out, "{label}: {value}").map_err(|err| SinkError::DeliveryFailed(err.to_string()))
}
