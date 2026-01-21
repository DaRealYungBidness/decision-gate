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

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::core::DecisionOutcome;
use crate::core::DecisionRecord;
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
}

impl Default for RunpackBuilder {
    fn default() -> Self {
        Self {
            manifest_version: RunpackVersion("v1".to_string()),
            hash_algorithm: DEFAULT_HASH_ALGORITHM,
            verifier_mode: VerifierMode::OfflineStrict,
        }
    }
}

impl RunpackBuilder {
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

        let manifest = RunpackManifest {
            manifest_version: self.manifest_version.clone(),
            generated_at,
            scenario_id: spec.scenario_id.clone(),
            run_id: state.run_id.clone(),
            spec_hash,
            hash_algorithm: self.hash_algorithm,
            verifier_mode: self.verifier_mode,
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
            match reader.read(&entry.path) {
                Ok(bytes) => {
                    let actual = hash_bytes(self.hash_algorithm, &bytes);
                    if actual != entry.hash {
                        errors.push(format!("hash mismatch for {}", entry.path));
                    }
                    checked = checked.saturating_add(1);
                }
                Err(_) => {
                    errors.push(format!("missing artifact {}", entry.path));
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
    let bytes = reader.read(DECISION_LOG_PATH).map_err(|_| "missing decision log".to_string())?;
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
