// decision-gate-core/src/runtime/runpack.rs
// ============================================================================
// Module: Decision Gate Runpack Builder and Verifier
// Description: Deterministic runpack generation and offline verification.
// Purpose: Export and validate Decision Gate run artifacts with canonical hashing.
// Dependencies: crate::{core, interfaces}, serde
// ============================================================================

//! ## Overview
//! Runpack generation exports scenario specs, logs, and disclosures into a
//! deterministic artifact bundle. The verifier replays integrity checks and
//! enforces fail-closed behavior for missing or tampered artifacts.
//!
//! Security posture: runpack verification treats artifacts as untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::core::AnchorRequirement;
use crate::core::ConditionId;
use crate::core::DecisionOutcome;
use crate::core::DecisionRecord;
use crate::core::EvidenceAnchorPolicy;
use crate::core::EvidenceResult;
use crate::core::GateEvalRecord;
use crate::core::ProviderId;
use crate::core::RunState;
use crate::core::ScenarioSpec;
use crate::core::Timestamp;
use crate::core::hashing::DEFAULT_HASH_ALGORITHM;
use crate::core::hashing::HashAlgorithm;
use crate::core::hashing::hash_bytes;
use crate::core::hashing::hash_canonical_json;
use crate::core::runpack::ArtifactKind;
use crate::core::runpack::ArtifactRecord;
use crate::core::runpack::FileHashEntry;
use crate::core::runpack::RunpackIntegrity;
use crate::core::runpack::RunpackManifest;
use crate::core::runpack::RunpackSecurityContext;
use crate::core::runpack::RunpackVersion;
use crate::core::runpack::VerifierMode;
use crate::interfaces::Artifact;
use crate::interfaces::ArtifactError;
use crate::interfaces::ArtifactReader;
use crate::interfaces::ArtifactSink;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Runpack path for the scenario specification artifact.
const SCENARIO_SPEC_PATH: &str = "artifacts/scenario_spec.json";
/// Runpack path for trigger logs.
const TRIGGER_LOG_PATH: &str = "artifacts/triggers.json";
/// Runpack path for gate evaluation logs.
const GATE_EVAL_LOG_PATH: &str = "artifacts/gate_evals.json";
/// Runpack path for decision logs.
const DECISION_LOG_PATH: &str = "artifacts/decisions.json";
/// Runpack path for packet logs.
const PACKET_LOG_PATH: &str = "artifacts/packets.json";
/// Runpack path for submission logs.
const SUBMISSION_LOG_PATH: &str = "artifacts/submissions.json";
/// Runpack path for tool call logs.
const TOOL_LOG_PATH: &str = "artifacts/tool_calls.json";
/// Runpack path for verifier reports.
const VERIFIER_REPORT_PATH: &str = "artifacts/verifier_report.json";
/// Maximum artifact size accepted by the runpack verifier.
pub const MAX_RUNPACK_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

// ============================================================================
// SECTION: Builder
// ============================================================================

/// Decision Gate runpack builder for deterministic exports.
#[derive(Debug, Clone)]
pub struct RunpackBuilder {
    /// Manifest version identifier.
    pub manifest_version: RunpackVersion,
    /// Hash algorithm used for runpack artifacts.
    pub hash_algorithm: HashAlgorithm,
    /// Verifier mode used by the runpack.
    pub verifier_mode: VerifierMode,
    /// Anchor policy enforced by the control plane.
    pub anchor_policy: EvidenceAnchorPolicy,
    /// Optional security context metadata.
    pub security_context: Option<RunpackSecurityContext>,
}

impl Default for RunpackBuilder {
    fn default() -> Self {
        Self {
            manifest_version: RunpackVersion("v1".to_string()),
            hash_algorithm: DEFAULT_HASH_ALGORITHM,
            verifier_mode: VerifierMode::OfflineStrict,
            anchor_policy: EvidenceAnchorPolicy::default(),
            security_context: None,
        }
    }
}

impl RunpackBuilder {
    /// Creates a new builder with an explicit anchor policy.
    #[must_use]
    pub fn new(anchor_policy: EvidenceAnchorPolicy) -> Self {
        Self {
            anchor_policy,
            ..Self::default()
        }
    }

    /// Sets the security context metadata for the runpack.
    #[must_use]
    pub fn with_security_context(mut self, context: RunpackSecurityContext) -> Self {
        self.security_context = Some(context);
        self
    }

    /// Builds a runpack and writes artifacts to the provided sink.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackError`] when runpack generation fails.
    pub fn build<S: ArtifactSink>(
        &self,
        sink: &mut S,
        spec: &ScenarioSpec,
        state: &RunState,
        generated_at: Timestamp,
    ) -> Result<RunpackManifest, RunpackError> {
        let spec_hash = spec
            .canonical_hash_with(self.hash_algorithm)
            .map_err(|err| RunpackError::Hash(err.to_string()))?;
        if spec_hash != state.spec_hash {
            return Err(RunpackError::Hash("run state spec hash mismatch".to_string()));
        }

        let mut artifacts = Vec::new();
        let mut file_hashes = Vec::new();

        write_json_artifact(
            sink,
            spec,
            SCENARIO_SPEC_PATH,
            ArtifactKind::ScenarioSpec,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;
        write_json_artifact(
            sink,
            &state.triggers,
            TRIGGER_LOG_PATH,
            ArtifactKind::TriggerLog,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;
        write_json_artifact(
            sink,
            &state.gate_evals,
            GATE_EVAL_LOG_PATH,
            ArtifactKind::GateEvalLog,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;
        write_json_artifact(
            sink,
            &state.decisions,
            DECISION_LOG_PATH,
            ArtifactKind::DecisionLog,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;
        write_json_artifact(
            sink,
            &state.packets,
            PACKET_LOG_PATH,
            ArtifactKind::PacketLog,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;
        write_json_artifact(
            sink,
            &state.submissions,
            SUBMISSION_LOG_PATH,
            ArtifactKind::SubmissionLog,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;
        write_json_artifact(
            sink,
            &state.tool_calls,
            TOOL_LOG_PATH,
            ArtifactKind::ToolTranscript,
            &mut artifacts,
            &mut file_hashes,
            self.hash_algorithm,
        )?;

        let integrity = build_integrity(&file_hashes, self.hash_algorithm)?;

        let anchor_policy = if self.anchor_policy.providers.is_empty() {
            None
        } else {
            Some(self.anchor_policy.clone())
        };

        let manifest = RunpackManifest {
            manifest_version: self.manifest_version.clone(),
            generated_at,
            scenario_id: spec.scenario_id.clone(),
            tenant_id: state.tenant_id,
            namespace_id: state.namespace_id,
            run_id: state.run_id.clone(),
            spec_hash,
            hash_algorithm: self.hash_algorithm,
            verifier_mode: self.verifier_mode,
            anchor_policy,
            security: self.security_context.clone(),
            integrity,
            artifacts,
        };

        sink.finalize(&manifest)?;
        Ok(manifest)
    }

    /// Builds a runpack and includes an offline verification report.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackError`] when runpack generation or verification fails.
    pub fn build_with_verification<S: ArtifactSink, R: ArtifactReader>(
        &self,
        sink: &mut S,
        reader: &R,
        spec: &ScenarioSpec,
        state: &RunState,
        generated_at: Timestamp,
    ) -> Result<(RunpackManifest, VerificationReport), RunpackError> {
        let mut manifest = self.build(sink, spec, state, generated_at)?;
        let verifier = RunpackVerifier::new(self.hash_algorithm);
        let report = verifier.verify_manifest(reader, &manifest)?;

        let report_bytes = serde_jcs::to_vec(&report)
            .map_err(|err| RunpackError::Serialization(err.to_string()))?;
        let report_hash = hash_bytes(self.hash_algorithm, &report_bytes);
        let artifact = Artifact {
            kind: ArtifactKind::VerifierReport,
            path: VERIFIER_REPORT_PATH.to_string(),
            content_type: Some("application/json".to_string()),
            bytes: report_bytes,
            required: true,
        };
        sink.write(&artifact)?;

        manifest.artifacts.push(ArtifactRecord {
            artifact_id: VERIFIER_REPORT_PATH.to_string(),
            kind: ArtifactKind::VerifierReport,
            path: VERIFIER_REPORT_PATH.to_string(),
            content_type: Some("application/json".to_string()),
            hash: report_hash.clone(),
            required: true,
        });
        manifest.integrity.file_hashes.push(FileHashEntry {
            path: VERIFIER_REPORT_PATH.to_string(),
            hash: report_hash,
        });
        manifest.integrity = build_integrity(&manifest.integrity.file_hashes, self.hash_algorithm)?;
        sink.finalize(&manifest)?;

        Ok((manifest, report))
    }
}

// ============================================================================
// SECTION: Verifier
// ============================================================================

/// Runpack verifier for offline validation.
pub struct RunpackVerifier {
    /// Hash algorithm used for verification.
    hash_algorithm: HashAlgorithm,
}

impl RunpackVerifier {
    /// Creates a new verifier.
    #[must_use]
    pub const fn new(hash_algorithm: HashAlgorithm) -> Self {
        Self {
            hash_algorithm,
        }
    }

    /// Verifies a runpack manifest using the provided artifact reader.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackError`] when verification fails.
    pub fn verify_manifest<R: ArtifactReader>(
        &self,
        reader: &R,
        manifest: &RunpackManifest,
    ) -> Result<VerificationReport, RunpackError> {
        let mut errors = Vec::new();
        let mut checked = 0usize;

        if manifest.hash_algorithm != self.hash_algorithm {
            errors.push("hash algorithm mismatch".to_string());
        }

        for entry in &manifest.integrity.file_hashes {
            match reader.read_with_limit(&entry.path, MAX_RUNPACK_ARTIFACT_BYTES) {
                Ok(bytes) => {
                    let actual = hash_bytes(self.hash_algorithm, &bytes);
                    if actual != entry.hash {
                        errors.push(format!("hash mismatch for {}", entry.path));
                    }
                    checked = checked.saturating_add(1);
                }
                Err(err) => {
                    errors.push(format!("artifact read failed for {}: {}", entry.path, err));
                }
            }
        }

        if let Ok(root_hash) =
            hash_canonical_json(self.hash_algorithm, &manifest.integrity.file_hashes)
        {
            if root_hash != manifest.integrity.root_hash {
                errors.push("root hash mismatch".to_string());
            }
        } else {
            errors.push("failed to compute root hash".to_string());
        }

        if let Err(err) = verify_decisions(reader) {
            errors.push(err);
        }
        if let Some(anchor_policy) = &manifest.anchor_policy {
            match verify_anchor_policy(reader, manifest, anchor_policy) {
                Ok(anchor_errors) => errors.extend(anchor_errors),
                Err(err) => errors.push(err),
            }
        }

        let status =
            if errors.is_empty() { VerificationStatus::Pass } else { VerificationStatus::Fail };

        Ok(VerificationReport {
            status,
            checked_files: checked,
            errors,
        })
    }
}

// ============================================================================
// SECTION: Verification Types
// ============================================================================

/// Verification status for runpack reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Verification succeeded.
    Pass,
    /// Verification failed.
    Fail,
}

/// Offline verification report for runpacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    /// Verification status.
    pub status: VerificationStatus,
    /// Count of checked files.
    pub checked_files: usize,
    /// Error messages, if any.
    pub errors: Vec<String>,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Runpack generation or verification errors.
#[derive(Debug, Error)]
pub enum RunpackError {
    /// Artifact errors.
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
    /// Hashing errors.
    #[error("hashing error: {0}")]
    Hash(String),
    /// Serialization errors.
    #[error("serialization error: {0}")]
    Serialization(String),
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Writes a JSON artifact into the runpack and updates hashes.
fn write_json_artifact<S: ArtifactSink, T: Serialize>(
    sink: &mut S,
    value: &T,
    path: &str,
    kind: ArtifactKind,
    artifacts: &mut Vec<ArtifactRecord>,
    file_hashes: &mut Vec<FileHashEntry>,
    algorithm: HashAlgorithm,
) -> Result<(), RunpackError> {
    let bytes =
        serde_jcs::to_vec(value).map_err(|err| RunpackError::Serialization(err.to_string()))?;
    let hash = hash_bytes(algorithm, &bytes);
    let artifact = Artifact {
        kind,
        path: path.to_string(),
        content_type: Some("application/json".to_string()),
        bytes,
        required: true,
    };
    sink.write(&artifact)?;

    artifacts.push(ArtifactRecord {
        artifact_id: path.to_string(),
        kind,
        path: path.to_string(),
        content_type: Some("application/json".to_string()),
        hash: hash.clone(),
        required: true,
    });
    file_hashes.push(FileHashEntry {
        path: path.to_string(),
        hash,
    });
    Ok(())
}

/// Builds integrity metadata from file hashes.
fn build_integrity(
    file_hashes: &[FileHashEntry],
    algorithm: HashAlgorithm,
) -> Result<RunpackIntegrity, RunpackError> {
    let root_hash = hash_canonical_json(algorithm, file_hashes)
        .map_err(|err| RunpackError::Hash(err.to_string()))?;
    Ok(RunpackIntegrity {
        file_hashes: file_hashes.to_vec(),
        root_hash,
    })
}

/// Verifies decision log structure and uniqueness.
fn verify_decisions<R: ArtifactReader>(reader: &R) -> Result<(), String> {
    let bytes = reader
        .read_with_limit(DECISION_LOG_PATH, MAX_RUNPACK_ARTIFACT_BYTES)
        .map_err(|err| format!("decision log read failed: {err}"))?;
    let decisions: Vec<DecisionRecord> =
        serde_json::from_slice(&bytes).map_err(|err| format!("invalid decision log: {err}"))?;

    let mut seen_trigger_ids = Vec::new();
    for decision in &decisions {
        if seen_trigger_ids.iter().any(|id: &String| id == decision.trigger_id.as_str()) {
            return Err(format!("duplicate decision for trigger {}", decision.trigger_id));
        }
        seen_trigger_ids.push(decision.trigger_id.as_str().to_string());
        match &decision.outcome {
            DecisionOutcome::Start {
                ..
            }
            | DecisionOutcome::Advance {
                ..
            }
            | DecisionOutcome::Hold {
                ..
            }
            | DecisionOutcome::Fail {
                ..
            }
            | DecisionOutcome::Complete {
                ..
            } => {}
        }
    }

    Ok(())
}

/// Validates evidence anchors in the runpack against the policy.
fn verify_anchor_policy<R: ArtifactReader>(
    reader: &R,
    _manifest: &RunpackManifest,
    anchor_policy: &EvidenceAnchorPolicy,
) -> Result<Vec<String>, String> {
    if anchor_policy.providers.is_empty() {
        return Ok(Vec::new());
    }

    let spec_bytes = reader
        .read_with_limit(SCENARIO_SPEC_PATH, MAX_RUNPACK_ARTIFACT_BYTES)
        .map_err(|err| format!("scenario spec read failed: {err}"))?;
    let spec: ScenarioSpec = serde_json::from_slice(&spec_bytes)
        .map_err(|err| format!("invalid scenario spec: {err}"))?;

    let condition_map = condition_provider_map(&spec);
    let eval_bytes = reader
        .read_with_limit(GATE_EVAL_LOG_PATH, MAX_RUNPACK_ARTIFACT_BYTES)
        .map_err(|err| format!("gate eval log read failed: {err}"))?;
    let gate_evals: Vec<GateEvalRecord> = serde_json::from_slice(&eval_bytes)
        .map_err(|err| format!("invalid gate eval log: {err}"))?;

    let mut errors = Vec::new();
    for record in gate_evals {
        for evidence in &record.evidence {
            if evidence.result.error.is_some() {
                continue;
            }
            let Some(provider_id) = condition_map.get(&evidence.condition_id) else {
                errors.push(format!(
                    "condition {} missing from scenario spec (trigger {}, stage {})",
                    evidence.condition_id, record.trigger_id, record.stage_id
                ));
                continue;
            };
            let Some(requirement) = anchor_policy.requirement_for(provider_id) else {
                continue;
            };
            if let Err(message) = validate_anchor_requirement(requirement, &evidence.result) {
                errors.push(format!(
                    "anchor invalid for condition {} (trigger {}, stage {}): {}",
                    evidence.condition_id, record.trigger_id, record.stage_id, message
                ));
            }
        }
    }

    Ok(errors)
}

/// Builds a condition-to-provider lookup from a scenario spec.
fn condition_provider_map(spec: &ScenarioSpec) -> BTreeMap<ConditionId, ProviderId> {
    let mut map = BTreeMap::new();
    for condition in &spec.conditions {
        map.insert(condition.condition_id.clone(), condition.query.provider_id.clone());
    }
    map
}

/// Ensures a single evidence result satisfies an anchor requirement.
fn validate_anchor_requirement(
    requirement: &AnchorRequirement,
    result: &EvidenceResult,
) -> Result<(), String> {
    let anchor =
        result.evidence_anchor.as_ref().ok_or_else(|| "missing evidence_anchor".to_string())?;
    if anchor.anchor_type != requirement.anchor_type {
        return Err(format!(
            "anchor_type mismatch: expected {} got {}",
            requirement.anchor_type, anchor.anchor_type
        ));
    }
    let value: Value = serde_json::from_str(&anchor.anchor_value)
        .map_err(|_| "anchor_value must be canonical JSON".to_string())?;
    let object =
        value.as_object().ok_or_else(|| "anchor_value must be a JSON object".to_string())?;
    for field in &requirement.required_fields {
        match object.get(field) {
            Some(Value::String(_) | Value::Number(_)) => {}
            Some(Value::Bool(_)) => {
                return Err(format!("anchor field {field} must be string or number"));
            }
            Some(Value::Null) => {
                return Err(format!("anchor field {field} must be set"));
            }
            Some(Value::Array(_) | Value::Object(_)) => {
                return Err(format!("anchor field {field} must be scalar"));
            }
            None => return Err(format!("anchor field {field} missing")),
        }
    }
    Ok(())
}
