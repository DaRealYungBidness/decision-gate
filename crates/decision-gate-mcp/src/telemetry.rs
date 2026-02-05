// crates/decision-gate-mcp/src/telemetry.rs
// ============================================================================
// Module: MCP Telemetry
// Description: Observability hooks for MCP transport and tool routing.
// Purpose: Provide metric events and latency buckets without hard deps.
// Dependencies: decision-gate-contract, decision-gate-core
// ============================================================================

//! ## Overview
//! This module exposes a thin metrics interface for MCP request counters and
//! latency histograms. It is intentionally dependency-light so downstream
//! deployments can plug in Prometheus or OpenTelemetry without redesign.
//! Security posture: telemetry must avoid leaking raw evidence or secrets and
//! treat labels as untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::time::Duration;

use decision_gate_contract::ToolName;

use crate::config::ServerTransport;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Default latency buckets in milliseconds for MCP request histograms.
pub const MCP_LATENCY_BUCKETS_MS: &[u64] =
    &[1, 2, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 30_000];

// ============================================================================
// SECTION: Metric Labels
// ============================================================================

/// MCP request method classification.
///
/// # Invariants
/// - Variants are stable for telemetry labeling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum McpMethod {
    /// JSON-RPC tools/list.
    ToolsList,
    /// JSON-RPC tools/call.
    ToolsCall,
    /// JSON-RPC resources/list.
    ResourcesList,
    /// JSON-RPC resources/read.
    ResourcesRead,
    /// Invalid or malformed JSON-RPC request.
    Invalid,
    /// Unsupported JSON-RPC method.
    Other,
}

impl McpMethod {
    /// Returns a stable label for the method.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToolsList => "tools/list",
            Self::ToolsCall => "tools/call",
            Self::ResourcesList => "resources/list",
            Self::ResourcesRead => "resources/read",
            Self::Invalid => "invalid",
            Self::Other => "other",
        }
    }
}

/// MCP request outcome classification.
///
/// # Invariants
/// - Variants are stable for telemetry labeling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum McpOutcome {
    /// Successful request.
    Ok,
    /// Failed request.
    Error,
}

impl McpOutcome {
    /// Returns a stable label for the outcome.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

/// MCP request metric event payload.
///
/// # Invariants
/// - Optional fields are `None` when the metadata is unavailable.
#[derive(Debug, Clone)]
pub struct McpMetricEvent {
    /// Transport used for the request.
    pub transport: ServerTransport,
    /// JSON-RPC method classification.
    pub method: McpMethod,
    /// Tool name when available (tools/call).
    pub tool: Option<ToolName>,
    /// Request outcome.
    pub outcome: McpOutcome,
    /// JSON-RPC error code when present.
    pub error_code: Option<i64>,
    /// Normalized error kind label.
    pub error_kind: Option<&'static str>,
    /// Unsafe client correlation identifier when available.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-issued correlation identifier when available.
    pub server_correlation_id: Option<String>,
    /// Request body size in bytes.
    pub request_bytes: usize,
    /// Response body size in bytes.
    pub response_bytes: usize,
}

// ============================================================================
// SECTION: Trait
// ============================================================================

/// Metrics sink for MCP requests and latencies.
pub trait McpMetrics: Send + Sync {
    /// Records a request counter event.
    fn record_request(&self, event: McpMetricEvent);
    /// Records a latency observation for the request.
    fn record_latency(&self, event: McpMetricEvent, latency: Duration);
}

/// No-op metrics sink.
///
/// # Invariants
/// - Metrics are intentionally discarded.
pub struct NoopMetrics;

impl McpMetrics for NoopMetrics {
    fn record_request(&self, _event: McpMetricEvent) {}

    fn record_latency(&self, _event: McpMetricEvent, _latency: Duration) {}
}
