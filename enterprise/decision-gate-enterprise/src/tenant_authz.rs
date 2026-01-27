// enterprise/decision-gate-enterprise/src/tenant_authz.rs
// ============================================================================
// Module: Enterprise Tenant Authorization
// Description: Principal-to-tenant/namespace authorization mapping.
// Purpose: Enforce explicit tenant/namespace scoping for managed deployments.
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_mcp::AuthContext;
use decision_gate_mcp::TenantAccessRequest;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::TenantAuthzDecision;
use serde::Deserialize;
use serde::Serialize;

/// Namespace scope selection for a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceScope {
    /// Allow all namespaces for the tenant.
    All,
    /// Allow only the listed namespaces.
    AllowList(BTreeSet<String>),
}

/// Tenant scope entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantScope {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace scope policy.
    pub namespaces: NamespaceScope,
}

/// Principal authorization scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipalScope {
    /// Principal identifier string (subject or token fingerprint).
    pub principal_id: String,
    /// Tenant scopes allowed for this principal.
    pub tenants: Vec<TenantScope>,
}

/// Tenant authorization policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAuthzPolicy {
    /// Principal scopes.
    pub principals: Vec<PrincipalScope>,
    /// Require tenant and namespace on every request.
    #[serde(default = "default_require_tenant")]
    pub require_tenant: bool,
}

const fn default_require_tenant() -> bool {
    true
}

impl Default for TenantAuthzPolicy {
    fn default() -> Self {
        Self {
            principals: Vec::new(),
            require_tenant: default_require_tenant(),
        }
    }
}

/// Tenant authorizer using a static principal scope map.
pub struct MappedTenantAuthorizer {
    /// Policy configuration driving authorization decisions.
    policy: TenantAuthzPolicy,
    /// Index of principal id to permitted tenant scopes.
    index: BTreeMap<String, Vec<TenantScope>>,
}

impl MappedTenantAuthorizer {
    /// Builds a mapped authorizer from policy.
    #[must_use]
    pub fn new(policy: TenantAuthzPolicy) -> Self {
        let mut index = BTreeMap::new();
        for principal in &policy.principals {
            index.insert(principal.principal_id.clone(), principal.tenants.clone());
        }
        Self {
            policy,
            index,
        }
    }

    /// Returns true when a scope matches the tenant and namespace.
    fn matches_scope(
        scope: &TenantScope,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
    ) -> bool {
        if scope.tenant_id.as_str() != tenant_id.as_str() {
            return false;
        }
        match &scope.namespaces {
            NamespaceScope::All => true,
            NamespaceScope::AllowList(namespaces) => namespaces.contains(namespace_id.as_str()),
        }
    }
}

impl TenantAuthorizer for MappedTenantAuthorizer {
    fn authorize(
        &self,
        auth: &AuthContext,
        request: TenantAccessRequest<'_>,
    ) -> TenantAuthzDecision {
        let Some(tenant_id) = request.tenant_id else {
            if self.policy.require_tenant {
                return TenantAuthzDecision {
                    allowed: false,
                    reason: "missing_tenant_id".to_string(),
                };
            }
            return TenantAuthzDecision {
                allowed: true,
                reason: "tenant_optional".to_string(),
            };
        };
        let Some(namespace_id) = request.namespace_id else {
            if self.policy.require_tenant {
                return TenantAuthzDecision {
                    allowed: false,
                    reason: "missing_namespace_id".to_string(),
                };
            }
            return TenantAuthzDecision {
                allowed: true,
                reason: "namespace_optional".to_string(),
            };
        };
        let principal_id = auth.principal_id();
        let Some(scopes) = self.index.get(&principal_id) else {
            return TenantAuthzDecision {
                allowed: false,
                reason: "principal_unmapped".to_string(),
            };
        };
        let allowed =
            scopes.iter().any(|scope| Self::matches_scope(scope, tenant_id, namespace_id));
        if allowed {
            TenantAuthzDecision {
                allowed: true,
                reason: "tenant_scope_allowed".to_string(),
            }
        } else {
            TenantAuthzDecision {
                allowed: false,
                reason: "tenant_scope_denied".to_string(),
            }
        }
    }
}
