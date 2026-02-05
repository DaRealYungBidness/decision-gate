// crates/decision-gate-contract/src/types.rs
// ============================================================================
// Module: Contract Types
// Description: Shared data models for Decision Gate contract artifacts.
// Purpose: Provide canonical shapes for tooling, providers, schemas, and manifests.
// Dependencies: decision-gate-core, serde, serde_json
// ============================================================================

//! ## Overview
//! This module defines the typed contract shapes that are serialized into the
//! generated artifacts under `Docs/generated/decision-gate`. These structures
//! are the canonical source for docs, SDK generation, and validation tooling.
//! Security posture: artifacts are consumed by external tooling; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::Comparator;
// ============================================================================
// SECTION: Re-Exports
// ============================================================================
/// Canonical MCP tool names for Decision Gate.
pub use decision_gate_core::ToolName;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::HashDigest;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// ============================================================================
// SECTION: Manifest Types
// ============================================================================

/// Manifest describing the generated contract artifacts.
///
/// # Invariants
/// - When produced by [`crate::ContractBuilder`], `contract_version` matches the crate version that
///   generated the artifacts.
/// - When produced by [`crate::ContractBuilder`], `artifacts` are ordered by their `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractManifest {
    /// Contract version identifier (matches the crate version).
    pub contract_version: String,
    /// Hash algorithm used for artifact digests.
    pub hash_algorithm: HashAlgorithm,
    /// Artifacts included in the bundle, ordered by path.
    pub artifacts: Vec<ManifestArtifact>,
}

/// Manifest entry describing a single artifact.
///
/// # Invariants
/// - When produced by [`crate::ContractBuilder`], `path` is a safe, relative path under the output
///   directory.
/// - When produced by [`crate::ContractBuilder`], `digest` is computed using `hash_algorithm` from
///   the associated [`ContractManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestArtifact {
    /// Relative artifact path under the output directory.
    pub path: String,
    /// Artifact content type.
    pub content_type: String,
    /// Content digest for the artifact payload.
    pub digest: HashDigest,
}

// ============================================================================
// SECTION: Bundle Types
// ============================================================================

/// Generated contract bundle with artifacts and manifest metadata.
///
/// # Invariants
/// - When produced by [`crate::ContractBuilder`], `manifest` is derived from `artifacts` and
///   matches their digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractBundle {
    /// Manifest describing the artifacts.
    pub manifest: ContractManifest,
    /// Artifact payloads included in the bundle.
    pub artifacts: Vec<ContractArtifact>,
}

/// Artifact payload with content bytes.
///
/// # Invariants
/// - When produced by [`crate::ContractBuilder`], `path` is a safe, relative path under the output
///   directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractArtifact {
    /// Relative artifact path under the output directory.
    pub path: String,
    /// MIME content type for the artifact.
    pub content_type: String,
    /// Serialized artifact payload bytes.
    pub bytes: Vec<u8>,
}

// ============================================================================
// SECTION: Tooling Contracts
// ============================================================================

/// Tool definition used by MCP tool listing.
///
/// # Invariants
/// - `name` is a stable MCP tool identifier.
/// - `input_schema` is a JSON Schema payload for the tool input shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// MCP tool name.
    pub name: ToolName,
    /// Tool description for clients.
    pub description: String,
    /// JSON schema for tool input.
    pub input_schema: Value,
}

/// Tool contract with full request and response schemas.
///
/// # Invariants
/// - `input_schema` and `output_schema` are JSON Schema payloads.
/// - `examples` are expected to validate against the schemas when emitted by the contract
///   generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolContract {
    /// Tool name.
    pub name: ToolName,
    /// Tool description.
    pub description: String,
    /// JSON schema for tool input payload.
    pub input_schema: Value,
    /// JSON schema for tool response payload.
    pub output_schema: Value,
    /// Example payloads for documentation and SDKs.
    pub examples: Vec<ToolExample>,
    /// Notes describing tool usage and security considerations.
    pub notes: Vec<String>,
}

/// Tool example with input/output payloads.
///
/// # Invariants
/// - `input` and `output` align with the tool schemas when generated by the contract generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExample {
    /// Short example description.
    pub description: String,
    /// Example input payload.
    pub input: Value,
    /// Example output payload.
    pub output: Value,
}

// ============================================================================
// SECTION: Tooltip Catalog
// ============================================================================

/// Tooltip manifest used to annotate documentation code blocks.
///
/// # Invariants
/// - `entries` are ordered by term when emitted by the contract generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TooltipsManifest {
    /// Tooltip manifest version.
    pub version: String,
    /// Tooltip entries, ordered by term.
    pub entries: Vec<TooltipEntry>,
}

/// Tooltip entry for a term used in documentation.
///
/// # Invariants
/// - `term` is a stable, ASCII token used in docs and UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TooltipEntry {
    /// Term to highlight in code blocks.
    pub term: String,
    /// Tooltip title label.
    pub title: String,
    /// Tooltip body description.
    pub description: String,
}

// ============================================================================
// SECTION: Provider Contracts
// ============================================================================

/// Provider contract describing capabilities and check schemas.
///
/// # Invariants
/// - `provider_id` matches the provider identifier used in [`decision_gate_core::EvidenceQuery`].
/// - `checks` are ordered deterministically when emitted by the contract generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderContract {
    /// Provider identifier used in [`decision_gate_core::EvidenceQuery`].
    pub provider_id: String,
    /// Provider display name.
    pub name: String,
    /// Provider description.
    pub description: String,
    /// Provider transport kind ("builtin" or "mcp").
    pub transport: String,
    /// Provider-level configuration schema.
    pub config_schema: Value,
    /// Supported checks exposed by the provider.
    pub checks: Vec<CheckContract>,
    /// Notes describing provider behavior and determinism.
    pub notes: Vec<String>,
}

/// Determinism classification for provider checks.
///
/// # Invariants
/// - Serialized as `snake_case` for contract stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismClass {
    /// Outputs are fully determined by inputs and internal state.
    Deterministic,
    /// Outputs depend on caller-supplied time or trigger context.
    TimeDependent,
    /// Outputs depend on external systems or mutable environments.
    External,
}

impl DeterminismClass {
    /// Returns a stable string label for documentation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::TimeDependent => "time_dependent",
            Self::External => "external",
        }
    }
}

/// Check contract describing parameters and output value.
///
/// # Invariants
/// - `check_id` matches the check identifier used in [`decision_gate_core::EvidenceQuery`].
/// - `allowed_comparators` are in canonical order when emitted by the contract generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckContract {
    /// Check identifier used in [`decision_gate_core::EvidenceQuery`].
    pub check_id: String,
    /// Check description.
    pub description: String,
    /// Determinism classification for check outputs.
    pub determinism: DeterminismClass,
    /// Whether [`decision_gate_core::EvidenceQuery::params`] is required for this check.
    pub params_required: bool,
    /// JSON schema for check parameters.
    pub params_schema: Value,
    /// JSON schema for check output value.
    pub result_schema: Value,
    /// Allow-list of supported comparators for this check.
    pub allowed_comparators: Vec<Comparator>,
    /// Evidence anchor types emitted by this check.
    pub anchor_types: Vec<String>,
    /// Content types returned for populated evidence values.
    pub content_types: Vec<String>,
    /// Example check invocations.
    pub examples: Vec<CheckExample>,
}

/// Check example with parameters and expected output shape.
///
/// # Invariants
/// - `params` and `result` align with the check schemas when emitted by the contract generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckExample {
    /// Short example description.
    pub description: String,
    /// Example params payload.
    pub params: Value,
    /// Example output value.
    pub result: Value,
}
