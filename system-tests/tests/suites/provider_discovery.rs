// system-tests/tests/suites/provider_discovery.rs
// ============================================================================
// Module: Provider Discovery System Tests
// Description: End-to-end discovery of provider contracts and schemas.
// Purpose: Validate MCP discovery tools over HTTP and stdio transports.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! End-to-end discovery of provider contracts and schemas.
//! Purpose: Validate MCP discovery tools over HTTP and stdio transports.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::path::PathBuf;
use std::time::Duration;

use decision_gate_mcp::tools::ProviderCheckSchemaGetRequest;
use decision_gate_mcp::tools::ProviderCheckSchemaGetResponse;
use decision_gate_mcp::tools::ProviderContractGetRequest;
use decision_gate_mcp::tools::ProviderContractGetResponse;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::readiness::wait_for_stdio_ready;
use helpers::stdio_client::StdioMcpClient;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_discovery_tools() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_discovery_tools")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.provider_discovery.denylist.retain(|_| false);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let contract_request = ProviderContractGetRequest {
        provider_id: "time".to_string(),
    };
    let contract: ProviderContractGetResponse = client
        .call_tool_typed("provider_contract_get", serde_json::to_value(&contract_request)?)
        .await?;
    if contract.provider_id != "time" {
        return Err(format!("expected provider_id time, got {}", contract.provider_id).into());
    }
    if contract.contract.provider_id != "time" {
        return Err(format!(
            "expected contract provider_id time, got {}",
            contract.contract.provider_id
        )
        .into());
    }

    let schema_request = ProviderCheckSchemaGetRequest {
        provider_id: "time".to_string(),
        check_id: "after".to_string(),
    };
    let schema: ProviderCheckSchemaGetResponse = client
        .call_tool_typed("provider_check_schema_get", serde_json::to_value(&schema_request)?)
        .await?;
    if schema.provider_id != "time" {
        return Err(format!("expected schema provider_id time, got {}", schema.provider_id).into());
    }
    if schema.check_id != "after" {
        return Err(format!("expected check_id after, got {}", schema.check_id).into());
    }
    if schema.allowed_comparators.is_empty() {
        return Err("expected allowed_comparators to be non-empty".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider discovery tools responded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stdio_provider_discovery_tools() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("stdio_provider_discovery_tools")?;
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[server]
transport = "stdio"
mode = "strict"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "stdio"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[provider_discovery]
denylist = []
max_response_bytes = 1048576

[[providers]]
name = "time"
type = "builtin"
"#;
    std::fs::write(&config_path, config_contents)?;

    let stderr_path = reporter.artifacts().root().join("mcp.stderr.log");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
    wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;

    let contract_request = ProviderContractGetRequest {
        provider_id: "time".to_string(),
    };
    let contract_output =
        client.call_tool("provider_contract_get", serde_json::to_value(&contract_request)?).await?;
    let contract: ProviderContractGetResponse = serde_json::from_value(contract_output)?;
    if contract.provider_id != "time" {
        return Err(format!("expected provider_id time, got {}", contract.provider_id).into());
    }

    let schema_request = ProviderCheckSchemaGetRequest {
        provider_id: "time".to_string(),
        check_id: "after".to_string(),
    };
    let schema_output = client
        .call_tool("provider_check_schema_get", serde_json::to_value(&schema_request)?)
        .await?;
    let schema: ProviderCheckSchemaGetResponse = serde_json::from_value(schema_output)?;
    if schema.provider_id != "time" {
        return Err(format!("expected schema provider_id time, got {}", schema.provider_id).into());
    }
    if schema.check_id != "after" {
        return Err(format!("expected check_id after, got {}", schema.check_id).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["stdio provider discovery tools responded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "mcp.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_discovery_denylist_and_size_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("provider_discovery_denylist_and_size_limits")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.provider_discovery.denylist = vec!["time".to_string()];
    config.provider_discovery.max_response_bytes = 64;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let denied_request = ProviderContractGetRequest {
        provider_id: "time".to_string(),
    };
    let Err(err) =
        client.call_tool("provider_contract_get", serde_json::to_value(&denied_request)?).await
    else {
        return Err("expected provider discovery denial".into());
    };
    if !err.contains("provider contract disclosure denied") && !err.contains("unauthorized") {
        return Err(format!("unexpected denylist error: {err}").into());
    }

    let allowed_request = ProviderContractGetRequest {
        provider_id: "env".to_string(),
    };
    let Err(err) =
        client.call_tool("provider_contract_get", serde_json::to_value(&allowed_request)?).await
    else {
        return Err("expected provider discovery size limit rejection".into());
    };
    if !err.contains("provider discovery response exceeds size limit") {
        return Err(format!("unexpected size limit error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["provider discovery denylist and size limits enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}
