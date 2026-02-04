// system-tests/tests/suites/audit_registry.rs
// ============================================================================
// Module: Registry Audit Tests
// Description: Validate registry and security audit events end-to-end.
// Purpose: Ensure audit logs capture registry ACL decisions and security posture.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Validate registry and security audit events end-to-end.
//! Purpose: Ensure audit logs capture registry ACL decisions and security posture.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::num::NonZeroU64;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::core::hashing::HashAlgorithm;
use decision_gate_core::core::hashing::hash_bytes;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::ServerAuditConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use serde_json::Value;
use serde_json::json;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "End-to-end audit assertions are clearer in one flow.")]
async fn registry_security_audit_events() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("registry_security_audit_events")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.dev.permissive = true;

    let audit_path = reporter.artifacts().root().join("audit.log");
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some(audit_path.display().to_string()),
        log_precheck_payloads: false,
    };

    let allowed_token = "audit-allowed".to_string();
    let denied_token = "audit-denied".to_string();

    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![allowed_token.clone(), denied_token.clone()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![PrincipalConfig {
            subject: token_subject(&allowed_token),
            policy_class: Some("prod".to_string()),
            roles: vec![PrincipalRoleConfig {
                name: "TenantAdmin".to_string(),
                tenant_id: Some(tenant_id_one()),
                namespace_id: Some(namespace_id_one()),
            }],
        }],
    });

    let server = spawn_mcp_server(config).await?;
    let allowed = server.client(Duration::from_secs(5))?.with_bearer_token(allowed_token.clone());
    wait_for_server_ready(&allowed, Duration::from_secs(5)).await?;

    let record = DataShapeRecord {
        tenant_id: tenant_id_one(),
        namespace_id: namespace_id_one(),
        schema_id: DataShapeId::new("audit-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("audit schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let request = SchemasRegisterRequest {
        record: record.clone(),
    };
    allowed.call_tool("schemas_register", serde_json::to_value(&request)?).await?;

    let denied = server.client(Duration::from_secs(5))?.with_bearer_token(denied_token.clone());
    let Err(err) = denied.call_tool("schemas_register", serde_json::to_value(&request)?).await
    else {
        return Err("expected registry deny audit event".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let contents = fs::read_to_string(&audit_path)?;
    let mut events: Vec<Value> = Vec::new();
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        events.push(value);
    }

    let security_event = events
        .iter()
        .find(|event| event.get("event").and_then(Value::as_str) == Some("security_audit"))
        .ok_or("missing security_audit event")?;
    if security_event.get("dev_permissive").and_then(Value::as_bool) != Some(true) {
        return Err("security audit missing dev_permissive=true".into());
    }
    if security_event.get("namespace_authority").and_then(Value::as_str) != Some("dg_registry") {
        return Err("security audit missing namespace_authority".into());
    }

    let registry_events: Vec<&Value> = events
        .iter()
        .filter(|event| event.get("event").and_then(Value::as_str) == Some("registry_audit"))
        .collect();
    if registry_events.is_empty() {
        return Err("missing registry_audit events".into());
    }

    let allowed_event = registry_events
        .iter()
        .find(|event| event.get("allowed").and_then(Value::as_bool) == Some(true))
        .ok_or("missing allowed registry_audit event")?;
    let denied_event = registry_events
        .iter()
        .find(|event| event.get("allowed").and_then(Value::as_bool) == Some(false))
        .ok_or("missing denied registry_audit event")?;

    if allowed_event.get("action").and_then(Value::as_str) != Some("register") {
        return Err("allowed registry_audit missing register action".into());
    }
    if denied_event.get("action").and_then(Value::as_str) != Some("register") {
        return Err("denied registry_audit missing register action".into());
    }

    let allowed_roles = allowed_event
        .get("roles")
        .and_then(Value::as_array)
        .ok_or("allowed registry_audit missing roles")?;
    if !allowed_roles.iter().any(|role| role.as_str() == Some("TenantAdmin")) {
        return Err("allowed registry_audit missing TenantAdmin role".into());
    }

    let denied_principal = denied_event
        .get("principal_id")
        .and_then(Value::as_str)
        .ok_or("denied registry_audit missing principal_id")?;
    if denied_principal != token_subject(&denied_token) {
        return Err("denied registry_audit principal_id mismatch".into());
    }

    let mut transcripts = allowed.transcript();
    transcripts.extend(denied.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["registry + security audit events validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "audit.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

fn token_subject(token: &str) -> String {
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    format!("token:{}", digest.value)
}
