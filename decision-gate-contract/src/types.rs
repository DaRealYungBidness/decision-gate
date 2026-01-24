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

use std::fmt;

use decision_gate_core::Comparator;
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
    pub name: ToolName,
    /// Tool description for clients.
    pub description: String,
    /// JSON schema for tool input.
    pub input_schema: Value,
}

/// Canonical tool names for Decision Gate MCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    /// Register a `ScenarioSpec` and compute its hash.
    ScenarioDefine,
    /// Start a new scenario run.
    ScenarioStart,
    /// Fetch a read-only run status snapshot.
    ScenarioStatus,
    /// Evaluate the next agent-driven step.
    ScenarioNext,
    /// Submit external artifacts for audit.
    ScenarioSubmit,
    /// Submit a trigger event and evaluate the run.
    ScenarioTrigger,
    /// Query evidence providers with disclosure policy applied.
    EvidenceQuery,
    /// Export runpack artifacts.
    RunpackExport,
    /// Verify runpack artifacts offline.
    RunpackVerify,
    /// List registered evidence providers.
    ProvidersList,
    /// List registered data shapes.
    SchemasList,
    /// Register a data shape schema.
    SchemasRegister,
    /// Fetch a data shape schema.
    SchemasGet,
    /// List registered scenarios.
    ScenariosList,
    /// Precheck a scenario with asserted data.
    Precheck,
}

impl ToolName {
    /// Returns the canonical string name for the tool.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScenarioDefine => "scenario_define",
            Self::ScenarioStart => "scenario_start",
            Self::ScenarioStatus => "scenario_status",
            Self::ScenarioNext => "scenario_next",
            Self::ScenarioSubmit => "scenario_submit",
            Self::ScenarioTrigger => "scenario_trigger",
            Self::EvidenceQuery => "evidence_query",
            Self::RunpackExport => "runpack_export",
            Self::RunpackVerify => "runpack_verify",
            Self::ProvidersList => "providers_list",
            Self::SchemasList => "schemas_list",
            Self::SchemasRegister => "schemas_register",
            Self::SchemasGet => "schemas_get",
            Self::ScenariosList => "scenarios_list",
            Self::Precheck => "precheck",
        }
    }

    /// Returns all Decision Gate tool names in canonical order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::ScenarioDefine,
            Self::ScenarioStart,
            Self::ScenarioStatus,
            Self::ScenarioNext,
            Self::ScenarioSubmit,
            Self::ScenarioTrigger,
            Self::EvidenceQuery,
            Self::RunpackExport,
            Self::RunpackVerify,
            Self::ProvidersList,
            Self::SchemasList,
            Self::SchemasRegister,
            Self::SchemasGet,
            Self::ScenariosList,
            Self::Precheck,
        ]
    }

    /// Parses a tool name from its string representation.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "scenario_define" => Some(Self::ScenarioDefine),
            "scenario_start" => Some(Self::ScenarioStart),
            "scenario_status" => Some(Self::ScenarioStatus),
            "scenario_next" => Some(Self::ScenarioNext),
            "scenario_submit" => Some(Self::ScenarioSubmit),
            "scenario_trigger" => Some(Self::ScenarioTrigger),
            "evidence_query" => Some(Self::EvidenceQuery),
            "runpack_export" => Some(Self::RunpackExport),
            "runpack_verify" => Some(Self::RunpackVerify),
            "providers_list" => Some(Self::ProvidersList),
            "schemas_list" => Some(Self::SchemasList),
            "schemas_register" => Some(Self::SchemasRegister),
            "schemas_get" => Some(Self::SchemasGet),
            "scenarios_list" => Some(Self::ScenariosList),
            "precheck" => Some(Self::Precheck),
            _ => None,
        }
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Tool contract with full request and response schemas.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TooltipsManifest {
    /// Tooltip manifest version.
    pub version: String,
    /// Tooltip entries, ordered by term.
    pub entries: Vec<TooltipEntry>,
}

/// Tooltip entry for a term used in documentation.
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

/// Determinism classification for provider predicates.
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

/// Predicate contract describing parameters and output value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateContract {
    /// Predicate name used in `EvidenceQuery`.
    pub name: String,
    /// Predicate description.
    pub description: String,
    /// Determinism classification for predicate outputs.
    pub determinism: DeterminismClass,
    /// Whether `EvidenceQuery.params` is required for this predicate.
    pub params_required: bool,
    /// JSON schema for predicate parameters.
    pub params_schema: Value,
    /// JSON schema for predicate output value.
    pub result_schema: Value,
    /// Allow-list of supported comparators for this predicate.
    pub allowed_comparators: Vec<Comparator>,
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
