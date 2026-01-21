// decision-gate-core/src/core/runpack.rs
// ============================================================================
// Module: Decision Gate Runpack Manifest
// Description: Runpack manifest schemas and integrity metadata.
// Purpose: Provide canonical runpack indices for offline verification.
// Dependencies: crate::core::{hashing, identifiers, time}, serde
// ============================================================================

//! ## Overview
//! Runpack manifests index Decision Gate artifacts with deterministic hashes. Verifiers
//! rely on the manifest to locate control-plane logs, disclosures, and evidence
//! metadata needed for offline verification.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;

use crate::core::hashing::HashAlgorithm;
use crate::core::hashing::HashDigest;
use crate::core::identifiers::RunId;
use crate::core::identifiers::ScenarioId;
use crate::core::time::Timestamp;

// ============================================================================
// SECTION: Manifest Types
// ============================================================================

/// Runpack verification mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierMode {
    /// Verifier may only use bundled artifacts (no external fetch).
    OfflineStrict,
    /// Verifier may fetch external artifacts when references exist.
    OfflineWithFetch,
}

/// Runpack manifest version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunpackVersion(pub String);

/// Runpack manifest describing Decision Gate artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpackManifest {
    /// Manifest version identifier.
    pub manifest_version: RunpackVersion,
    /// Timestamp when the runpack was generated.
    pub generated_at: Timestamp,
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Run identifier.
    pub run_id: RunId,
    /// Hash of the scenario specification.
    pub spec_hash: HashDigest,
    /// Hash algorithm used for runpack artifacts.
    pub hash_algorithm: HashAlgorithm,
    /// Verifier mode for offline verification.
    pub verifier_mode: VerifierMode,
    /// Integrity metadata for the runpack.
    pub integrity: RunpackIntegrity,
    /// Artifact index entries.
    pub artifacts: Vec<ArtifactRecord>,
}

/// Runpack integrity metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpackIntegrity {
    /// File hash entries for runpack artifacts.
    pub file_hashes: Vec<FileHashEntry>,
    /// Root hash computed over the file hash list.
    pub root_hash: HashDigest,
}

/// Hash entry for a file or artifact within the runpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHashEntry {
    /// Runpack-relative path.
    pub path: String,
    /// Hash digest of the file contents.
    pub hash: HashDigest,
}

/// Artifact record indexed by the runpack manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    /// Artifact identifier.
    pub artifact_id: String,
    /// Artifact kind.
    pub kind: ArtifactKind,
    /// Runpack-relative path to the artifact.
    pub path: String,
    /// Content type for the artifact when applicable.
    pub content_type: Option<String>,
    /// Hash digest for the artifact content.
    pub hash: HashDigest,
    /// Indicates whether the artifact is required for verification.
    pub required: bool,
}

/// Artifact kinds included in Decision Gate runpacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Scenario specification artifact.
    ScenarioSpec,
    /// Trigger log artifact.
    TriggerLog,
    /// Gate evaluation log artifact.
    GateEvalLog,
    /// Decision log artifact.
    DecisionLog,
    /// Packet log artifact.
    PacketLog,
    /// Dispatch receipt log artifact.
    DispatchLog,
    /// Evidence record log artifact.
    EvidenceLog,
    /// Submission log artifact.
    SubmissionLog,
    /// Tool-call transcript artifact.
    ToolTranscript,
    /// Verifier output report.
    VerifierReport,
    /// Custom artifact record.
    Custom,
}
