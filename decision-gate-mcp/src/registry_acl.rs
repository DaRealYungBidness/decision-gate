// decision-gate-mcp/src/registry_acl.rs
// ============================================================================
// Module: Schema Registry ACL
// Description: Role-based access control for schema registry operations.
// Purpose: Enforce per-tenant/namespace ACL with explicit auditability.
// Dependencies: decision-gate-core, decision-gate-mcp config/auth
// ============================================================================

//! ## Overview
//! Registry ACL enforcement for schema registry operations, mapping authenticated
//! principals to scoped roles and policy classes.
//!
//! Security posture: Registry ACL decisions gate schema disclosure and writes;
//! see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;

use crate::auth::AuthContext;
use crate::auth::AuthMethod;
use crate::config::PrincipalConfig;
use crate::config::PrincipalRoleConfig;
use crate::config::RegistryAclAction;
use crate::config::RegistryAclConfig;
use crate::config::RegistryAclDefault;
use crate::config::RegistryAclEffect;
use crate::config::RegistryAclMode;
use crate::config::RegistryAclRule;
use crate::config::ServerAuthConfig;

// ============================================================================
// SECTION: Role Constants
// ============================================================================

/// Role name: tenant-wide admin.
const ROLE_TENANT_ADMIN: &str = "TenantAdmin";
/// Role name: namespace owner.
const ROLE_NAMESPACE_OWNER: &str = "NamespaceOwner";
/// Role name: namespace admin.
const ROLE_NAMESPACE_ADMIN: &str = "NamespaceAdmin";
/// Role name: namespace writer.
const ROLE_NAMESPACE_WRITER: &str = "NamespaceWriter";
/// Role name: namespace reader.
const ROLE_NAMESPACE_READER: &str = "NamespaceReader";
/// Role name: schema manager.
const ROLE_SCHEMA_MANAGER: &str = "SchemaManager";

/// Registry principal resolved from auth context.
///
/// # Invariants
/// - `principal_id` is a stable identifier for ACL decisions.
#[derive(Debug, Clone)]
pub struct RegistryPrincipal {
    /// Principal identifier string.
    pub principal_id: String,
    /// Roles assigned to the principal.
    pub roles: Vec<PrincipalRole>,
    /// Optional policy class label.
    pub policy_class: Option<String>,
    /// Authentication method.
    pub auth_method: AuthMethod,
}

/// Role binding with optional tenant/namespace scope.
///
/// # Invariants
/// - A role without scope applies tenant-wide.
#[derive(Debug, Clone)]
pub struct PrincipalRole {
    /// Role name.
    pub name: String,
    /// Optional tenant scope.
    pub tenant_id: Option<TenantId>,
    /// Optional namespace scope.
    pub namespace_id: Option<NamespaceId>,
}

/// Principal profile for mapping auth subjects to roles/policy class.
#[derive(Debug, Clone)]
struct PrincipalProfile {
    /// Role bindings resolved for this principal.
    roles: Vec<PrincipalRole>,
    /// Optional policy class label tied to the principal.
    policy_class: Option<String>,
}

/// Resolves principals from auth context using configured mappings.
///
/// # Invariants
/// - Resolver mappings are immutable after construction.
#[derive(Debug, Clone, Default)]
pub struct PrincipalResolver {
    /// Principal profiles keyed by subject identifier.
    profiles: BTreeMap<String, PrincipalProfile>,
}

impl PrincipalResolver {
    /// Builds a resolver from optional server auth configuration.
    #[must_use]
    pub fn from_config(auth: Option<&ServerAuthConfig>) -> Self {
        let mut profiles = BTreeMap::new();
        if let Some(auth) = auth {
            for principal in &auth.principals {
                profiles
                    .insert(principal.subject.clone(), PrincipalProfile::from_config(principal));
            }
        }
        Self {
            profiles,
        }
    }

    /// Resolves the principal for a request.
    #[must_use]
    pub fn resolve(&self, auth: &AuthContext) -> RegistryPrincipal {
        let principal_id = auth.principal_id();
        let profile = self.profiles.get(&principal_id);
        RegistryPrincipal {
            principal_id,
            roles: profile.map_or_else(Vec::new, |p| p.roles.clone()),
            policy_class: profile.and_then(|p| p.policy_class.clone()),
            auth_method: auth.method,
        }
    }
}

impl PrincipalProfile {
    /// Builds a principal profile from config.
    fn from_config(config: &PrincipalConfig) -> Self {
        let roles = config.roles.iter().map(PrincipalRole::from_config).collect();
        Self {
            roles,
            policy_class: config.policy_class.clone(),
        }
    }
}

impl PrincipalRole {
    /// Builds a role binding from config.
    fn from_config(config: &PrincipalRoleConfig) -> Self {
        Self {
            name: config.name.clone(),
            tenant_id: config.tenant_id,
            namespace_id: config.namespace_id,
        }
    }
}

/// Registry ACL decision outcome.
///
/// # Invariants
/// - `allowed` is the authoritative decision for the request.
#[derive(Debug, Clone)]
pub struct RegistryAclDecision {
    /// Whether access is allowed.
    pub allowed: bool,
    /// Reason string for audit logs.
    pub reason: String,
}

/// Registry ACL evaluator.
///
/// # Invariants
/// - Behavior is fully determined by the stored configuration.
#[derive(Debug, Clone)]
pub struct RegistryAcl {
    /// Selected ACL evaluation mode.
    mode: RegistryAclMode,
    /// Default effect when no rules match.
    default_effect: RegistryAclDefault,
    /// Custom rules evaluated when configured.
    rules: Vec<RegistryAclRule>,
    /// Whether schema signing metadata is required.
    require_signing: bool,
    /// Allow local-only subjects when using built-in ACL.
    allow_local_only: bool,
}

impl RegistryAcl {
    /// Builds a registry ACL evaluator from config.
    #[must_use]
    pub fn new(config: &RegistryAclConfig) -> Self {
        Self {
            mode: config.mode,
            default_effect: config.default,
            rules: config.rules.clone(),
            require_signing: config.require_signing,
            allow_local_only: config.allow_local_only,
        }
    }

    /// Returns whether schema signing metadata is required.
    #[must_use]
    pub const fn require_signing(&self) -> bool {
        self.require_signing
    }

    /// Evaluates access for a registry action.
    #[must_use]
    pub fn authorize(
        &self,
        principal: &RegistryPrincipal,
        action: RegistryAclAction,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
    ) -> RegistryAclDecision {
        match self.mode {
            RegistryAclMode::Builtin => builtin_decision(
                principal,
                action,
                *tenant_id,
                *namespace_id,
                self.allow_local_only,
            ),
            RegistryAclMode::Custom => custom_decision(
                principal,
                action,
                *tenant_id,
                *namespace_id,
                &self.rules,
                self.default_effect,
            ),
        }
    }
}

/// Evaluates the built-in ACL decision for the given request.
fn builtin_decision(
    principal: &RegistryPrincipal,
    action: RegistryAclAction,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    allow_local_only: bool,
) -> RegistryAclDecision {
    if allow_local_only && principal.auth_method == AuthMethod::Local {
        return RegistryAclDecision {
            allowed: true,
            reason: "builtin_allow_local_only".to_string(),
        };
    }
    let policy_class = principal.policy_class.as_deref().unwrap_or("prod").to_ascii_lowercase();
    let is_prod = policy_class == "prod";

    let allow_read = has_any_role(
        principal,
        tenant_id,
        namespace_id,
        &[
            ROLE_TENANT_ADMIN,
            ROLE_NAMESPACE_OWNER,
            ROLE_NAMESPACE_ADMIN,
            ROLE_NAMESPACE_WRITER,
            ROLE_NAMESPACE_READER,
            ROLE_SCHEMA_MANAGER,
        ],
    );

    let allow_write = has_any_role(
        principal,
        tenant_id,
        namespace_id,
        &[ROLE_TENANT_ADMIN, ROLE_NAMESPACE_OWNER, ROLE_NAMESPACE_ADMIN],
    ) || (!is_prod
        && has_any_role(principal, tenant_id, namespace_id, &[ROLE_SCHEMA_MANAGER]));

    match action {
        RegistryAclAction::List | RegistryAclAction::Get => {
            if allow_read {
                RegistryAclDecision {
                    allowed: true,
                    reason: "builtin_allow_read".to_string(),
                }
            } else {
                RegistryAclDecision {
                    allowed: false,
                    reason: "builtin_deny_read".to_string(),
                }
            }
        }
        RegistryAclAction::Register => {
            if allow_write {
                RegistryAclDecision {
                    allowed: true,
                    reason: "builtin_allow_write".to_string(),
                }
            } else {
                RegistryAclDecision {
                    allowed: false,
                    reason: "builtin_deny_write".to_string(),
                }
            }
        }
    }
}

/// Evaluates the custom ACL decision for the given request.
fn custom_decision(
    principal: &RegistryPrincipal,
    action: RegistryAclAction,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    rules: &[RegistryAclRule],
    default_effect: RegistryAclDefault,
) -> RegistryAclDecision {
    for rule in rules {
        if !rule.actions.is_empty() && !rule.actions.contains(&action) {
            continue;
        }
        if !rule.tenants.is_empty() && !rule.tenants.contains(&tenant_id) {
            continue;
        }
        if !rule.namespaces.is_empty() && !rule.namespaces.contains(&namespace_id) {
            continue;
        }
        if !rule.subjects.is_empty() && !rule.subjects.iter().any(|s| s == &principal.principal_id)
        {
            continue;
        }
        if !rule.roles.is_empty()
            && !rule
                .roles
                .iter()
                .any(|role| principal_has_role(principal, tenant_id, namespace_id, role))
        {
            continue;
        }
        if !rule.policy_classes.is_empty()
            && !rule
                .policy_classes
                .iter()
                .any(|pc| principal.policy_class.as_deref() == Some(pc.as_str()))
        {
            continue;
        }

        return match rule.effect {
            RegistryAclEffect::Allow => RegistryAclDecision {
                allowed: true,
                reason: "custom_allow".to_string(),
            },
            RegistryAclEffect::Deny => RegistryAclDecision {
                allowed: false,
                reason: "custom_deny".to_string(),
            },
        };
    }

    match default_effect {
        RegistryAclDefault::Allow => RegistryAclDecision {
            allowed: true,
            reason: "default_allow".to_string(),
        },
        RegistryAclDefault::Deny => RegistryAclDecision {
            allowed: false,
            reason: "default_deny".to_string(),
        },
    }
}

/// Returns true when the principal has the requested role within scope.
fn principal_has_role(
    principal: &RegistryPrincipal,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    role_name: &str,
) -> bool {
    principal.roles.iter().any(|role| {
        role.name == role_name
            && role.tenant_id.as_ref().is_none_or(|tenant| *tenant == tenant_id)
            && role.namespace_id.as_ref().is_none_or(|namespace| *namespace == namespace_id)
    })
}

/// Returns true when the principal has any of the requested roles in scope.
fn has_any_role(
    principal: &RegistryPrincipal,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    roles: &[&str],
) -> bool {
    roles.iter().any(|role| principal_has_role(principal, tenant_id, namespace_id, role))
}
