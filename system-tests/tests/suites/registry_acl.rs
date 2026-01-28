// system-tests/tests/suites/registry_acl.rs
// ============================================================================
// Module: Registry ACL Tests
// Description: End-to-end registry ACL and signing enforcement coverage.
// Purpose: Validate builtin ACL matrix, principal mapping, and signing rules.
// Dependencies: system-tests helpers
// ============================================================================

//! Registry ACL system tests.


use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeSignature;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::core::hashing::HashAlgorithm;
use decision_gate_core::core::hashing::hash_bytes;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::SchemaRegistryType;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasGetResponse;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasListResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;
use helpers::stdio_client::StdioMcpClient;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "ACL matrix coverage is a full end-to-end check.")]
async fn registry_acl_builtin_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("registry_acl_builtin_matrix")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let tenant_id = TenantId::from_raw(1).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");

    let mut cases = vec![
        RoleCase::new("tenant_admin", vec!["TenantAdmin"], "prod", true, true),
        RoleCase::new("namespace_owner", vec!["NamespaceOwner"], "prod", true, true),
        RoleCase::new("namespace_admin", vec!["NamespaceAdmin"], "prod", true, true),
        RoleCase::new("namespace_writer", vec!["NamespaceWriter"], "prod", true, false),
        RoleCase::new("namespace_reader", vec!["NamespaceReader"], "prod", true, false),
        RoleCase::new("schema_manager_prod", vec!["SchemaManager"], "prod", true, false),
        RoleCase::new("schema_manager_project", vec!["SchemaManager"], "project", true, true),
    ];

    let mut bearer_tokens = Vec::new();
    let mut principals = Vec::new();
    for case in &mut cases {
        let token = format!("token-{}", case.label);
        bearer_tokens.push(token.clone());
        principals.push(PrincipalConfig {
            subject: token_subject(&token),
            policy_class: Some(case.policy_class.to_string()),
            roles: case
                .roles
                .iter()
                .map(|name| PrincipalRoleConfig {
                    name: name.to_string(),
                    tenant_id: Some(tenant_id.clone()),
                    namespace_id: Some(namespace_id.clone()),
                })
                .collect(),
        });
        case.token = token;
    }
    let unmapped_token = "token-unmapped".to_string();
    bearer_tokens.push(unmapped_token.clone());

    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens,
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals,
    });

    let server = spawn_mcp_server(config).await?;
    let admin_case = cases
        .iter()
        .find(|case| case.label == "tenant_admin")
        .ok_or("missing tenant_admin case")?;

    let admin = server.client(Duration::from_secs(5))?.with_bearer_token(admin_case.token.clone());
    wait_for_server_ready(&admin, Duration::from_secs(5)).await?;

    let base_record = build_schema_record(&tenant_id, &namespace_id, "base", "v1", None);
    let register_request = SchemasRegisterRequest {
        record: base_record.clone(),
    };
    admin
        .call_tool_typed::<serde_json::Value>(
            "schemas_register",
            serde_json::to_value(&register_request)?,
        )
        .await?;

    let mut transcripts = admin.transcript();

    for case in &cases {
        let client = server.client(Duration::from_secs(5))?.with_bearer_token(case.token.clone());

        assert_registry_list(&client, &tenant_id, &namespace_id, case.expect_read).await?;
        assert_registry_get(
            &client,
            &tenant_id,
            &namespace_id,
            &base_record.schema_id,
            &base_record.version,
            case.expect_read,
        )
        .await?;

        let record = build_schema_record(
            &tenant_id,
            &namespace_id,
            &format!("schema-{}", case.label),
            "v1",
            None,
        );
        assert_registry_register(&client, record, case.expect_register).await?;

        transcripts.extend(client.transcript());
    }

    let unmapped = server.client(Duration::from_secs(5))?.with_bearer_token(unmapped_token);
    assert_registry_list(&unmapped, &tenant_id, &namespace_id, false).await?;
    assert_registry_get(
        &unmapped,
        &tenant_id,
        &namespace_id,
        &base_record.schema_id,
        &base_record.version,
        false,
    )
    .await?;
    let record = build_schema_record(&tenant_id, &namespace_id, "schema-unmapped", "v1", None);
    assert_registry_register(&unmapped, record, false).await?;
    transcripts.extend(unmapped.transcript());

    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["builtin registry ACL matrix validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Multiple auth transports are validated in one pass.")]
async fn registry_acl_principal_subject_mapping() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("registry_acl_principal_subject_mapping")?;
    let tenant_id = TenantId::from_raw(1).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let record = build_schema_record(&tenant_id, &namespace_id, "subject-map", "v1", None);
    let mut transcripts = Vec::new();

    // Stdio allowed subject.
    {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("decision-gate.toml");
        let config_contents = format!(
            "[server]\ntransport = \"stdio\"\nmode = \"strict\"\n\n[server.auth]\nmode = \
             \"local_only\"\n\n[[server.auth.principals]]\nsubject = \"stdio\"\npolicy_class = \
             \"prod\"\n\n[[server.auth.principals.roles]]\nname = \"TenantAdmin\"\ntenant_id = \
             {}\nnamespace_id = {}\n\n[namespace]\nallow_default = true\ndefault_tenants \
             = [{}]\n\n[[providers]]\nname = \"time\"\ntype = \"builtin\"\n",
            &tenant_id.to_string(),
            &namespace_id.to_string(),
            &tenant_id.to_string(),
        );
        std::fs::write(&config_path, config_contents)?;
        let stderr_path = reporter.artifacts().root().join("stdio-allowed.stderr.log");
        let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
        let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
        helpers::readiness::wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;
        let request = SchemasRegisterRequest {
            record: record.clone(),
        };
        client.call_tool("schemas_register", serde_json::to_value(&request)?).await?;
        transcripts.extend(client.transcript());
    }

    // Stdio denied subject (mapping mismatch).
    {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("decision-gate.toml");
        let config_contents = format!(
            "[server]\ntransport = \"stdio\"\nmode = \"strict\"\n\n[server.auth]\nmode = \
             \"local_only\"\n\n[[server.auth.principals]]\nsubject = \"loopback\"\npolicy_class = \
             \"prod\"\n\n[[server.auth.principals.roles]]\nname = \"TenantAdmin\"\ntenant_id = \
             {}\nnamespace_id = {}\n\n[namespace]\nallow_default = true\ndefault_tenants \
             = [{}]\n\n[[providers]]\nname = \"time\"\ntype = \"builtin\"\n",
            &tenant_id.to_string(),
            &namespace_id.to_string(),
            &tenant_id.to_string(),
        );
        std::fs::write(&config_path, config_contents)?;
        let stderr_path = reporter.artifacts().root().join("stdio-denied.stderr.log");
        let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
        let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
        helpers::readiness::wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;
        let request = SchemasRegisterRequest {
            record: record.clone(),
        };
        let Err(err) = client.call_tool("schemas_register", serde_json::to_value(&request)?).await
        else {
            return Err("expected stdio registry access denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        transcripts.extend(client.transcript());
    }

    // Loopback HTTP allowed subject.
    {
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::LocalOnly,
            bearer_tokens: Vec::new(),
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![principal(
                "loopback",
                "prod",
                &["TenantAdmin"],
                &tenant_id,
                &namespace_id,
            )],
        });
        let server = spawn_mcp_server(config).await?;
        let client = server.client(Duration::from_secs(5))?;
        wait_for_server_ready(&client, Duration::from_secs(5)).await?;
        let request = SchemasRegisterRequest {
            record: record.clone(),
        };
        client.call_tool("schemas_register", serde_json::to_value(&request)?).await?;
        transcripts.extend(client.transcript());
        server.shutdown().await;
    }

    // Loopback HTTP denied subject.
    {
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::LocalOnly,
            bearer_tokens: Vec::new(),
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![principal(
                "stdio",
                "prod",
                &["TenantAdmin"],
                &tenant_id,
                &namespace_id,
            )],
        });
        let server = spawn_mcp_server(config).await?;
        let client = server.client(Duration::from_secs(5))?;
        wait_for_server_ready(&client, Duration::from_secs(5)).await?;
        let request = SchemasRegisterRequest {
            record: record.clone(),
        };
        let Err(err) = client.call_tool("schemas_register", serde_json::to_value(&request)?).await
        else {
            return Err("expected loopback registry access denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        transcripts.extend(client.transcript());
        server.shutdown().await;
    }

    // Bearer token subject mapping.
    {
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        let allowed_token = "token-allowed".to_string();
        let denied_token = "token-denied".to_string();
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec![allowed_token.clone(), denied_token.clone()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: vec![principal(
                &token_subject(&allowed_token),
                "prod",
                &["TenantAdmin"],
                &tenant_id,
                &namespace_id,
            )],
        });
        let server = spawn_mcp_server(config).await?;
        let allowed =
            server.client(Duration::from_secs(5))?.with_bearer_token(allowed_token.clone());
        wait_for_server_ready(&allowed, Duration::from_secs(5)).await?;
        let request = SchemasRegisterRequest {
            record: record.clone(),
        };
        allowed.call_tool("schemas_register", serde_json::to_value(&request)?).await?;
        transcripts.extend(allowed.transcript());

        let denied = server.client(Duration::from_secs(5))?.with_bearer_token(denied_token);
        let Err(err) = denied.call_tool("schemas_register", serde_json::to_value(&request)?).await
        else {
            return Err("expected bearer registry access denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        transcripts.extend(denied.transcript());
        server.shutdown().await;
    }

    // mTLS subject mapping.
    {
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::Mtls,
            bearer_tokens: Vec::new(),
            mtls_subjects: vec!["CN=allowed".to_string(), "CN=denied".to_string()],
            allowed_tools: Vec::new(),
            principals: vec![principal(
                "CN=allowed",
                "prod",
                &["TenantAdmin"],
                &tenant_id,
                &namespace_id,
            )],
        });
        let server = spawn_mcp_server(config).await?;
        let allowed =
            server.client(Duration::from_secs(5))?.with_client_subject("CN=allowed".to_string());
        wait_for_server_ready(&allowed, Duration::from_secs(5)).await?;
        let request = SchemasRegisterRequest {
            record: record.clone(),
        };
        allowed.call_tool("schemas_register", serde_json::to_value(&request)?).await?;
        transcripts.extend(allowed.transcript());

        let denied =
            server.client(Duration::from_secs(5))?.with_client_subject("CN=denied".to_string());
        let Err(err) = denied.call_tool("schemas_register", serde_json::to_value(&request)?).await
        else {
            return Err("expected mTLS registry access denial".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        transcripts.extend(denied.transcript());
        server.shutdown().await;
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["principal subject mapping validated across transports".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "stdio-allowed.stderr.log".to_string(),
            "stdio-denied.stderr.log".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Signing enforcement covers memory + sqlite in one pass.")]
async fn registry_acl_signing_required_memory_and_sqlite() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reporter = TestReporter::new("registry_acl_signing_required_memory_and_sqlite")?;
    let tenant_id = TenantId::from_raw(1).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");

    let mut transcripts = Vec::new();

    let temp_dir = TempDir::new()?;
    let sqlite_path = temp_dir.path().join("registry.sqlite");

    for (label, registry_type, registry_path) in [
        ("memory", SchemaRegistryType::Memory, None),
        ("sqlite", SchemaRegistryType::Sqlite, Some(sqlite_path.as_path())),
    ] {
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.schema_registry.registry_type = registry_type;
        if let Some(path) = registry_path {
            config.schema_registry.path = Some(path.to_path_buf());
        }
        config.schema_registry.acl.require_signing = true;

        let server = spawn_mcp_server(config).await?;
        let client = server.client(Duration::from_secs(5))?;
        wait_for_server_ready(&client, Duration::from_secs(5)).await?;

        let unsigned = build_schema_record(
            &tenant_id,
            &namespace_id,
            &format!("unsigned-{label}"),
            "v1",
            None,
        );
        let request = SchemasRegisterRequest {
            record: unsigned,
        };
        let Err(err) = client.call_tool("schemas_register", serde_json::to_value(&request)?).await
        else {
            return Err("expected unsigned schema rejection".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }

        let invalid_signing = DataShapeSignature {
            key_id: "".to_string(),
            signature: "".to_string(),
            algorithm: Some("ed25519".to_string()),
        };
        let invalid = build_schema_record(
            &tenant_id,
            &namespace_id,
            &format!("invalid-{label}"),
            "v1",
            Some(invalid_signing),
        );
        let request = SchemasRegisterRequest {
            record: invalid,
        };
        let Err(err) = client.call_tool("schemas_register", serde_json::to_value(&request)?).await
        else {
            return Err("expected invalid signing rejection".into());
        };
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }

        let signing = DataShapeSignature {
            key_id: "key-1".to_string(),
            signature: "sig".to_string(),
            algorithm: Some("ed25519".to_string()),
        };
        let signed = build_schema_record(
            &tenant_id,
            &namespace_id,
            &format!("signed-{label}"),
            "v1",
            Some(signing.clone()),
        );
        let request = SchemasRegisterRequest {
            record: signed.clone(),
        };
        client.call_tool("schemas_register", serde_json::to_value(&request)?).await?;

        let list_request = SchemasListRequest {
            tenant_id: tenant_id.clone(),
            namespace_id: namespace_id.clone(),
            cursor: None,
            limit: None,
        };
        let list: SchemasListResponse =
            client.call_tool_typed("schemas_list", serde_json::to_value(&list_request)?).await?;
        let listed = list
            .items
            .iter()
            .find(|item| item.schema_id == signed.schema_id)
            .ok_or("signed schema missing from list")?;
        if listed.signing.is_none() {
            return Err("signed schema missing signing metadata".into());
        }

        let get_request = SchemasGetRequest {
            tenant_id: tenant_id.clone(),
            namespace_id: namespace_id.clone(),
            schema_id: signed.schema_id.clone(),
            version: signed.version.clone(),
        };
        let get: SchemasGetResponse =
            client.call_tool_typed("schemas_get", serde_json::to_value(&get_request)?).await?;
        if get.record.signing.is_none() {
            return Err("signed schema missing signing metadata in get".into());
        }

        transcripts.extend(client.transcript());
        server.shutdown().await;
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["schema signing required enforcement validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

struct RoleCase {
    label: &'static str,
    roles: Vec<&'static str>,
    policy_class: &'static str,
    expect_read: bool,
    expect_register: bool,
    token: String,
}

impl RoleCase {
    fn new(
        label: &'static str,
        roles: Vec<&'static str>,
        policy_class: &'static str,
        expect_read: bool,
        expect_register: bool,
    ) -> Self {
        Self {
            label,
            roles,
            policy_class,
            expect_read,
            expect_register,
            token: String::new(),
        }
    }
}

fn token_subject(token: &str) -> String {
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    format!("token:{}", digest.value)
}

fn principal(
    subject: &str,
    policy_class: &str,
    roles: &[&str],
    tenant_id: &TenantId,
    namespace_id: &NamespaceId,
) -> PrincipalConfig {
    PrincipalConfig {
        subject: subject.to_string(),
        policy_class: Some(policy_class.to_string()),
        roles: roles
            .iter()
            .map(|name| PrincipalRoleConfig {
                name: (*name).to_string(),
                tenant_id: Some(tenant_id.clone()),
                namespace_id: Some(namespace_id.clone()),
            })
            .collect(),
    }
}

fn build_schema_record(
    tenant_id: &TenantId,
    namespace_id: &NamespaceId,
    schema_id: &str,
    version: &str,
    signing: Option<DataShapeSignature>,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id: tenant_id.clone(),
        namespace_id: namespace_id.clone(),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("registry acl schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing,
    }
}

async fn assert_registry_list(
    client: &McpHttpClient,
    tenant_id: &TenantId,
    namespace_id: &NamespaceId,
    should_allow: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = SchemasListRequest {
        tenant_id: tenant_id.clone(),
        namespace_id: namespace_id.clone(),
        cursor: None,
        limit: None,
    };
    let result = client
        .call_tool_typed::<SchemasListResponse>("schemas_list", serde_json::to_value(&request)?)
        .await;
    if should_allow {
        result.map(|_| ()).map_err(|err| err.into())
    } else {
        let err = result.err().ok_or("expected registry list denial")?;
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        Ok(())
    }
}

async fn assert_registry_get(
    client: &McpHttpClient,
    tenant_id: &TenantId,
    namespace_id: &NamespaceId,
    schema_id: &DataShapeId,
    version: &DataShapeVersion,
    should_allow: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = SchemasGetRequest {
        tenant_id: tenant_id.clone(),
        namespace_id: namespace_id.clone(),
        schema_id: schema_id.clone(),
        version: version.clone(),
    };
    let result = client
        .call_tool_typed::<SchemasGetResponse>("schemas_get", serde_json::to_value(&request)?)
        .await;
    if should_allow {
        result.map(|_| ()).map_err(|err| err.into())
    } else {
        let err = result.err().ok_or("expected registry get denial")?;
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        Ok(())
    }
}

async fn assert_registry_register(
    client: &McpHttpClient,
    record: DataShapeRecord,
    should_allow: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = SchemasRegisterRequest {
        record,
    };
    let result = client
        .call_tool_typed::<serde_json::Value>("schemas_register", serde_json::to_value(&request)?)
        .await;
    if should_allow {
        result.map(|_| ()).map_err(|err| err.into())
    } else {
        let err = result.err().ok_or("expected registry register denial")?;
        if !err.contains("unauthorized") {
            return Err(format!("expected unauthorized, got {err}").into());
        }
        Ok(())
    }
}
