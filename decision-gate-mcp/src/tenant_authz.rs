// decision-gate-mcp/src/tenant_authz.rs
// ============================================================================
// Module: Tenant Authorization
// Description: Tenant/namespace authorization hooks for MCP tool calls.
// Purpose: Provide a pluggable, fail-closed tenant authz seam for enterprise.
// Dependencies: decision-gate-core, decision-gate-contract
// ============================================================================

//! Tenant authorization hooks for MCP tool calls.

use decision_gate_contract::ToolName;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;

use crate::auth::AuthContext;

/// Tenant authorization action for audit labeling.
#[derive(Debug, Clone, Copy)]
pub enum TenantAuthzAction<'a> {
    /// Tool call action.
    ToolCall(&'a ToolName),
}

/// Tenant authorization request context.
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
#[derive(Debug, Clone)]
pub struct TenantAuthzDecision {
    /// Whether access is allowed.
    pub allowed: bool,
    /// Reason label for audit logs.
    pub reason: String,
}

/// Tenant authorization interface.
pub trait TenantAuthorizer: Send + Sync {
    /// Authorize tenant/namespace access for the given request.
    fn authorize(
        &self,
        auth: &AuthContext,
        request: TenantAccessRequest<'_>,
    ) -> TenantAuthzDecision;
}

/// No-op tenant authorizer that always allows.
pub struct NoopTenantAuthorizer;

impl TenantAuthorizer for NoopTenantAuthorizer {
    fn authorize(
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
