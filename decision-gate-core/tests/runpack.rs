// decision-gate-core/tests/runpack.rs
// ============================================================================
// Module: Runpack Tests
// Description: Tests for runpack generation and verification.
// ============================================================================
//! ## Overview
//! Validates deterministic runpack exports and verifier behavior.

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

use std::collections::BTreeMap;
use std::sync::Mutex;

use decision_gate_core::Artifact;
use decision_gate_core::ArtifactError;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactSink;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::runtime::RunpackBuilder;
use decision_gate_core::runtime::RunpackVerifier;

// ============================================================================
// SECTION: In-Memory Artifact Store
// ============================================================================

#[derive(Default)]
struct InMemoryArtifactStore {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl ArtifactSink for InMemoryArtifactStore {
    fn write(
        &mut self,
        artifact: &Artifact,
    ) -> Result<decision_gate_core::ArtifactRef, ArtifactError> {
        {
            let mut guard = self
                .files
                .lock()
                .map_err(|_| ArtifactError::Sink("artifact store mutex poisoned".to_string()))?;
            guard.insert(artifact.path.clone(), artifact.bytes.clone());
        }
        Ok(decision_gate_core::ArtifactRef {
            uri: artifact.path.clone(),
        })
    }

    fn finalize(
        &mut self,
        manifest: &decision_gate_core::RunpackManifest,
    ) -> Result<decision_gate_core::ArtifactRef, ArtifactError> {
        let bytes =
            serde_jcs::to_vec(manifest).map_err(|err| ArtifactError::Sink(err.to_string()))?;
        {
            let mut guard = self
                .files
                .lock()
                .map_err(|_| ArtifactError::Sink("artifact store mutex poisoned".to_string()))?;
            guard.insert("run_manifest.json".to_string(), bytes);
        }
        Ok(decision_gate_core::ArtifactRef {
            uri: "run_manifest.json".to_string(),
        })
    }
}

impl ArtifactReader for InMemoryArtifactStore {
    fn read(&self, path: &str) -> Result<Vec<u8>, ArtifactError> {
        let guard = self
            .files
            .lock()
            .map_err(|_| ArtifactError::Sink("artifact store mutex poisoned".to_string()))?;
        guard.get(path).cloned().ok_or_else(|| ArtifactError::Sink("missing artifact".to_string()))
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn minimal_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Tests runpack build and verify.
#[test]
fn test_runpack_build_and_verify() {
    let spec = minimal_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");

    let state = RunState {
        tenant_id: TenantId::new("tenant"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        status: RunStatus::Active,
        dispatch_targets: vec![],
        triggers: vec![],
        gate_evals: vec![],
        decisions: vec![],
        packets: vec![],
        submissions: vec![],
        tool_calls: vec![],
    };

    let mut store = InMemoryArtifactStore::default();
    let builder = RunpackBuilder::default();
    let manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Pass);
}
