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

use decision_gate_core::AnchorRequirement;
use decision_gate_core::Artifact;
use decision_gate_core::ArtifactError;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactSink;
use decision_gate_core::ConditionSpec;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceAnchorPolicy;
use decision_gate_core::EvidenceRecord;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateEvalRecord;
use decision_gate_core::GateEvaluation;
use decision_gate_core::GateId;
use decision_gate_core::GateTraceEntry;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderAnchorPolicy;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStatus;
use decision_gate_core::RunpackVersion;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::runtime::RunpackBuilder;
use decision_gate_core::runtime::RunpackVerifier;
use ret_logic::TriState;

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

impl InMemoryArtifactStore {
    fn insert_bytes(&self, path: &str, bytes: Vec<u8>) {
        let mut guard = self.files.lock().expect("artifact store mutex poisoned");
        guard.insert(path.to_string(), bytes);
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn minimal_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn anchor_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("anchor-scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![decision_gate_core::GateSpec {
                gate_id: GateId::new("gate-anchor"),
                requirement: ret_logic::Requirement::condition("anchor_pred".into()),
                trust: None,
            }],
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id: "anchor_pred".into(),
            query: decision_gate_core::EvidenceQuery {
                provider_id: ProviderId::new("assetcore_read"),
                check_id: "balance_amount_scaled".to_string(),
                params: Some(serde_json::json!({"container_id": "vault-001"})),
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(serde_json::json!(1)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn anchor_policy() -> EvidenceAnchorPolicy {
    EvidenceAnchorPolicy {
        providers: vec![ProviderAnchorPolicy {
            provider_id: ProviderId::new("assetcore_read"),
            requirement: AnchorRequirement {
                anchor_type: "assetcore.anchor_set".to_string(),
                required_fields: vec![
                    "assetcore.namespace_id".to_string(),
                    "assetcore.commit_id".to_string(),
                    "assetcore.world_seq".to_string(),
                ],
            },
        }],
    }
}

fn anchor_state(spec: &ScenarioSpec, anchor: Option<EvidenceAnchor>) -> RunState {
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");
    RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: spec.scenario_id.clone(),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
        status: RunStatus::Active,
        dispatch_targets: vec![],
        triggers: vec![],
        gate_evals: vec![GateEvalRecord {
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            stage_id: StageId::new("stage-1"),
            evaluation: GateEvaluation {
                gate_id: GateId::new("gate-anchor"),
                status: TriState::True,
                trace: vec![GateTraceEntry {
                    condition_id: "anchor_pred".into(),
                    status: TriState::True,
                }],
            },
            evidence: vec![EvidenceRecord {
                condition_id: "anchor_pred".into(),
                status: TriState::True,
                result: EvidenceResult {
                    value: Some(EvidenceValue::Json(serde_json::json!(1))),
                    lane: decision_gate_core::TrustLane::Verified,
                    error: None,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: anchor,
                    signature: None,
                    content_type: Some("application/json".to_string()),
                },
            }],
        }],
        decisions: vec![],
        packets: vec![],
        submissions: vec![],
        tool_calls: vec![],
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
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
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

/// Verifies unsupported runpack manifest versions fail closed.
#[test]
fn runpack_verifier_rejects_unknown_manifest_version() {
    let spec = minimal_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");

    let state = RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
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
    let mut manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    manifest.manifest_version = RunpackVersion("v999".to_string());

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert_eq!(report.checked_files, 0);
    assert!(report.errors.iter().any(|err| err.contains("unsupported manifest version")));
}

/// Tests verifier rejection of oversized artifacts.
#[test]
fn runpack_verifier_rejects_oversized_artifact() {
    let spec = minimal_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");

    let state = RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
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

    let oversized = vec![0u8; decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES + 1];
    store.insert_bytes("artifacts/decisions.json", oversized);

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(report.errors.iter().any(|err| err.contains("artifact too large")));
}

/// Verifies anchor policy enforcement fails when anchors are missing.
#[test]
fn runpack_verifier_requires_anchor_policy() {
    let spec = anchor_spec();
    let state = anchor_state(&spec, None);
    let mut store = InMemoryArtifactStore::default();
    let builder = RunpackBuilder::new(anchor_policy());
    let manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(report.errors.iter().any(|err| err.contains("anchor invalid")));
}

/// Verifies anchor policy enforcement passes when anchors are present.
#[test]
fn runpack_verifier_accepts_anchor_policy() {
    let spec = anchor_spec();
    let anchor_value = serde_json::json!({
        "assetcore.namespace_id": 1,
        "assetcore.commit_id": "commit-1",
        "assetcore.world_seq": 42
    });
    let anchor = EvidenceAnchor {
        anchor_type: "assetcore.anchor_set".to_string(),
        anchor_value: serde_json::to_string(&anchor_value).expect("anchor json"),
    };
    let state = anchor_state(&spec, Some(anchor));
    let mut store = InMemoryArtifactStore::default();
    let builder = RunpackBuilder::new(anchor_policy());
    let manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Pass);
}

// ============================================================================
// SECTION: Hash Validation and Tampering Tests
// ============================================================================

/// Verifies that tampering with artifact bytes is detected via hash mismatch.
#[test]
fn runpack_verifier_detects_tampered_artifact() {
    let spec = minimal_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");

    let state = RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
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

    // Tamper with artifact after build
    store.insert_bytes("artifacts/scenario_spec.json", b"tampered content".to_vec());

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(
        report
            .errors
            .iter()
            .any(|err| err.contains("hash mismatch for artifacts/scenario_spec.json")),
        "expected hash mismatch error, got: {:?}",
        report.errors
    );
}

/// Verifies that modifying the root hash in the manifest is detected.
#[test]
fn runpack_verifier_detects_root_hash_mismatch() {
    let spec = minimal_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");

    let state = RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
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
    let mut manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    // Tamper with root hash (provide invalid bytes)
    manifest.integrity.root_hash =
        decision_gate_core::hashing::HashDigest::new(DEFAULT_HASH_ALGORITHM, &[0u8; 32]);

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(
        report.errors.iter().any(|err| err.contains("root hash mismatch")),
        "expected root hash mismatch error, got: {:?}",
        report.errors
    );
}

// Note: Hash algorithm mismatch test is not included because HashAlgorithm currently
// only has one variant (Sha256). When additional algorithms are added, a test should
// be added here to verify the algorithm mismatch detection at runpack.rs:340-342.

/// Verifies that missing artifacts cause verification to fail closed.
#[test]
fn runpack_verifier_fails_on_missing_artifact() {
    let spec = minimal_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");

    let state = RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
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

    // Remove an artifact to simulate missing file
    {
        let mut guard = store.files.lock().expect("artifact store mutex");
        guard.remove("artifacts/scenario_spec.json");
    }

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(
        report
            .errors
            .iter()
            .any(|err| err.contains("artifact read failed for artifacts/scenario_spec.json")),
        "expected artifact read failed error, got: {:?}",
        report.errors
    );
}

// ============================================================================
// SECTION: Anchor Policy Edge Cases
// ============================================================================

/// Verifies that anchors missing required fields fail verification.
#[test]
fn runpack_verifier_rejects_anchor_missing_required_field() {
    let spec = anchor_spec();
    // Create anchor with incomplete field set (missing assetcore.world_seq)
    let anchor_value = serde_json::json!({
        "assetcore.namespace_id": 1,
        "assetcore.commit_id": "commit-1"
        // Missing "assetcore.world_seq"
    });
    let anchor = EvidenceAnchor {
        anchor_type: "assetcore.anchor_set".to_string(),
        anchor_value: serde_json::to_string(&anchor_value).expect("anchor json"),
    };
    let state = anchor_state(&spec, Some(anchor));
    let mut store = InMemoryArtifactStore::default();
    let builder = RunpackBuilder::new(anchor_policy());
    let manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(
        report.errors.iter().any(|err| err.contains("anchor invalid")),
        "expected anchor invalid error, got: {:?}",
        report.errors
    );
}

/// Verifies that anchors with wrong type fail verification.
#[test]
fn runpack_verifier_rejects_anchor_type_mismatch() {
    let spec = anchor_spec();
    let anchor_value = serde_json::json!({
        "assetcore.namespace_id": 1,
        "assetcore.commit_id": "commit-1",
        "assetcore.world_seq": 42
    });
    // Wrong anchor type - policy expects "assetcore.anchor_set"
    let anchor = EvidenceAnchor {
        anchor_type: "assetcore.anchor_invalid".to_string(),
        anchor_value: serde_json::to_string(&anchor_value).expect("anchor json"),
    };
    let state = anchor_state(&spec, Some(anchor));
    let mut store = InMemoryArtifactStore::default();
    let builder = RunpackBuilder::new(anchor_policy());
    let manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(
        report.errors.iter().any(|err| err.contains("anchor invalid")),
        "expected anchor invalid error, got: {:?}",
        report.errors
    );
}

/// Verifies that anchors with invalid field types fail verification.
#[test]
fn runpack_verifier_rejects_anchor_wrong_field_type() {
    let spec = anchor_spec();
    // Provide boolean for world_seq (only string/number are allowed)
    let anchor_value = serde_json::json!({
        "assetcore.namespace_id": 1,
        "assetcore.commit_id": "commit-1",
        "assetcore.world_seq": true
    });
    let anchor = EvidenceAnchor {
        anchor_type: "assetcore.anchor_set".to_string(),
        anchor_value: serde_json::to_string(&anchor_value).expect("anchor json"),
    };
    let state = anchor_state(&spec, Some(anchor));
    let mut store = InMemoryArtifactStore::default();
    let builder = RunpackBuilder::new(anchor_policy());
    let manifest =
        builder.build(&mut store, &spec, &state, Timestamp::Logical(1)).expect("runpack build");

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier.verify_manifest(&store, &manifest).expect("runpack verify");

    assert_eq!(report.status, decision_gate_core::runtime::VerificationStatus::Fail);
    assert!(
        report
            .errors
            .iter()
            .any(|err| err.contains("anchor invalid") || err.contains("must be string or number")),
        "expected anchor invalid error, got: {:?}",
        report.errors
    );
}
