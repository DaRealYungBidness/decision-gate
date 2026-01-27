//! Enterprise usage metering system tests.
// enterprise-system-tests/tests/usage.rs
// ============================================================================
// Module: Usage Metering Tests
// Description: Validate usage metering, quotas, idempotency, and rate limiting.
// Purpose: Ensure billing-grade enforcement and fail-closed behavior.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::EvidenceContext;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::core::hashing::HashAlgorithm;
use decision_gate_core::core::hashing::hash_bytes;
use decision_gate_core::runtime::ScenarioStatus;
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
use decision_gate_enterprise::usage::QuotaLimit;
use decision_gate_enterprise::usage::QuotaPolicy;
use decision_gate_enterprise::usage::QuotaScope;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::UsageMetric;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::RateLimitConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Usage matrix test exercises multiple tools.")]
async fn usage_metering_tool_call_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("usage_metering_tool_call_matrix")?;

    let temp_dir = TempDir::new()?;
    let ledger_path = temp_dir.path().join("usage.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let token = "usage-token".to_string();
    let principal = token_principal(&token);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: principal.clone(),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(TenantId::new("tenant-1")),
                namespace_id: Some(NamespaceId::new("default")),
            }],
        }],
    });

    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Sqlite,
                sqlite_path: Some(ledger_path.clone()),
            },
            ..EnterpriseUsageConfig::default()
        },
        source_modified_at: None,
    };

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal.clone(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("usage-matrix", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    fixture.tenant_id = TenantId::new("tenant-1");

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", serde_json::to_value(&define_request)?).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    client
        .call_tool_typed::<decision_gate_core::RunState>(
            "scenario_start",
            serde_json::to_value(&start_request)?,
        )
        .await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "usage".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    client
        .call_tool_typed::<decision_gate_core::runtime::TriggerResult>(
            "scenario_trigger",
            serde_json::to_value(&trigger_request)?,
        )
        .await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: decision_gate_core::runtime::StatusRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    client
        .call_tool_typed::<ScenarioStatus>(
            "scenario_status",
            serde_json::to_value(&status_request)?,
        )
        .await?;

    let record = DataShapeRecord {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: DataShapeId::new("usage-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("usage schema".to_string()),
        created_at: Timestamp::Logical(4),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    client
        .call_tool_typed::<serde_json::Value>(
            "schemas_register",
            serde_json::to_value(&register_request)?,
        )
        .await?;

    let list_request = SchemasListRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        cursor: None,
        limit: None,
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
            "schemas_list",
            serde_json::to_value(&list_request)?,
        )
        .await?;

    let get_request = SchemasGetRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: record.schema_id.clone(),
        version: record.version.clone(),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::SchemasGetResponse>(
            "schemas_get",
            serde_json::to_value(&get_request)?,
        )
        .await?;

    let evidence_request = EvidenceQueryRequest {
        query: decision_gate_core::EvidenceQuery {
            provider_id: decision_gate_core::ProviderId::new("time"),
            predicate: "after".to_string(),
            params: Some(json!({"timestamp": 0})),
        },
        context: EvidenceContext {
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            run_id: fixture.run_id.clone(),
            scenario_id: define_output.scenario_id.clone(),
            stage_id: fixture.stage_id.clone(),
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            trigger_time: Timestamp::Logical(5),
            correlation_id: None,
        },
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::EvidenceQueryResponse>(
            "evidence_query",
            serde_json::to_value(&evidence_request)?,
        )
        .await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    std::fs::create_dir_all(&runpack_dir)?;
    let runpack_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(6),
        include_verification: false,
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::RunpackExportResponse>(
            "runpack_export",
            serde_json::to_value(&runpack_request)?,
        )
        .await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        scenario_id: Some(define_output.scenario_id.clone()),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"after": true}),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::PrecheckToolResponse>(
            "precheck",
            serde_json::to_value(&precheck_request)?,
        )
        .await?;

    let counts = usage_counts(&ledger_path)?;
    for metric in [
        "tool_calls",
        "runs_started",
        "evidence_queries",
        "runpack_exports",
        "schemas_written",
        "registry_entries",
        "storage_bytes",
    ] {
        if !counts.contains_key(metric) {
            return Err(format!("missing usage metric {metric}").into());
        }
    }
    let schema_bytes = serde_json::to_vec(&record.schema)?.len() as u64;
    let storage_units = counts.get("storage_bytes").copied().unwrap_or_default();
    if storage_units < schema_bytes {
        return Err("storage_bytes usage did not record schema size".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["usage metrics recorded for tool matrix".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_idempotency_request_id() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("usage_idempotency_request_id")?;
    let temp_dir = TempDir::new()?;
    let ledger_path = temp_dir.path().join("usage.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let token = "idempotent-token".to_string();
    let principal = token_principal(&token);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: principal.clone(),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(TenantId::new("tenant-1")),
                namespace_id: Some(NamespaceId::new("default")),
            }],
        }],
    });

    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Sqlite,
                sqlite_path: Some(ledger_path.clone()),
            },
            ..EnterpriseUsageConfig::default()
        },
        source_modified_at: None,
    };

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal.clone(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let id = 42;
    client.call_tool_with_id("schemas_list", serde_json::to_value(&list_request)?, id).await?;
    client.call_tool_with_id("schemas_list", serde_json::to_value(&list_request)?, id).await?;

    let counts = usage_counts(&ledger_path)?;
    let tool_calls = counts.get("tool_calls").copied().unwrap_or_default();
    if tool_calls != 1 {
        return Err(format!("expected 1 tool_calls entry, got {tool_calls}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["usage idempotency enforced via request_id".to_string()],
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
async fn usage_quota_deny_before_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("usage_quota_deny_before_mutation")?;
    let temp_dir = TempDir::new()?;
    let ledger_path = temp_dir.path().join("usage.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let token = "quota-token".to_string();
    let principal = token_principal(&token);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: principal.clone(),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(TenantId::new("tenant-1")),
                namespace_id: Some(NamespaceId::new("default")),
            }],
        }],
    });

    let quota_policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::SchemasWritten,
            max_units: 0,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };

    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Sqlite,
                sqlite_path: Some(ledger_path.clone()),
            },
            quotas: quota_policy,
        },
        source_modified_at: None,
    };

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal.clone(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let record = DataShapeRecord {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("quota-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("quota schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let request = SchemasRegisterRequest {
        record,
    };
    let Err(err) = client.call_tool("schemas_register", serde_json::to_value(&request)?).await
    else {
        return Err("expected quota denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let list: decision_gate_mcp::tools::SchemasListResponse =
        client.call_tool_typed("schemas_list", serde_json::to_value(&list_request)?).await?;
    if !list.items.is_empty() {
        return Err("schema registry mutated despite quota denial".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["quota denial prevented mutations".to_string()],
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
#[allow(clippy::too_many_lines, reason = "Quota scope test uses two server configurations.")]
async fn usage_quota_scope_tenant_namespace() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("usage_quota_scope_tenant_namespace")?;

    // Tenant-scoped quota: limit total tool calls across namespaces.
    {
        let temp_dir = TempDir::new()?;
        let ledger_path = temp_dir.path().join("usage.sqlite");
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        let token = "scope-tenant".to_string();
        let principal = token_principal(&token);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec![token.clone()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![PrincipalConfig {
                subject: principal.clone(),
                policy_class: Some("prod".to_string()),
                roles: vec![PrincipalRoleConfig {
                    name: "TenantAdmin".to_string(),
                    tenant_id: Some(TenantId::new("tenant-1")),
                    namespace_id: None,
                }],
            }],
        });
        let quota_policy = QuotaPolicy {
            limits: vec![QuotaLimit {
                metric: UsageMetric::ToolCall,
                max_units: 2,
                window_ms: 60_000,
                scope: QuotaScope::Tenant,
            }],
        };
        let enterprise_config = EnterpriseConfig {
            storage: EnterpriseStorageConfig::default(),
            runpacks: EnterpriseRunpackConfig::default(),
            usage: EnterpriseUsageConfig {
                ledger: UsageLedgerConfig {
                    ledger_type: UsageLedgerType::Sqlite,
                    sqlite_path: Some(ledger_path.clone()),
                },
                quotas: quota_policy,
            },
            source_modified_at: None,
        };
        let tenant_policy = TenantAuthzPolicy {
            principals: vec![PrincipalScope {
                principal_id: principal.clone(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                }],
            }],
            require_tenant: true,
        };
        let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));
        let server = spawn_enterprise_server_from_configs(
            config,
            enterprise_config,
            tenant_authorizer,
            Arc::new(McpNoopAuditSink),
        )
        .await?;
        let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
        wait_for_server_ready(&client, Duration::from_secs(5)).await?;
        for namespace in ["default", "other"] {
            let list_request = SchemasListRequest {
                tenant_id: TenantId::new("tenant-1"),
                namespace_id: NamespaceId::new(namespace),
                cursor: None,
                limit: None,
            };
            client
                .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                    "schemas_list",
                    serde_json::to_value(&list_request)?,
                )
                .await
                .map_err(|err| format!("tenant-scope list {namespace} failed: {err}"))?;
        }
        let list_request = SchemasListRequest {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            cursor: None,
            limit: None,
        };
        let Err(err) = client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await
        else {
            return Err("expected tenant-scope quota denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        server.shutdown().await;
    }

    // Namespace-scoped quota: limit tool calls per namespace.
    {
        let temp_dir = TempDir::new()?;
        let ledger_path = temp_dir.path().join("usage.sqlite");
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        let token = "scope-namespace".to_string();
        let principal = token_principal(&token);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec![token.clone()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![PrincipalConfig {
                subject: principal.clone(),
                policy_class: Some("prod".to_string()),
                roles: vec![PrincipalRoleConfig {
                    name: "TenantAdmin".to_string(),
                    tenant_id: Some(TenantId::new("tenant-1")),
                    namespace_id: None,
                }],
            }],
        });
        let quota_policy = QuotaPolicy {
            limits: vec![QuotaLimit {
                metric: UsageMetric::ToolCall,
                max_units: 1,
                window_ms: 60_000,
                scope: QuotaScope::Namespace,
            }],
        };
        let enterprise_config = EnterpriseConfig {
            storage: EnterpriseStorageConfig::default(),
            runpacks: EnterpriseRunpackConfig::default(),
            usage: EnterpriseUsageConfig {
                ledger: UsageLedgerConfig {
                    ledger_type: UsageLedgerType::Sqlite,
                    sqlite_path: Some(ledger_path.clone()),
                },
                quotas: quota_policy,
            },
            source_modified_at: None,
        };
        let tenant_policy = TenantAuthzPolicy {
            principals: vec![PrincipalScope {
                principal_id: principal.clone(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                }],
            }],
            require_tenant: true,
        };
        let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));
        let server = spawn_enterprise_server_from_configs(
            config,
            enterprise_config,
            tenant_authorizer,
            Arc::new(McpNoopAuditSink),
        )
        .await?;
        let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
        wait_for_server_ready(&client, Duration::from_secs(5)).await?;

        let list_request = SchemasListRequest {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            cursor: None,
            limit: None,
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&list_request)?,
            )
            .await
            .map_err(|err| format!("namespace-scope list failed: {err}"))?;
        let list_request_other = SchemasListRequest {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("other"),
            cursor: None,
            limit: None,
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&list_request_other)?,
            )
            .await
            .map_err(|err| format!("namespace-scope list other failed: {err}"))?;
        let Err(err) = client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await
        else {
            return Err("expected namespace-scope quota denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        server.shutdown().await;
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["quota scope behavior validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Rate limit test covers token and tenant scopes.")]
async fn usage_rate_limit_tenant_token() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("usage_rate_limit_tenant_token")?;

    // Token-level rate limit.
    {
        let temp_dir = TempDir::new()?;
        let ledger_path = temp_dir.path().join("usage.sqlite");
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.server.limits.rate_limit = Some(RateLimitConfig {
            max_requests: 2,
            window_ms: 5_000,
            max_entries: 100,
        });
        let warmup_token = "rate-warmup".to_string();
        let token = "rate-token".to_string();
        let principal = token_principal(&token);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec![warmup_token.clone(), token.clone()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![PrincipalConfig {
                subject: principal.clone(),
                policy_class: Some("prod".to_string()),
                roles: vec![PrincipalRoleConfig {
                    name: "TenantAdmin".to_string(),
                    tenant_id: Some(TenantId::new("tenant-1")),
                    namespace_id: Some(NamespaceId::new("default")),
                }],
            }],
        });
        let enterprise_config = EnterpriseConfig {
            storage: EnterpriseStorageConfig::default(),
            runpacks: EnterpriseRunpackConfig::default(),
            usage: EnterpriseUsageConfig {
                ledger: UsageLedgerConfig {
                    ledger_type: UsageLedgerType::Sqlite,
                    sqlite_path: Some(ledger_path.clone()),
                },
                ..EnterpriseUsageConfig::default()
            },
            source_modified_at: None,
        };
        let tenant_policy = TenantAuthzPolicy {
            principals: vec![PrincipalScope {
                principal_id: principal.clone(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                }],
            }],
            require_tenant: true,
        };
        let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));
        let server = spawn_enterprise_server_from_configs(
            config,
            enterprise_config,
            tenant_authorizer,
            Arc::new(McpNoopAuditSink),
        )
        .await?;
        let warmup_client =
            server.client(Duration::from_secs(5))?.with_bearer_token(warmup_token.clone());
        wait_for_server_ready(&warmup_client, Duration::from_secs(5)).await?;
        let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
        let list_request = SchemasListRequest {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            cursor: None,
            limit: None,
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&list_request)?,
            )
            .await?;
        client
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&list_request)?,
            )
            .await?;
        let Err(err) = client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await
        else {
            return Err("expected rate limit denial".into());
        };
        if !err.contains("rate limit") {
            return Err(format!("expected rate limit error, got {err}").into());
        }
        server.shutdown().await;
    }

    // Tenant-level quota across tokens.
    {
        let temp_dir = TempDir::new()?;
        let ledger_path = temp_dir.path().join("usage.sqlite");
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        let token_a = "tenant-a".to_string();
        let token_b = "tenant-b".to_string();
        let principal_a = token_principal(&token_a);
        let principal_b = token_principal(&token_b);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec![token_a.clone(), token_b.clone()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![
                PrincipalConfig {
                    subject: principal_a.clone(),
                    policy_class: Some("prod".to_string()),
                    roles: vec![PrincipalRoleConfig {
                        name: "TenantAdmin".to_string(),
                        tenant_id: Some(TenantId::new("tenant-1")),
                        namespace_id: Some(NamespaceId::new("default")),
                    }],
                },
                PrincipalConfig {
                    subject: principal_b.clone(),
                    policy_class: Some("prod".to_string()),
                    roles: vec![PrincipalRoleConfig {
                        name: "TenantAdmin".to_string(),
                        tenant_id: Some(TenantId::new("tenant-1")),
                        namespace_id: Some(NamespaceId::new("default")),
                    }],
                },
            ],
        });
        let quota_policy = QuotaPolicy {
            limits: vec![QuotaLimit {
                metric: UsageMetric::ToolCall,
                max_units: 2,
                window_ms: 60_000,
                scope: QuotaScope::Tenant,
            }],
        };
        let enterprise_config = EnterpriseConfig {
            storage: EnterpriseStorageConfig::default(),
            runpacks: EnterpriseRunpackConfig::default(),
            usage: EnterpriseUsageConfig {
                ledger: UsageLedgerConfig {
                    ledger_type: UsageLedgerType::Sqlite,
                    sqlite_path: Some(ledger_path.clone()),
                },
                quotas: quota_policy,
            },
            source_modified_at: None,
        };
        let tenant_policy = TenantAuthzPolicy {
            principals: vec![
                PrincipalScope {
                    principal_id: principal_a.clone(),
                    tenants: vec![TenantScope {
                        tenant_id: TenantId::new("tenant-1"),
                        namespaces: NamespaceScope::All,
                    }],
                },
                PrincipalScope {
                    principal_id: principal_b.clone(),
                    tenants: vec![TenantScope {
                        tenant_id: TenantId::new("tenant-1"),
                        namespaces: NamespaceScope::All,
                    }],
                },
            ],
            require_tenant: true,
        };
        let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));
        let server = spawn_enterprise_server_from_configs(
            config,
            enterprise_config,
            tenant_authorizer,
            Arc::new(McpNoopAuditSink),
        )
        .await?;
        let client_a = server.client(Duration::from_secs(5))?.with_bearer_token(token_a.clone());
        let client_b = server.client(Duration::from_secs(5))?.with_bearer_token(token_b.clone());
        wait_for_server_ready(&client_a, Duration::from_secs(5)).await?;
        let list_request = SchemasListRequest {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            cursor: None,
            limit: None,
        };
        client_a
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&list_request)?,
            )
            .await?;
        client_b
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&list_request)?,
            )
            .await?;
        let Err(err) =
            client_b.call_tool("schemas_list", serde_json::to_value(&list_request)?).await
        else {
            return Err("expected tenant quota denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        server.shutdown().await;
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["rate limit and tenant quotas enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn quota_evasion_concurrency() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("quota_evasion_concurrency")?;
    let temp_dir = TempDir::new()?;
    let ledger_path = temp_dir.path().join("usage.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let token = "concurrency-token".to_string();
    let principal = token_principal(&token);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: principal.clone(),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(TenantId::new("tenant-1")),
                namespace_id: Some(NamespaceId::new("default")),
            }],
        }],
    });
    let quota_policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 1,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Sqlite,
                sqlite_path: Some(ledger_path.clone()),
            },
            quotas: quota_policy,
        },
        source_modified_at: None,
    };
    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal.clone(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));
    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let id = 99;
    let call_a = client.call_tool_with_id("schemas_list", serde_json::to_value(&list_request)?, id);
    let call_b = client.call_tool_with_id("schemas_list", serde_json::to_value(&list_request)?, id);
    let (result_a, result_b) = tokio::join!(call_a, call_b);
    if result_a.is_err() && result_b.is_err() {
        return Err("expected at least one concurrent call to succeed".into());
    }

    let counts = usage_counts(&ledger_path)?;
    let tool_calls = counts.get("tool_calls").copied().unwrap_or_default();
    if tool_calls > 1 {
        return Err(format!("expected at most 1 tool_calls entry, got {tool_calls}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["concurrent quota evasion prevented".to_string()],
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
async fn usage_ledger_corruption() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("usage_ledger_corruption")?;
    let temp_dir = TempDir::new()?;
    let ledger_path = temp_dir.path().join("usage.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let token = "ledger-token".to_string();
    let principal = token_principal(&token);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: principal.clone(),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(TenantId::new("tenant-1")),
                namespace_id: Some(NamespaceId::new("default")),
            }],
        }],
    });

    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Sqlite,
                sqlite_path: Some(ledger_path.clone()),
            },
            quotas: QuotaPolicy {
                limits: vec![QuotaLimit {
                    metric: UsageMetric::ToolCall,
                    max_units: 100,
                    window_ms: 60_000,
                    scope: QuotaScope::Tenant,
                }],
            },
        },
        source_modified_at: None,
    };

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal.clone(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let conn = Connection::open(&ledger_path)?;
    conn.execute("DROP TABLE usage_events", [])?;

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let Err(err) = client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await
    else {
        return Err("expected usage ledger corruption denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["usage ledger corruption fails closed".to_string()],
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

fn usage_counts(path: &Path) -> Result<BTreeMap<String, u64>, Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare("SELECT metric, SUM(units) FROM usage_events GROUP BY metric")?;
    let mut map = BTreeMap::new();
    let rows = stmt.query_map([], |row| {
        let metric: String = row.get(0)?;
        let total: i64 = row.get(1)?;
        Ok((metric, total))
    })?;
    for row in rows {
        let (metric, total) = row?;
        map.insert(metric, u64::try_from(total).unwrap_or(u64::MAX));
    }
    Ok(map)
}
