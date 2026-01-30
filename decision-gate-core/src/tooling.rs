// decision-gate-core/src/tooling.rs
// ============================================================================
// Module: Tooling Identifiers
// Description: Canonical MCP tool identifiers for Decision Gate.
// Purpose: Shared tool naming across contracts, runtime, and config.
// Dependencies: serde
// ============================================================================

//! ## Overview
//! Canonical tool identifiers used by Decision Gate MCP.
//! These names are part of the external contract surface.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

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
    /// Fetch a provider contract by provider identifier.
    ProviderContractGet,
    /// Fetch predicate schema details for a provider.
    ProviderSchemaGet,
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
            Self::ProviderContractGet => "provider_contract_get",
            Self::ProviderSchemaGet => "provider_schema_get",
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
            Self::ProviderContractGet,
            Self::ProviderSchemaGet,
            Self::SchemasRegister,
            Self::SchemasList,
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
            "provider_contract_get" => Some(Self::ProviderContractGet),
            "provider_schema_get" => Some(Self::ProviderSchemaGet),
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
