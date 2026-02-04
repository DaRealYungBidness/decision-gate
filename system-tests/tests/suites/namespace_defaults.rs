// system-tests/tests/suites/namespace_defaults.rs
// ============================================================================
// Module: Namespace Default Policy Tests
// Description: Validate default namespace allowlist enforcement.
// Purpose: Ensure default namespace is restricted to allowlisted tenants.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Validate default namespace allowlist enforcement.
//! Purpose: Ensure default namespace is restricted to allowlisted tenants.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::num::NonZeroU64;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

use crate::helpers;

fn tenant_id(value: u64) -> TenantId {
    TenantId::new(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
}

fn namespace_id(value: u64) -> NamespaceId {
    NamespaceId::new(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
}

#[tokio::test(flavor = "multi_thread")]
async fn default_namespace_allowlist_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("default_namespace_allowlist_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.namespace.allow_default = true;
    config.namespace.default_tenants = vec![tenant_id(100)];
    if let Some(auth) = config.server.auth.as_mut()
        && let Some(principal) = auth.principals.iter_mut().find(|p| p.subject == "loopback")
    {
        principal.roles.push(PrincipalRoleConfig {
            name: "TenantAdmin".to_string(),
            tenant_id: Some(tenant_id(100)),
            namespace_id: Some(namespace_id(1)),
        });
    }

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let allowed_record = DataShapeRecord {
        tenant_id: tenant_id(100),
        namespace_id: namespace_id(1),
        schema_id: DataShapeId::new("allowed-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("allowed schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let request = SchemasRegisterRequest {
        record: allowed_record,
    };
    client.call_tool("schemas_register", serde_json::to_value(&request)?).await?;

    let denied_record = DataShapeRecord {
        tenant_id: tenant_id(101),
        namespace_id: namespace_id(1),
        schema_id: DataShapeId::new("denied-schema"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {"after": {"type": "boolean"}},
            "required": ["after"]
        }),
        description: Some("denied schema".to_string()),
        created_at: Timestamp::Logical(2),
        signing: None,
    };
    let request = SchemasRegisterRequest {
        record: denied_record,
    };
    let Err(err) = client.call_tool("schemas_register", serde_json::to_value(&request)?).await
    else {
        return Err("expected default namespace denial for tenant".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    let fixture = ScenarioFixture::time_after("default-namespace", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let Err(err) =
        client.call_tool("scenario_define", serde_json::to_value(&define_request)?).await
    else {
        return Err("expected scenario_define denial without tenant".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized, got {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["default namespace allowlist enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
