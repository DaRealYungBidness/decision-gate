//! Enterprise transport parity system tests.
// enterprise-system-tests/tests/transport_parity.rs
// ============================================================================
// Module: Transport Parity Tests
// Description: Validate HTTP and stdio transport parity for enterprise config.
// Purpose: Ensure transport surfaces produce identical results.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::RunState;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_enterprise::config::EnterpriseConfig;
use decision_gate_enterprise::config::EnterpriseUsageConfig;
use decision_gate_enterprise::config::UsageLedgerConfig;
use decision_gate_enterprise::config::UsageLedgerType;
use decision_gate_enterprise::tenant_authz::NamespaceScope;
use decision_gate_enterprise::tenant_authz::PrincipalScope;
use decision_gate_enterprise::tenant_authz::TenantAuthzPolicy;
use decision_gate_enterprise::tenant_authz::TenantScope;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::stdio_client::StdioMcpClient;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread")]
async fn transport_parity_enterprise() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("transport_parity_enterprise")?;

    let temp_dir = tempfile::TempDir::new()?;
    let http_path = temp_dir.path().join("decision-gate-http.toml");
    let stdio_path = temp_dir.path().join("decision-gate-stdio.toml");
    let enterprise_path = temp_dir.path().join("decision-gate-enterprise.toml");

    let bind = allocate_bind_addr()?.to_string();
    write_config(&http_path, "http", Some(&bind))?;
    write_config(&stdio_path, "stdio", None)?;
    write_enterprise_config(&enterprise_path)?;

    let http_config = DecisionGateConfig::load(Some(&http_path))?;

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![
            PrincipalScope {
                principal_id: "loopback".to_string(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                }],
            },
            PrincipalScope {
                principal_id: "stdio".to_string(),
                tenants: vec![TenantScope {
                    tenant_id: TenantId::new("tenant-1"),
                    namespaces: NamespaceScope::All,
                }],
            },
        ],
        require_tenant: true,
    };

    let enterprise_config = EnterpriseConfig {
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Memory,
                sqlite_path: None,
            },
            ..EnterpriseUsageConfig::default()
        },
        ..EnterpriseConfig::load(Some(&enterprise_path))?
    };

    let tenant_authorizer = std::sync::Arc::new(
        decision_gate_enterprise::tenant_authz::MappedTenantAuthorizer::new(tenant_policy.clone()),
    );

    let server = spawn_enterprise_server_from_configs(
        http_config,
        enterprise_config.clone(),
        tenant_authorizer,
        std::sync::Arc::new(decision_gate_mcp::McpNoopAuditSink),
    )
    .await?;
    let http_client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&http_client, Duration::from_secs(5)).await?;

    let tenant_policy_json = serde_json::to_string(&tenant_policy)?;
    helpers::env::set_var("DECISION_GATE_ENTERPRISE_TENANT_POLICY", &tenant_policy_json);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_enterprise_stdio_server"));
    let stderr_path = reporter.artifacts().root().join("stdio.stderr.log");
    let mut stdio_client =
        StdioMcpClient::spawn(&binary, &stdio_path, &enterprise_path, &stderr_path)?;

    let http_tools = http_client.list_tools().await?;
    let stdio_tools = stdio_client.list_tools().await?;
    let http_names: BTreeSet<_> = http_tools.iter().map(|tool| tool.name.clone()).collect();
    let stdio_names: BTreeSet<_> = stdio_tools.iter().map(|tool| tool.name.clone()).collect();
    if http_names != stdio_names {
        return Err("tool list mismatch between transports".into());
    }

    let mut fixture = ScenarioFixture::time_after("parity", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let http_define: ScenarioDefineResponse = http_client
        .call_tool_typed("scenario_define", serde_json::to_value(&define_request)?)
        .await?;
    let stdio_define_value =
        stdio_client.call_tool("scenario_define", serde_json::to_value(&define_request)?).await?;
    let stdio_define: ScenarioDefineResponse = serde_json::from_value(stdio_define_value)?;
    if serde_json::to_value(&http_define)? != serde_json::to_value(&stdio_define)? {
        return Err("scenario_define response mismatch".into());
    }

    let start_request = ScenarioStartRequest {
        scenario_id: fixture.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(0),
        issue_entry_packets: false,
    };
    let http_start: RunState = http_client
        .call_tool_typed("scenario_start", serde_json::to_value(&start_request)?)
        .await?;
    let stdio_start_value =
        stdio_client.call_tool("scenario_start", serde_json::to_value(&start_request)?).await?;
    let stdio_start: RunState = serde_json::from_value(stdio_start_value)?;
    if serde_json::to_value(&http_start)? != serde_json::to_value(&stdio_start)? {
        return Err("scenario_start response mismatch".into());
    }

    let status_request = ScenarioStatusRequest {
        scenario_id: fixture.scenario_id.clone(),
        request: decision_gate_core::StatusRequest {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: fixture.namespace_id.clone(),
            run_id: fixture.run_id.clone(),
            requested_at: Timestamp::Logical(1),
            correlation_id: None,
        },
    };
    let http_status: Value =
        http_client.call_tool("scenario_status", serde_json::to_value(&status_request)?).await?;
    let stdio_status: Value =
        stdio_client.call_tool("scenario_status", serde_json::to_value(&status_request)?).await?;
    if http_status != stdio_status {
        return Err("scenario_status response mismatch".into());
    }

    let mut transcripts = http_client.transcript();
    transcripts.extend(stdio_client.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["HTTP and stdio parity validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "stdio.stderr.log".to_string(),
        ],
    )?;

    helpers::env::remove_var("DECISION_GATE_ENTERPRISE_TENANT_POLICY");
    stdio_client.shutdown()?;
    server.shutdown().await;
    Ok(())
}

fn write_config(
    path: &Path,
    transport: &str,
    bind: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut content = String::new();
    content.push_str("[server]\n");
    content.push_str(&format!("transport = \"{transport}\"\n"));
    content.push_str("mode = \"strict\"\n");
    if let Some(bind) = bind {
        content.push_str(&format!("bind = \"{bind}\"\n"));
    }
    content.push_str("max_body_bytes = 1048576\n\n");
    content.push_str("[namespace]\n");
    content.push_str("allow_default = true\n");
    content.push_str("default_tenants = [\"tenant-1\"]\n\n");
    content.push_str("[[providers]]\n");
    content.push_str("name = \"time\"\n");
    content.push_str("type = \"builtin\"\n");
    std::fs::write(path, content)?;
    Ok(())
}

fn write_enterprise_config(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let content = "[usage.ledger]\nledger_type = \"memory\"\n";
    std::fs::write(path, content)?;
    Ok(())
}
