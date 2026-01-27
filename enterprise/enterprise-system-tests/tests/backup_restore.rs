//! Enterprise backup/restore system tests.
// enterprise-system-tests/tests/backup_restore.rs
// ============================================================================
// Module: Backup/Restore Validation Tests
// Description: Validate Postgres + S3 backup/restore fidelity.
// Purpose: Ensure restored data passes hash validation and integrity checks.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::sync::Arc;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunStateStore;
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
use decision_gate_store_enterprise::runpack_store::RunpackKey;
use decision_gate_store_enterprise::runpack_store::RunpackStore;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStore;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStoreConfig;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::infra::PostgresFixture;
use helpers::infra::S3Fixture;
use helpers::infra::build_postgres_store_blocking;
use helpers::infra::io_error;
use helpers::infra::wait_for_postgres;
use helpers::infra::with_postgres_clients;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Backup/restore test exercises multiple systems.")]
async fn enterprise_backup_restore_validation() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_backup_restore_validation")?;

    let postgres = PostgresFixture::start()?;
    wait_for_postgres(&postgres.url).await?;
    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let token = "backup-token".to_string();
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
                max_connections: 8,
                connect_timeout_ms: 5_000,
                statement_timeout_ms: 30_000,
            }),
        },
        runpacks: EnterpriseRunpackConfig {
            s3: Some(S3RunpackStoreConfig {
                bucket: s3.bucket.clone(),
                region: Some(s3.region.clone()),
                prefix: Some("primary".to_string()),
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

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: principal_id.clone(),
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
        enterprise_config.clone(),
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("backup", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", serde_json::to_value(&define_request)?).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: fixture.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(0),
        issue_entry_packets: false,
    };
    let _: serde_json::Value =
        client.call_tool("scenario_start", serde_json::to_value(&start_request)?).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: fixture.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            run_id: fixture.run_id.clone(),
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(1),
            source_id: "backup".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let _: serde_json::Value =
        client.call_tool("scenario_trigger", serde_json::to_value(&trigger_request)?).await?;

    let schema_record = DataShapeRecord {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("backup-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("backup schema".to_string()),
        created_at: Timestamp::Logical(2),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: schema_record.clone(),
    };
    let _: serde_json::Value =
        client.call_tool("schemas_register", serde_json::to_value(&register_request)?).await?;

    let export_request = RunpackExportRequest {
        scenario_id: fixture.scenario_id.clone(),
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: fixture.run_id.clone(),
        output_dir: None,
        manifest_name: None,
        generated_at: Timestamp::Logical(2),
        include_verification: false,
    };
    let export_response: serde_json::Value =
        client.call_tool("runpack_export", serde_json::to_value(&export_request)?).await?;
    let storage_uri = export_response
        .get("storage_uri")
        .and_then(|value| value.as_str())
        .ok_or("missing storage_uri for backup")?
        .to_string();

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    server.shutdown().await;

    let restore_postgres = PostgresFixture::start()?;
    wait_for_postgres(&restore_postgres.url).await?;
    let restore_config = postgres_config(&restore_postgres.url);
    tokio::task::spawn_blocking(move || {
        let _ = build_postgres_store_blocking(restore_config)?;
        Ok::<(), std::io::Error>(())
    })
    .await
    .map_err(io_error)??;
    with_postgres_clients(&postgres.url, &restore_postgres.url, copy_postgres_tables).await?;

    let restore_bucket = "decision-gate-restore".to_string();
    let s3_client = s3.client().await?;
    if let Err(err) = s3_client.create_bucket().bucket(&restore_bucket).send().await {
        let message = format!("{err}");
        if !message.contains("BucketAlready") {
            return Err(format!("restore bucket creation failed: {message}").into());
        }
    }
    let (bucket_name, object_key) = parse_s3_uri(&storage_uri)?;
    s3_client
        .copy_object()
        .bucket(&restore_bucket)
        .key(&object_key)
        .copy_source(format!("{bucket_name}/{object_key}"))
        .send()
        .await?;
    let restored_meta = s3_client
        .head_object()
        .bucket(&restore_bucket)
        .key(&object_key)
        .send()
        .await?
        .metadata()
        .cloned()
        .unwrap_or_default();
    if !restored_meta.contains_key("sha256") {
        return Err("restored runpack missing sha256 metadata".into());
    }

    let restore_config = postgres_config(&restore_postgres.url);
    let tenant_id = fixture.tenant_id.clone();
    let namespace_id = fixture.namespace_id.clone();
    let run_id = fixture.run_id.clone();
    let schema_tenant_id = schema_record.tenant_id.clone();
    let schema_namespace_id = schema_record.namespace_id.clone();
    let schema_id = schema_record.schema_id.clone();
    let schema_version = schema_record.version.clone();
    tokio::task::spawn_blocking(move || {
        let restore_store = build_postgres_store_blocking(restore_config)?;
        let restored = restore_store
            .load(&tenant_id, &namespace_id, &run_id)
            .map_err(io_error)?
            .ok_or_else(|| io_error("missing restored run state"))?;
        if restored.run_id != run_id {
            return Err(io_error("restored run state mismatch"));
        }

        let restored_schema = restore_store
            .get(&schema_tenant_id, &schema_namespace_id, &schema_id, &schema_version)
            .map_err(io_error)?
            .ok_or_else(|| io_error("missing restored schema"))?;
        if restored_schema.schema_id != schema_id {
            return Err(io_error("restored schema mismatch"));
        }
        Ok::<(), std::io::Error>(())
    })
    .await
    .map_err(io_error)??;

    let runpack_bucket = restore_bucket.clone();
    let runpack_region = s3.region.clone();
    let runpack_endpoint = s3.endpoint.clone();
    let runpack_force_path = s3.force_path_style;
    let runpack_run_id = fixture.run_id.clone();
    let runpack_check = std::thread::spawn(move || {
        let runpack_store = S3RunpackStore::new(S3RunpackStoreConfig {
            bucket: runpack_bucket,
            region: Some(runpack_region),
            prefix: Some("primary".to_string()),
            endpoint: Some(runpack_endpoint),
            force_path_style: runpack_force_path,
            server_side_encryption: None,
            kms_key_id: None,
            max_archive_bytes: None,
        })
        .map_err(io_error)?;
        let dest_dir = tempfile::TempDir::new().map_err(io_error)?;
        let runpack_key = RunpackKey {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            run_id: runpack_run_id,
        };
        runpack_store.get_dir(&runpack_key, dest_dir.path()).map_err(io_error)?;
        let manifest_path = dest_dir.path().join("manifest.json");
        if !manifest_path.exists() {
            return Err(io_error("restored runpack missing manifest"));
        }
        Ok::<(), std::io::Error>(())
    });
    runpack_check.join().map_err(|_| io_error("runpack worker panicked"))??;

    reporter.finish(
        "pass",
        vec!["backup/restore validation succeeded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

fn postgres_config(url: &str) -> PostgresStoreConfig {
    PostgresStoreConfig {
        connection: url.to_string(),
        max_connections: 8,
        connect_timeout_ms: 5_000,
        statement_timeout_ms: 30_000,
    }
}

fn copy_postgres_tables(
    source: &mut postgres::Client,
    dest: &mut postgres::Client,
) -> Result<(), std::io::Error> {
    let run_rows = source
        .query("SELECT tenant_id, namespace_id, run_id, latest_version FROM runs", &[])
        .map_err(io_error)?;
    for row in run_rows {
        let tenant_id: String = row.get(0);
        let namespace_id: String = row.get(1);
        let run_id: String = row.get(2);
        let latest_version: i64 = row.get(3);
        dest.execute(
            "INSERT INTO runs (tenant_id, namespace_id, run_id, latest_version) VALUES ($1, $2, \
             $3, $4)",
            &[&tenant_id, &namespace_id, &run_id, &latest_version],
        )
        .map_err(io_error)?;
    }

    let state_rows = source
        .query(
            "SELECT tenant_id, namespace_id, run_id, version, state_json, state_hash, \
             hash_algorithm, saved_at FROM run_state_versions",
            &[],
        )
        .map_err(io_error)?;
    for row in state_rows {
        let tenant_id: String = row.get(0);
        let namespace_id: String = row.get(1);
        let run_id: String = row.get(2);
        let version: i64 = row.get(3);
        let state_json: String = row.get(4);
        let state_hash: String = row.get(5);
        let hash_algorithm: String = row.get(6);
        let saved_at: i64 = row.get(7);
        dest.execute(
            "INSERT INTO run_state_versions (tenant_id, namespace_id, run_id, version, \
             state_json, state_hash, hash_algorithm, saved_at) VALUES ($1, $2, $3, $4, $5, $6, \
             $7, $8)",
            &[
                &tenant_id,
                &namespace_id,
                &run_id,
                &version,
                &state_json,
                &state_hash,
                &hash_algorithm,
                &saved_at,
            ],
        )
        .map_err(io_error)?;
    }

    let schema_rows = source
        .query(
            "SELECT tenant_id, namespace_id, schema_id, version, schema_json, schema_hash, \
             hash_algorithm, description, created_at_json, signing_key_id, signing_signature, \
             signing_algorithm FROM data_shapes",
            &[],
        )
        .map_err(io_error)?;
    for row in schema_rows {
        let tenant_id: String = row.get(0);
        let namespace_id: String = row.get(1);
        let schema_id: String = row.get(2);
        let version: String = row.get(3);
        let schema_json: String = row.get(4);
        let schema_hash: String = row.get(5);
        let hash_algorithm: String = row.get(6);
        let description: Option<String> = row.get(7);
        let created_at_json: String = row.get(8);
        let signing_key_id: Option<String> = row.get(9);
        let signing_signature: Option<String> = row.get(10);
        let signing_algorithm: Option<String> = row.get(11);
        dest.execute(
            "INSERT INTO data_shapes (tenant_id, namespace_id, schema_id, version, schema_json, \
             schema_hash, hash_algorithm, description, created_at_json, signing_key_id, \
             signing_signature, signing_algorithm) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
             $10, $11, $12)",
            &[
                &tenant_id,
                &namespace_id,
                &schema_id,
                &version,
                &schema_json,
                &schema_hash,
                &hash_algorithm,
                &description,
                &created_at_json,
                &signing_key_id,
                &signing_signature,
                &signing_algorithm,
            ],
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn parse_s3_uri(uri: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let trimmed = uri.strip_prefix("s3://").ok_or("invalid s3 uri")?;
    let mut parts = trimmed.splitn(2, '/');
    let bucket = parts.next().ok_or("missing bucket")?.to_string();
    let key = parts.next().ok_or("missing key")?.to_string();
    Ok((bucket, key))
}

fn set_s3_env(s3: &S3Fixture) {
    helpers::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
    helpers::env::set_var("AWS_ACCESS_KEY_ID", &s3.access_key);
    helpers::env::set_var("AWS_SECRET_ACCESS_KEY", &s3.secret_key);
    helpers::env::set_var("AWS_REGION", &s3.region);
}

fn token_principal(token: &str) -> String {
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    format!("token:{}", digest.value)
}
