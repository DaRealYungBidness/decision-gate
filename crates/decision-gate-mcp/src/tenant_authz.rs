// crates/decision-gate-mcp/src/tenant_authz.rs
// ============================================================================
// Module: Tenant Authorization
// Description: Tenant/namespace authorization hooks for MCP tool calls.
// Purpose: Provide a pluggable, fail-closed tenant authz seam for enterprise.
// Dependencies: decision-gate-core, decision-gate-contract
// ============================================================================

//! ## Overview
//! Tenant authorization hooks provide a pluggable, enterprise-grade seam for
//! enforcing tenant and namespace access checks for MCP tool calls.
//!
//! ## Layer Responsibilities
//! - Enforce tenant/namespace access for each tool call (fail closed).
//! - Surface deterministic allow/deny decisions for audit sinks.
//!
//! ## Invariants
//! - Authorization decisions must be deterministic for identical inputs.
//! - Missing tenant/namespace context must deny when required.
//! - Implementations must avoid side effects beyond audit logging.
//!
//! Security posture: tenant authorization is a trust boundary and must fail
//! closed on missing or invalid context; see `Docs/security/threat_model.md`.

use async_trait::async_trait;
use decision_gate_contract::ToolName;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;

use crate::auth::AuthContext;

/// Tenant authorization action for audit labeling.
///
/// # Invariants
/// - Variants identify the audited action only and do not imply access.
#[derive(Debug, Clone, Copy)]
pub enum TenantAuthzAction<'a> {
    /// Tool call action.
    ToolCall(&'a ToolName),
}

/// Tenant authorization request context.
///
/// # Invariants
/// - This is a pure request container; values are validated at the authz boundary.
#[derive(Debug, Clone, Copy)]
pub struct TenantAccessRequest<'a> {
    /// Action being authorized.
    pub action: TenantAuthzAction<'a>,
    /// Tenant identifier (when provided).
    pub tenant_id: Option<&'a TenantId>,
    /// Namespace identifier (when provided).
    pub namespace_id: Option<&'a NamespaceId>,
}

/// Tenant authorization decision outcome.
///
/// # Invariants
/// - `allowed` is the authoritative decision for the request.
#[derive(Debug, Clone)]
pub struct TenantAuthzDecision {
    /// Whether access is allowed.
    pub allowed: bool,
    /// Reason label for audit logs.
    pub reason: String,
}

/// Tenant authorization interface.
#[async_trait]
pub trait TenantAuthorizer: Send + Sync {
    /// Authorize tenant/namespace access for the given request.
    async fn authorize(
        &self,
        auth: &AuthContext,
        request: TenantAccessRequest<'_>,
    ) -> TenantAuthzDecision;
}

/// No-op tenant authorizer that always allows.
///
/// # Invariants
/// - Always returns an allow decision.
pub struct NoopTenantAuthorizer;

#[async_trait]
impl TenantAuthorizer for NoopTenantAuthorizer {
    async fn authorize(
        &self,
        _auth: &AuthContext,
        _request: TenantAccessRequest<'_>,
    ) -> TenantAuthzDecision {
        TenantAuthzDecision {
            allowed: true,
            reason: "noop_allow".to_string(),
        }
    }
}
