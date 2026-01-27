//! Enterprise config wiring system tests.
// enterprise-system-tests/tests/config_wiring.rs
// ============================================================================
// Module: Enterprise Config Wiring Tests
// Description: Validate Postgres + S3 wiring and runpack storage URIs.
// Purpose: Ensure enterprise config wires storage backends end-to-end.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

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
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use decision_gate_store_enterprise::postgres_store::PostgresStoreConfig;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStoreConfig;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::infra::PostgresFixture;
use helpers::infra::S3Fixture;
use helpers::infra::wait_for_postgres;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Config wiring test validates multiple backends.")]
async fn enterprise_config_wiring_postgres_s3() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_config_wiring_postgres_s3")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres(&postgres.url).await?;

    let s3 = S3Fixture::start().await?;
    helpers::env::set_var("AWS_ACCESS_KEY_ID", &s3.access_key);
    helpers::env::set_var("AWS_SECRET_ACCESS_KEY", &s3.secret_key);
    helpers::env::set_var("AWS_REGION", &s3.region);

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let token = "enterprise-token".to_string();
    let principal_id = token_principal(&token);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: principal_id.clone(),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(TenantId::new("tenant-1")),
                namespace_id: Some(NamespaceId::new("default")),
            }],
        }],
    });

    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig {
            postgres: Some(PostgresStoreConfig {
                connection: postgres.url.clone(),
                ..PostgresStoreConfig::default()
            }),
        },
        runpacks: EnterpriseRunpackConfig {
            s3: Some(S3RunpackStoreConfig {
                bucket: s3.bucket.clone(),
                region: Some(s3.region.clone()),
                prefix: Some("enterprise-tests".to_string()),
                endpoint: Some(s3.endpoint.clone()),
                force_path_style: s3.force_path_style,
                server_side_encryption: None,
                kms_key_id: None,
                max_archive_bytes: None,
            }),
        },
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Memory,
                sqlite_path: None,
            },
            ..EnterpriseUsageConfig::default()
        },
        source_modified_at: None,
    };

    let policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal_id.clone(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(policy));
    let audit_sink = Arc::new(McpNoopAuditSink);

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        audit_sink,
    )
    .await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("enterprise-wiring", "run-1", 0);
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
            source_id: "enterprise".to_string(),
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

    let record = DataShapeRecord {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("enterprise-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("enterprise schema".to_string()),
        created_at: Timestamp::Logical(3),
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

    let runpack_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        run_id: fixture.run_id.clone(),
        output_dir: None,
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(4),
        include_verification: false,
    };
    let export: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", serde_json::to_value(&runpack_request)?).await?;
    let storage_uri = export.storage_uri.clone().ok_or("missing storage_uri")?;
    if !storage_uri.starts_with(&format!("s3://{}", s3.bucket)) {
        return Err("storage_uri does not reference expected bucket".into());
    }
    if !storage_uri.contains("tenant-1/default") {
        return Err("storage_uri missing tenant/namespace prefix".into());
    }

    let s3_client = s3.client().await?;
    let object_key = storage_uri
        .strip_prefix(&format!("s3://{}/", s3.bucket))
        .ok_or("invalid storage_uri format")?;
    let _ = s3_client
        .head_object()
        .bucket(s3.bucket.clone())
        .key(object_key)
        .send()
        .await
        .map_err(|err| format!("head_object failed: {err}"))?;

    let mut pg_client = postgres::Client::connect(&postgres.url, postgres::NoTls)?;
    let run_rows = pg_client.query(
        "SELECT run_id FROM run_state_versions WHERE tenant_id = $1 AND namespace_id = $2",
        &[&"tenant-1", &"default"],
    )?;
    if run_rows.is_empty() {
        return Err("expected run_state_versions rows in Postgres".into());
    }
    let schema_rows = pg_client.query(
        "SELECT schema_id FROM data_shapes WHERE tenant_id = $1 AND namespace_id = $2",
        &[&"tenant-1", &"default"],
    )?;
    if schema_rows.is_empty() {
        return Err("expected data_shapes rows in Postgres".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["enterprise config wiring validated".to_string()],
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
