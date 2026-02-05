// crates/decision-gate-core/examples/minimal.rs
// ============================================================================
// Module: Decision Gate Minimal Example
// Description: Minimal end-to-end Decision Gate run using in-memory adapters.
// Purpose: Demonstrate scenario.next/status and runpack generation.
// Dependencies: decision-gate-core
// ============================================================================

//! ## Overview
//! Runs a minimal Decision Gate scenario using in-memory evidence and dispatch adapters.
//! This example is backend-agnostic and suitable for quick verification.

use std::collections::BTreeMap;
use std::sync::Mutex;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Artifact;
use decision_gate_core::ArtifactError;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactRef;
use decision_gate_core::ArtifactSink;
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
use decision_gate_core::RunStateStore;
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
use decision_gate_core::runtime::RunpackBuilder;
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

/// Evidence provider that returns a fixed JSON value.
struct ExampleEvidenceProvider;

impl EvidenceProvider for ExampleEvidenceProvider {
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

/// Dispatcher that returns a synthetic receipt for each target.
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
            dispatcher: "example".to_string(),
        })
    }
}

/// Policy decider that permits every dispatch.
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

/// In-memory artifact sink/reader used by the example.
#[derive(Default)]
struct InMemoryArtifacts {
    /// Stored artifacts keyed by path.
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl ArtifactSink for InMemoryArtifacts {
    fn write(&mut self, artifact: &Artifact) -> Result<ArtifactRef, ArtifactError> {
        {
            let mut guard = self
                .files
                .lock()
                .map_err(|_| ArtifactError::Sink("artifact store mutex poisoned".to_string()))?;
            guard.insert(artifact.path.clone(), artifact.bytes.clone());
        }
        Ok(ArtifactRef {
            uri: artifact.path.clone(),
        })
    }

    fn finalize(
        &mut self,
        manifest: &decision_gate_core::RunpackManifest,
    ) -> Result<ArtifactRef, ArtifactError> {
        let bytes =
            serde_jcs::to_vec(manifest).map_err(|err| ArtifactError::Sink(err.to_string()))?;
        {
            let mut guard = self
                .files
                .lock()
                .map_err(|_| ArtifactError::Sink("artifact store mutex poisoned".to_string()))?;
            guard.insert("run_manifest.json".to_string(), bytes);
        }
        Ok(ArtifactRef {
            uri: "run_manifest.json".to_string(),
        })
    }
}

impl ArtifactReader for InMemoryArtifacts {
    fn read_with_limit(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, ArtifactError> {
        let bytes = {
            let guard = self
                .files
                .lock()
                .map_err(|_| ArtifactError::Sink("artifact store mutex poisoned".to_string()))?;
            guard
                .get(path)
                .cloned()
                .ok_or_else(|| ArtifactError::Sink("missing artifact".to_string()))?
        };
        if bytes.len() > max_bytes {
            return Err(ArtifactError::TooLarge {
                path: path.to_string(),
                max_bytes,
                actual_bytes: bytes.len(),
            });
        }
        Ok(bytes)
    }
}

/// Builds the minimal scenario specification for the example run.
fn build_spec(namespace_id: NamespaceId) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("example"),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
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
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-ready"),
                requirement: ret_logic::Requirement::condition("ready".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id: "ready".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("example"),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::from_raw(1).ok_or(ExampleError("tenant id must be nonzero"))?;
    let namespace_id =
        NamespaceId::from_raw(1).ok_or(ExampleError("namespace id must be nonzero"))?;
    let spec = build_spec(namespace_id);
    let store = InMemoryRunStateStore::new();
    let engine = ControlPlane::new(
        spec.clone(),
        ExampleEvidenceProvider,
        ExampleDispatcher,
        store.clone(),
        Some(PermitAllPolicy),
        ControlPlaneConfig::default(),
    )?;

    let run_config = RunConfig {
        tenant_id,
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("example"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        }],
        policy_tags: Vec::new(),
    };

    engine.start_run(run_config, Timestamp::Logical(0), true)?;

    let next_request = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id,
        namespace_id,
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let result = engine.scenario_next(&next_request)?;

    let status_request = decision_gate_core::runtime::StatusRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id,
        namespace_id,
        requested_at: Timestamp::Logical(2),
        correlation_id: None,
    };
    let status = engine.scenario_status(&status_request)?;

    let _ = (result, status);

    let run_state = store
        .load(&tenant_id, &namespace_id, &decision_gate_core::RunId::new("run-1"))?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "run state missing"))?;

    let mut artifacts = InMemoryArtifacts::default();
    let builder = RunpackBuilder::default();
    let _manifest = builder.build(&mut artifacts, &spec, &run_state, Timestamp::Logical(2))?;
    Ok(())
}
