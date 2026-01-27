// enterprise/decision-gate-enterprise/tests/tenant_authz.rs
// ============================================================================
// Module: Tenant Authorization Tests
// Description: Unit tests for enterprise tenant authorization policy.
// Purpose: Validate allow/deny behavior for principal scopes.
// ============================================================================

//! Tenant authorization unit tests.

use std::collections::BTreeSet;

use decision_gate_contract::ToolName;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_enterprise::tenant_authz::MappedTenantAuthorizer;
use decision_gate_enterprise::tenant_authz::NamespaceScope;
use decision_gate_enterprise::tenant_authz::PrincipalScope;
use decision_gate_enterprise::tenant_authz::TenantAuthzPolicy;
use decision_gate_enterprise::tenant_authz::TenantScope;
use decision_gate_mcp::AuthContext;
use decision_gate_mcp::TenantAccessRequest;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::auth::AuthMethod;

fn auth_context(subject: &str) -> AuthContext {
    AuthContext {
        method: AuthMethod::Local,
        subject: Some(subject.to_string()),
        token_fingerprint: None,
    }
}

#[test]
fn tenant_authz_allows_matching_scope() {
    let mut namespaces = BTreeSet::new();
    namespaces.insert("default".to_string());
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::AllowList(namespaces),
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision.allowed);
}

#[test]
fn tenant_authz_denies_unmapped_principal() {
    let policy = TenantAuthzPolicy::default();
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("unknown"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!decision.allowed);
}

#[test]
fn tenant_authz_denies_namespace_outside_allowlist() {
    let mut namespaces = BTreeSet::new();
    namespaces.insert("default".to_string());
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "bob".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::AllowList(namespaces),
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("bob"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("other")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!decision.allowed);
}

#[test]
fn tenant_authz_allows_optional_tenant() {
    let policy = TenantAuthzPolicy {
        principals: Vec::new(),
        require_tenant: false,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("any"),
        TenantAccessRequest {
            tenant_id: None,
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision.allowed);
}

// ---------------------------------------------------------------------------
// New tests
// ---------------------------------------------------------------------------

#[test]
fn tenant_authz_namespace_scope_all_allows_any() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("anything-at-all")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision.allowed);
    assert_eq!(decision.reason, "tenant_scope_allowed");
}

#[test]
fn tenant_authz_require_tenant_missing_namespace_denies() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!decision.allowed);
    assert_eq!(decision.reason, "missing_namespace_id");
}

#[test]
fn tenant_authz_optional_tenant_missing_namespace_allows() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("t1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: false,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("t1")),
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision.allowed);
    assert_eq!(decision.reason, "namespace_optional");
}

#[test]
fn tenant_authz_principal_mapped_wrong_tenant_denies() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-2")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!decision.allowed);
    assert_eq!(decision.reason, "tenant_scope_denied");
}

#[test]
fn tenant_authz_multi_tenant_principal() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "admin".to_string(),
            tenants: vec![
                TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                },
                TenantScope {
                    tenant_id: TenantId::new("tenant-2"),
                    namespaces: NamespaceScope::All,
                },
            ],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);

    let decision_t1 = authorizer.authorize(
        &auth_context("admin"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision_t1.allowed);
    assert_eq!(decision_t1.reason, "tenant_scope_allowed");

    let decision_t2 = authorizer.authorize(
        &auth_context("admin"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-2")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision_t2.allowed);
    assert_eq!(decision_t2.reason, "tenant_scope_allowed");
}

#[test]
fn tenant_authz_empty_allowlist_denies_all() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::AllowList(BTreeSet::new()),
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let decision = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("any-namespace")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!decision.allowed);
    assert_eq!(decision.reason, "tenant_scope_denied");
}

#[test]
fn tenant_authz_reason_labels_are_exact() {
    // Path 1: missing_tenant_id (require_tenant=true, tenant=None)
    let policy_required = TenantAuthzPolicy {
        principals: Vec::new(),
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy_required);
    let d1 = authorizer.authorize(
        &auth_context("any"),
        TenantAccessRequest {
            tenant_id: None,
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!d1.allowed);
    assert_eq!(d1.reason, "missing_tenant_id");

    // Path 2: tenant_optional (require_tenant=false, tenant=None)
    let policy_optional = TenantAuthzPolicy {
        principals: Vec::new(),
        require_tenant: false,
    };
    let authorizer = MappedTenantAuthorizer::new(policy_optional);
    let d2 = authorizer.authorize(
        &auth_context("any"),
        TenantAccessRequest {
            tenant_id: None,
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(d2.allowed);
    assert_eq!(d2.reason, "tenant_optional");

    // Path 3: missing_namespace_id (require_tenant=true, tenant=Some, namespace=None)
    let policy_required_ns = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("t1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy_required_ns);
    let d3 = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("t1")),
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!d3.allowed);
    assert_eq!(d3.reason, "missing_namespace_id");

    // Path 4: namespace_optional (require_tenant=false, tenant=Some, namespace=None)
    let policy_optional_ns = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("t1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: false,
    };
    let authorizer = MappedTenantAuthorizer::new(policy_optional_ns);
    let d4 = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("t1")),
            namespace_id: None,
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(d4.allowed);
    assert_eq!(d4.reason, "namespace_optional");

    // Path 5: principal_unmapped (unknown principal)
    let policy_mapped = TenantAuthzPolicy {
        principals: Vec::new(),
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy_mapped);
    let d5 = authorizer.authorize(
        &auth_context("unknown"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("t1")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!d5.allowed);
    assert_eq!(d5.reason, "principal_unmapped");

    // Path 6: tenant_scope_allowed (matching scope)
    let mut ns = BTreeSet::new();
    ns.insert("default".to_string());
    let policy_allow = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("t1"),
                namespaces: NamespaceScope::AllowList(ns),
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy_allow);
    let d6 = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("t1")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(d6.allowed);
    assert_eq!(d6.reason, "tenant_scope_allowed");

    // Path 7: tenant_scope_denied (scope mismatch)
    let d7 = authorizer.authorize(
        &auth_context("alice"),
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("t1")),
            namespace_id: Some(&NamespaceId::new("forbidden")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(!d7.allowed);
    assert_eq!(d7.reason, "tenant_scope_denied");
}

#[test]
fn tenant_authz_token_fingerprint_principal_lookup() {
    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "token:abc123".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let authorizer = MappedTenantAuthorizer::new(policy);
    let auth = AuthContext {
        method: AuthMethod::Local,
        subject: None,
        token_fingerprint: Some("abc123".to_string()),
    };
    let decision = authorizer.authorize(
        &auth,
        TenantAccessRequest {
            tenant_id: Some(&TenantId::new("tenant-1")),
            namespace_id: Some(&NamespaceId::new("default")),
            action: decision_gate_mcp::TenantAuthzAction::ToolCall(&ToolName::RunpackExport),
        },
    );
    assert!(decision.allowed);
    assert_eq!(decision.reason, "tenant_scope_allowed");
}

#[test]
fn tenant_authz_policy_serde_roundtrip() {
    let mut namespaces = BTreeSet::new();
    namespaces.insert("default".to_string());
    let policy = TenantAuthzPolicy {
        principals: vec![
            PrincipalScope {
                principal_id: "alice".to_string(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("t1"),
                    namespaces: NamespaceScope::AllowList(namespaces),
                }],
            },
            PrincipalScope {
                principal_id: "bob".to_string(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("t2"),
                    namespaces: NamespaceScope::All,
                }],
            },
        ],
        require_tenant: true,
    };
    let json = serde_json::to_string(&policy).expect("serialize");
    let restored: TenantAuthzPolicy = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.principals.len(), policy.principals.len());
    assert_eq!(restored.require_tenant, policy.require_tenant);
}

#[test]
fn tenant_authz_default_policy_shape() {
    let policy = TenantAuthzPolicy::default();
    assert!(policy.require_tenant);
    assert!(policy.principals.is_empty());
}
