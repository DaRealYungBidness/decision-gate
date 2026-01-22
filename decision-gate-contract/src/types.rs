// decision-gate-contract/src/types.rs
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

use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::HashDigest;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// ============================================================================
// SECTION: Manifest Types
// ============================================================================

/// Manifest describing the generated contract artifacts.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractBundle {
    /// Manifest describing the artifacts.
    pub manifest: ContractManifest,
    /// Artifact payloads included in the bundle.
    pub artifacts: Vec<ContractArtifact>,
}

/// Artifact payload with content bytes.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// MCP tool name.
    pub name: String,
    /// Tool description for clients.
    pub description: String,
    /// JSON schema for tool input.
    pub input_schema: Value,
}

/// Tool contract with full request and response schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolContract {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON schema for tool input payload.
    pub input_schema: Value,
    /// JSON schema for tool response payload.
    pub output_schema: Value,
    /// Notes describing tool usage and security considerations.
    pub notes: Vec<String>,
}

// ============================================================================
// SECTION: Provider Contracts
// ============================================================================

/// Provider contract describing capabilities and predicate schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderContract {
    /// Provider identifier used in `EvidenceQuery`.
    pub provider_id: String,
    /// Provider display name.
    pub name: String,
    /// Provider description.
    pub description: String,
    /// Provider transport kind ("builtin" or "mcp").
    pub transport: String,
    /// Provider-level configuration schema.
    pub config_schema: Value,
    /// Supported predicates exposed by the provider.
    pub predicates: Vec<PredicateContract>,
    /// Notes describing provider behavior and determinism.
    pub notes: Vec<String>,
}

/// Predicate contract describing parameters and output value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateContract {
    /// Predicate name used in `EvidenceQuery`.
    pub name: String,
    /// Predicate description.
    pub description: String,
    /// Whether `EvidenceQuery.params` is required for this predicate.
    pub params_required: bool,
    /// JSON schema for predicate parameters.
    pub params_schema: Value,
    /// JSON schema for predicate output value.
    pub result_schema: Value,
    /// Evidence anchor types emitted by this predicate.
    pub anchor_types: Vec<String>,
    /// Content types returned for populated evidence values.
    pub content_types: Vec<String>,
    /// Example predicate invocations.
    pub examples: Vec<PredicateExample>,
}

/// Predicate example with parameters and expected output shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateExample {
    /// Short example description.
    pub description: String,
    /// Example params payload.
    pub params: Value,
    /// Example output value.
    pub result: Value,
}
