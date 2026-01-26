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

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod audit;
pub mod auth;
pub mod capabilities;
pub mod config;
pub mod evidence;
pub mod policy;
pub mod runpack;
pub mod server;
pub mod telemetry;
pub mod tools;
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
pub use runpack::FileArtifactReader;
pub use runpack::FileArtifactSink;
pub use server::McpServer;
pub use telemetry::MCP_LATENCY_BUCKETS_MS;
pub use telemetry::McpMethod;
pub use telemetry::McpMetricEvent;
pub use telemetry::McpMetrics;
pub use telemetry::McpOutcome;
pub use telemetry::NoopMetrics;
pub use tools::ToolRouter;

#[cfg(test)]
mod tests;
