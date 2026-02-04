// decision-gate-mcp/src/usage.rs
// ============================================================================
// Module: Usage Metering + Quotas
// Description: Usage metering hooks and quota checks for MCP tool calls.
// Purpose: Provide a pluggable, fail-closed usage/quota seam for enterprise.
// Dependencies: decision-gate-core, decision-gate-contract
// ============================================================================

//! ## Overview
//! Usage metering and quota hooks for MCP tool calls.
//!
//! Security posture: usage enforcement is a control-plane guardrail and must
//! fail closed when deny decisions are returned. See
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_contract::ToolName;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use serde::Deserialize;
use serde::Serialize;

use crate::auth::AuthContext;

/// Usage metric identifiers.
///
/// # Invariants
/// - Variants are stable for usage audit labeling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageMetric {
    /// Generic tool call count.
    ToolCall,
    /// Scenario run start count.
    RunsStarted,
    /// Evidence query count.
    EvidenceQueries,
    /// Runpack export count.
    RunpackExports,
    /// Schema registration count.
    SchemasWritten,
    /// Registry entry count.
    RegistryEntries,
    /// Storage bytes (schemas/runpacks/etc.).
    StorageBytes,
}

/// Usage check request.
///
/// # Invariants
/// - This is a pure request container; values are validated by the meter.
#[derive(Debug, Clone, Copy)]
pub struct UsageCheckRequest<'a> {
    /// Tool name when available.
    pub tool: &'a ToolName,
    /// Tenant identifier when provided.
    pub tenant_id: Option<&'a TenantId>,
    /// Namespace identifier when provided.
    pub namespace_id: Option<&'a NamespaceId>,
    /// Client correlation identifier when provided.
    pub correlation_id: Option<&'a str>,
    /// Server correlation identifier when provided.
    pub server_correlation_id: Option<&'a str>,
    /// Request identifier when available.
    pub request_id: Option<&'a str>,
    /// Usage metric being evaluated.
    pub metric: UsageMetric,
    /// Units requested (count or bytes).
    pub units: u64,
}

/// Usage record emitted after successful actions.
///
/// # Invariants
/// - Records are emitted only after successful tool actions.
#[derive(Debug, Clone, Copy)]
pub struct UsageRecord<'a> {
    /// Tool name when available.
    pub tool: &'a ToolName,
    /// Tenant identifier when provided.
    pub tenant_id: Option<&'a TenantId>,
    /// Namespace identifier when provided.
    pub namespace_id: Option<&'a NamespaceId>,
    /// Client correlation identifier when provided.
    pub correlation_id: Option<&'a str>,
    /// Server correlation identifier when provided.
    pub server_correlation_id: Option<&'a str>,
    /// Request identifier when available.
    pub request_id: Option<&'a str>,
    /// Usage metric being recorded.
    pub metric: UsageMetric,
    /// Units consumed (count or bytes).
    pub units: u64,
}

/// Usage decision outcome.
///
/// # Invariants
/// - `allowed` is the authoritative decision for the request.
#[derive(Debug, Clone)]
pub struct UsageDecision {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Reason label for audit logs.
    pub reason: String,
}

/// Usage metering + quota enforcement interface.
pub trait UsageMeter: Send + Sync {
    /// Checks whether usage is allowed for the request.
    fn check(&self, auth: &AuthContext, request: UsageCheckRequest<'_>) -> UsageDecision;

    /// Records usage after a successful action.
    fn record(&self, auth: &AuthContext, record: UsageRecord<'_>);
}

/// No-op usage meter that always allows and discards usage records.
///
/// # Invariants
/// - Always returns an allow decision and emits no records.
pub struct NoopUsageMeter;

impl UsageMeter for NoopUsageMeter {
    fn check(&self, _auth: &AuthContext, _request: UsageCheckRequest<'_>) -> UsageDecision {
        UsageDecision {
            allowed: true,
            reason: "noop_allow".to_string(),
        }
    }

    fn record(&self, _auth: &AuthContext, _record: UsageRecord<'_>) {}
}
