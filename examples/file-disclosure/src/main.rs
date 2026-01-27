// examples/file-disclosure/src/main.rs
// ============================================================================
// Module: Decision Gate File Disclosure Example
// Description: Demonstrates external file payload resolution via the broker.
// Purpose: Show end-to-end disclosure using FileSource and LogSink.
// Dependencies: decision-gate-core, decision-gate-broker, ret-logic
// ============================================================================

//! ## Overview
//! Runs a Decision Gate scenario that discloses a file-backed payload using the
//! broker's `FileSource` and `LogSink` implementations.

use std::io::Write;

use decision_gate_broker::CompositeBroker;
use decision_gate_broker::FileSource;
use decision_gate_broker::LogSink;
use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ContentRef;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceContext;
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
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
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
use tempfile::tempdir;
use url::Url;

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

/// Builds the file disclosure scenario spec.
fn build_spec(content_ref: ContentRef) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("file-disclosure"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![
            StageSpec {
                stage_id: StageId::new("stage-1"),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-ready"),
                    requirement: ret_logic::Requirement::predicate("ready".into()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Linear,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            StageSpec {
                stage_id: StageId::new("stage-2"),
                entry_packets: vec![PacketSpec {
                    packet_id: decision_gate_core::PacketId::new("packet-file"),
                    schema_id: SchemaId::new("schema-file"),
                    content_type: "application/octet-stream".to_string(),
                    visibility_labels: vec!["restricted".to_string()],
                    policy_tags: Vec::new(),
                    expiry: None,
                    payload: PacketPayload::External {
                        content_ref,
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
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let path = dir.path().join("payload.bin");
    std::fs::write(&path, b"file disclosure payload")?;

    let uri = Url::from_file_path(&path)
        .map_err(|()| std::io::Error::new(std::io::ErrorKind::InvalidInput, "file url failed"))?
        .to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"file disclosure payload");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(LogSink::new(std::io::stdout()))
        .build()?;

    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        build_spec(content_ref),
        ExampleEvidenceProvider,
        broker,
        store,
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )?;

    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("file-disclosure"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), false)?;

    let request = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let result = engine.scenario_next(&request)?;
    let outcome = outcome_summary(&result.decision.outcome);
    write_line("Decision", &outcome)?;

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
