// decision-gate-mcp/src/lib.rs
// ============================================================================
// Module: Decision Gate MCP
// Description: MCP server and evidence federation for Decision Gate.
// Purpose: Provide MCP tool adapters over the Decision Gate control plane.
// Dependencies: decision-gate-core, decision-gate-providers, axum, tokio
// ============================================================================

//! ## Overview
//! Decision Gate MCP exposes the control plane through MCP tools and federates
//! evidence queries across built-in and external MCP providers. All MCP tools
//! are thin wrappers over [`decision_gate_core::ControlPlane`].
//! Security posture: MCP is a trust boundary; all inputs are untrusted and must
//! be validated; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod audit;
pub mod auth;
pub mod capabilities;
pub mod config;
pub mod correlation;
pub mod docs;
pub mod evidence;
pub mod namespace_authority;
pub mod policy;
pub mod registry_acl;
pub mod runpack;
pub mod runpack_object_store;
pub mod runpack_storage;
pub mod server;
pub mod telemetry;
pub mod tenant_authz;
pub mod tools;
pub mod usage;
pub mod validation;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use audit::McpAuditEvent;
pub use audit::McpAuditSink;
pub use audit::McpFileAuditSink;
pub use audit::McpNoopAuditSink;
pub use audit::McpStderrAuditSink;
pub use audit::PrecheckAuditEvent;
pub use audit::TenantAuthzEvent;
pub use audit::UsageAuditEvent;
pub use auth::AuthAuditSink;
pub use auth::AuthContext;
pub use auth::DefaultToolAuthz;
pub use auth::NoopAuditSink;
pub use auth::RequestContext;
pub use auth::ToolAuthz;
pub use config::DecisionGateConfig;
pub use config::SchemaRegistryConfig;
pub use evidence::FederatedEvidenceProvider;
pub use evidence::ProviderClientConfig;
pub use namespace_authority::AssetCoreNamespaceAuthority;
pub use namespace_authority::NamespaceAuthority;
pub use namespace_authority::NamespaceAuthorityError;
pub use namespace_authority::NoopNamespaceAuthority;
pub use runpack::FileArtifactReader;
pub use runpack::FileArtifactSink;
pub use runpack_storage::RunpackStorage;
pub use runpack_storage::RunpackStorageError;
pub use runpack_storage::RunpackStorageKey;
pub use server::McpServer;
pub use server::ServerOverrides;
pub use telemetry::MCP_LATENCY_BUCKETS_MS;
pub use telemetry::McpMethod;
pub use telemetry::McpMetricEvent;
pub use telemetry::McpMetrics;
pub use telemetry::McpOutcome;
pub use telemetry::NoopMetrics;
pub use tenant_authz::NoopTenantAuthorizer;
pub use tenant_authz::TenantAccessRequest;
pub use tenant_authz::TenantAuthorizer;
pub use tenant_authz::TenantAuthzAction;
pub use tenant_authz::TenantAuthzDecision;
pub use tools::DocsProvider;
pub use tools::ToolRouter;
pub use tools::ToolVisibilityResolver;
pub use usage::NoopUsageMeter;
pub use usage::UsageCheckRequest;
pub use usage::UsageDecision;
pub use usage::UsageMeter;
pub use usage::UsageMetric;
pub use usage::UsageRecord;

#[cfg(test)]
mod tests;
