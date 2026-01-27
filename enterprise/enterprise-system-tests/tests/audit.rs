//! Enterprise audit system tests.
// enterprise-system-tests/tests/audit.rs
// ============================================================================
// Module: Audit Chain Tests
// Description: Validate hash-chained audit logs and deny-path coverage.
// Purpose: Ensure enterprise audit logs are tamper-evident and complete.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_enterprise::audit_chain::HashChainedAuditSink;
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
use decision_gate_mcp::UsageMetric;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn audit_chain_immutability() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("audit_chain_immutability")?;
    let audit_path = reporter.artifacts().root().join("audit.jsonl");

    let (server, client) = start_audit_server(&audit_path, QuotaPolicy::default()).await?;

    let mut fixture = ScenarioFixture::time_after("audit-chain", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", serde_json::to_value(&define_request)?).await?;

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: Some(5),
    };
    let _: serde_json::Value =
        client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await?;

    let envelopes = read_audit_log(&audit_path)?;
    if envelopes.len() < 2 {
        return Err("expected multiple audit entries".into());
    }
    validate_chain(&envelopes)?;

    let original = fs::read_to_string(&audit_path)?;
    let mut tampered = original.clone();
    if let Some(line) = original.lines().next() {
        let mutated = line.replace("\"event\":\"", "\"event\":\"tampered_");
        tampered = original.replacen(line, &mutated, 1);
    }
    if tampered == original {
        return Err("failed to tamper audit log for validation".into());
    }
    fs::write(&audit_path, tampered)?;

    let tampered_envelopes = read_audit_log(&audit_path)?;
    if validate_chain(&tampered_envelopes).is_ok() {
        return Err("expected audit chain tampering detection".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["audit hash chain detects tampering".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "audit.jsonl".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn audit_event_completeness() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("audit_event_completeness")?;
    let audit_path = reporter.artifacts().root().join("audit.jsonl");

    let (server, client) = start_audit_server(&audit_path, QuotaPolicy::default()).await?;

    let mut fixture = ScenarioFixture::time_after("audit-complete", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", serde_json::to_value(&define_request)?).await?;

    let record = schema_record("tenant-1", "default", "audit-schema");
    let register_request = SchemasRegisterRequest {
        record,
    };
    let _: serde_json::Value =
        client.call_tool("schemas_register", serde_json::to_value(&register_request)?).await?;

    let envelopes = read_audit_log(&audit_path)?;
    let tenant_authz =
        find_event(&envelopes, "tenant_authz").ok_or("missing tenant_authz event")?;
    assert_nonempty(tenant_authz, "principal_id")?;
    assert_bool(tenant_authz, "allowed")?;
    assert_nonempty(tenant_authz, "reason")?;
    assert_nonempty(tenant_authz, "tenant_id")?;
    assert_nonempty(tenant_authz, "namespace_id")?;

    let registry =
        find_event(&envelopes, "registry_audit").ok_or("missing registry_audit event")?;
    assert_nonempty(registry, "tenant_id")?;
    assert_nonempty(registry, "namespace_id")?;
    assert_nonempty(registry, "action")?;
    assert_bool(registry, "allowed")?;
    assert_nonempty(registry, "reason")?;
    assert_nonempty(registry, "principal_id")?;
    assert_array(registry, "roles")?;

    let usage = find_event(&envelopes, "usage_audit").ok_or("missing usage_audit event")?;
    assert_nonempty(usage, "metric")?;
    assert_nonempty(usage, "principal_id")?;
    assert_bool(usage, "allowed")?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["audit events include required fields".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "audit.jsonl".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn audit_export_jsonl_format() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("audit_export_jsonl_format")?;
    let audit_path = reporter.artifacts().root().join("audit.jsonl");

    let (server, client) = start_audit_server(&audit_path, QuotaPolicy::default()).await?;

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: Some(1),
    };
    let _: serde_json::Value =
        client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await?;

    let envelopes = read_audit_log(&audit_path)?;
    if envelopes.is_empty() {
        return Err("expected audit JSONL entries".into());
    }
    for env in &envelopes {
        if env.prev_hash.is_empty() || env.hash.is_empty() {
            return Err("audit envelope hash fields are empty".into());
        }
        if !env.payload.is_object() {
            return Err("audit payload must be a JSON object".into());
        }
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["audit export JSONL format validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "audit.jsonl".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn audit_deny_paths_coverage() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("audit_deny_paths_coverage")?;
    let audit_path = reporter.artifacts().root().join("audit.jsonl");

    let quotas = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 0,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let (server, client) = start_audit_server(&audit_path, quotas).await?;

    let fixture = ScenarioFixture::time_after("audit-deny", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let Err(err) =
        client.call_tool("scenario_define", serde_json::to_value(&define_request)?).await
    else {
        return Err("expected missing tenant denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: Some(1),
    };
    let Err(err) = client.call_tool("schemas_list", serde_json::to_value(&list_request)?).await
    else {
        return Err("expected usage quota denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let envelopes = read_audit_log(&audit_path)?;
    let tenant_authz =
        find_event(&envelopes, "tenant_authz").ok_or("missing tenant_authz event")?;
    if tenant_authz.get("allowed").and_then(Value::as_bool).unwrap_or(true) {
        return Err("expected tenant_authz denial".into());
    }
    let usage = find_event(&envelopes, "usage_audit").ok_or("missing usage_audit event")?;
    if usage.get("allowed").and_then(Value::as_bool).unwrap_or(true) {
        return Err("expected usage denial".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["audit deny path coverage verified".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "audit.jsonl".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

async fn start_audit_server(
    audit_path: &Path,
    quotas: QuotaPolicy,
) -> Result<
    (helpers::harness::McpServerHandle, helpers::mcp_client::McpHttpClient),
    Box<dyn std::error::Error>,
> {
    if audit_path.exists() {
        fs::remove_file(audit_path)?;
    }
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let token = "audit-token".to_string();
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
    let enterprise_config = EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Memory,
                sqlite_path: None,
            },
            quotas,
        },
        source_modified_at: None,
    };

    let audit_sink = Arc::new(HashChainedAuditSink::new(audit_path)?);
    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        audit_sink,
    )
    .await?;

    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;
    Ok((server, client))
}

#[derive(Debug, Deserialize)]
struct AuditEnvelope {
    payload: Value,
    prev_hash: String,
    hash: String,
}

fn read_audit_log(path: &Path) -> Result<Vec<AuditEnvelope>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let envelope: AuditEnvelope = serde_json::from_str(line)?;
        out.push(envelope);
    }
    Ok(out)
}

fn validate_chain(envelopes: &[AuditEnvelope]) -> Result<(), Box<dyn std::error::Error>> {
    let mut prev = "0".to_string();
    for env in envelopes {
        let payload_bytes = serde_json::to_vec(&env.payload)?;
        let mut combined = prev.as_bytes().to_vec();
        combined.extend_from_slice(&payload_bytes);
        let digest = hash_bytes(HashAlgorithm::Sha256, &combined);
        if env.prev_hash != prev {
            return Err("audit chain prev_hash mismatch".into());
        }
        if env.hash != digest.value {
            return Err("audit chain hash mismatch".into());
        }
        prev = env.hash.clone();
    }
    Ok(())
}

fn find_event<'a>(envelopes: &'a [AuditEnvelope], event: &str) -> Option<&'a Value> {
    envelopes.iter().find_map(|env| {
        let payload = &env.payload;
        if payload.get("event").and_then(Value::as_str) == Some(event) {
            Some(payload)
        } else {
            None
        }
    })
}

fn assert_nonempty(payload: &Value, field: &str) -> Result<(), Box<dyn std::error::Error>> {
    let value = payload.get(field).and_then(Value::as_str).unwrap_or("");
    if value.trim().is_empty() {
        return Err(format!("missing audit field {field}").into());
    }
    Ok(())
}

fn assert_bool(payload: &Value, field: &str) -> Result<(), Box<dyn std::error::Error>> {
    if payload.get(field).and_then(Value::as_bool).is_none() {
        return Err(format!("missing audit bool field {field}").into());
    }
    Ok(())
}

fn assert_array(payload: &Value, field: &str) -> Result<(), Box<dyn std::error::Error>> {
    if payload.get(field).and_then(Value::as_array).is_none() {
        return Err(format!("missing audit array field {field}").into());
    }
    Ok(())
}

fn token_principal(token: &str) -> String {
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    format!("token:{}", digest.value)
}

fn schema_record(
    tenant: &str,
    namespace: &str,
    schema_id: &str,
) -> decision_gate_core::DataShapeRecord {
    decision_gate_core::DataShapeRecord {
        tenant_id: TenantId::new(tenant),
        namespace_id: NamespaceId::new(namespace),
        schema_id: decision_gate_core::DataShapeId::new(schema_id),
        version: decision_gate_core::DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("audit schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}
