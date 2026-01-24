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
pub mod runpack;
pub mod server;
pub mod telemetry;
pub mod tools;

#[cfg(test)]
mod tests {
    //! Test-only lint relaxations for panic-based assertions and debug output.
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
}

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use audit::McpAuditEvent;
pub use audit::McpAuditSink;
pub use audit::McpFileAuditSink;
pub use audit::McpNoopAuditSink;
pub use audit::McpStderrAuditSink;
pub use auth::AuthAuditSink;
pub use auth::AuthContext;
pub use auth::DefaultToolAuthz;
pub use auth::NoopAuditSink;
pub use auth::RequestContext;
pub use auth::ToolAuthz;
pub use config::DecisionGateConfig;
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
