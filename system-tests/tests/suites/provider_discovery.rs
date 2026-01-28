// system-tests/tests/suites/provider_discovery.rs
// ============================================================================
// Module: Provider Discovery System Tests
// Description: End-to-end discovery of provider contracts and schemas.
// Purpose: Validate MCP discovery tools over HTTP and stdio transports.
// Dependencies: system-tests helpers
// ============================================================================

//! Provider contract discovery system tests.


use std::path::PathBuf;
use std::time::Duration;

use decision_gate_mcp::tools::ProviderContractGetRequest;
use decision_gate_mcp::tools::ProviderContractGetResponse;
use decision_gate_mcp::tools::ProviderSchemaGetRequest;
use decision_gate_mcp::tools::ProviderSchemaGetResponse;
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
    assert_eq!(contract.provider_id, "time");
    assert_eq!(contract.contract.provider_id, "time");

    let schema_request = ProviderSchemaGetRequest {
        provider_id: "time".to_string(),
        predicate: "after".to_string(),
    };
    let schema: ProviderSchemaGetResponse = client
        .call_tool_typed("provider_schema_get", serde_json::to_value(&schema_request)?)
        .await?;
    assert_eq!(schema.provider_id, "time");
    assert_eq!(schema.predicate, "after");
    assert!(!schema.allowed_comparators.is_empty());

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
    assert_eq!(contract.provider_id, "time");

    let schema_request = ProviderSchemaGetRequest {
        provider_id: "time".to_string(),
        predicate: "after".to_string(),
    };
    let schema_output =
        client.call_tool("provider_schema_get", serde_json::to_value(&schema_request)?).await?;
    let schema: ProviderSchemaGetResponse = serde_json::from_value(schema_output)?;
    assert_eq!(schema.provider_id, "time");
    assert_eq!(schema.predicate, "after");

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
    Ok(())
}
