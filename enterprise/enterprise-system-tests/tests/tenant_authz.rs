//! Enterprise tenant authz system tests.
// enterprise-system-tests/tests/tenant_authz.rs
// ============================================================================
// Module: Tenant Authorization Tests
// Description: Validate tenant authz policy enforcement and mappings.
// Purpose: Ensure principals map to tenant/namespace scopes and ACLs.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::core::hashing::HashAlgorithm;
use decision_gate_core::core::hashing::hash_bytes;
use decision_gate_enterprise::config::EnterpriseConfig;
use decision_gate_enterprise::config::EnterpriseRunpackConfig;
use decision_gate_enterprise::config::EnterpriseStorageConfig;
use decision_gate_enterprise::config::EnterpriseUsageConfig;
use decision_gate_enterprise::config::UsageLedgerConfig;
use decision_gate_enterprise::config::UsageLedgerType;
use decision_gate_enterprise::tenant_authz::MappedTenantAuthorizer;
use decision_gate_enterprise::tenant_authz::NamespaceScope;
use decision_gate_enterprise::tenant_authz::PrincipalScope;
use decision_gate_enterprise::tenant_authz::TenantAuthzPolicy;
use decision_gate_enterprise::tenant_authz::TenantScope;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Tenant authz matrix covers multiple policy paths.")]
async fn enterprise_tenant_authz_core_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_tenant_authz_core_matrix")?;

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.namespace.default_tenants.push(TenantId::new("tenant-2"));

    let api_token = "api-token".to_string();
    let multi_token = "multi-token".to_string();
    let registry_deny_token = "registry-deny".to_string();

    let api_principal = token_principal(&api_token);
    let multi_principal = token_principal(&multi_token);
    let registry_principal = token_principal(&registry_deny_token);

    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![api_token.clone(), multi_token.clone(), registry_deny_token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![
            PrincipalConfig {
                subject: api_principal.clone(),
                policy_class: Some("prod".to_string()),
                roles: vec![PrincipalRoleConfig {
                    name: "TenantAdmin".to_string(),
                    tenant_id: Some(TenantId::new("tenant-1")),
                    namespace_id: Some(NamespaceId::new("default")),
                }],
            },
            PrincipalConfig {
                subject: multi_principal.clone(),
                policy_class: Some("prod".to_string()),
                roles: vec![PrincipalRoleConfig {
                    name: "TenantAdmin".to_string(),
                    tenant_id: Some(TenantId::new("tenant-1")),
                    namespace_id: Some(NamespaceId::new("default")),
                }],
            },
        ],
    });

    let tenant_allowlist = BTreeSet::from(["default".to_string()]);
    let policy = TenantAuthzPolicy {
        principals: vec![
            PrincipalScope {
                principal_id: api_principal.clone(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::AllowList(tenant_allowlist.clone()),
                }],
            },
            PrincipalScope {
                principal_id: multi_principal.clone(),
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
            },
            PrincipalScope {
                principal_id: registry_principal.clone(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                }],
            },
        ],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(policy));
    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Memory,
                sqlite_path: None,
            },
            ..EnterpriseUsageConfig::default()
        },
        source_modified_at: None,
    };

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let api_client = server.client(Duration::from_secs(5))?.with_bearer_token(api_token.clone());
    wait_for_server_ready(&api_client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("missing-tenant", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let Err(err) =
        api_client.call_tool("scenario_define", serde_json::to_value(&define_request)?).await
    else {
        return Err("expected missing tenant_id denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let mut allowed_fixture = ScenarioFixture::time_after("api-tenant", "run-1", 0);
    allowed_fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: allowed_fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse = api_client
        .call_tool_typed("scenario_define", serde_json::to_value(&define_request)?)
        .await?;

    let denied_record = DataShapeRecord {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("restricted"),
        schema_id: DataShapeId::new("restricted-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("restricted schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let request = SchemasRegisterRequest {
        record: denied_record,
    };
    let Err(err) = api_client.call_tool("schemas_register", serde_json::to_value(&request)?).await
    else {
        return Err("expected namespace allowlist denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let multi_client =
        server.client(Duration::from_secs(5))?.with_bearer_token(multi_token.clone());

    let mut tenant2_fixture = ScenarioFixture::time_after("multi-tenant-2", "run-1", 0);
    tenant2_fixture.spec.default_tenant_id = Some(TenantId::new("tenant-2"));
    let define_request = ScenarioDefineRequest {
        spec: tenant2_fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse = multi_client
        .call_tool_typed("scenario_define", serde_json::to_value(&define_request)?)
        .await?;

    let mut tenant3_fixture = ScenarioFixture::time_after("multi-tenant-3", "run-2", 0);
    tenant3_fixture.spec.default_tenant_id = Some(TenantId::new("tenant-3"));
    let define_request = ScenarioDefineRequest {
        spec: tenant3_fixture.spec.clone(),
    };
    let Err(err) =
        multi_client.call_tool("scenario_define", serde_json::to_value(&define_request)?).await
    else {
        return Err("expected multi-tenant scope denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let deny_client =
        server.client(Duration::from_secs(5))?.with_bearer_token(registry_deny_token.clone());
    let allowed_record = DataShapeRecord {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("allowed-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("allowed schema".to_string()),
        created_at: Timestamp::Logical(2),
        signing: None,
    };
    let request = SchemasRegisterRequest {
        record: allowed_record,
    };
    let Err(err) = deny_client.call_tool("schemas_register", serde_json::to_value(&request)?).await
    else {
        return Err("expected registry ACL denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let mut transcripts = api_client.transcript();
    transcripts.extend(multi_client.transcript());
    transcripts.extend(deny_client.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["tenant authz core matrix validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn enterprise_tenant_authz_jwt_subject_mapping() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_tenant_authz_jwt_subject_mapping")?;

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec!["user:alice".to_string(), "user:bob".to_string()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });

    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "user:alice".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(policy));
    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Memory,
                sqlite_path: None,
            },
            ..EnterpriseUsageConfig::default()
        },
        source_modified_at: None,
    };

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let alice =
        server.client(Duration::from_secs(5))?.with_client_subject("user:alice".to_string());
    wait_for_server_ready(&alice, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("jwt-subject", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse =
        alice.call_tool_typed("scenario_define", serde_json::to_value(&define_request)?).await?;

    let bob = server.client(Duration::from_secs(5))?.with_client_subject("user:bob".to_string());
    let Err(err) = bob.call_tool("scenario_define", serde_json::to_value(&define_request)?).await
    else {
        return Err("expected unmapped JWT subject denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let mut transcripts = alice.transcript();
    transcripts.extend(bob.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["JWT subject mapping enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

fn token_principal(token: &str) -> String {
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    format!("token:{}", digest.value)
}
