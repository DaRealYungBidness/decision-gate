// system-tests/tests/suites/config_defaults.rs
// =============================================================================
// Module: Config Defaults Tests
// Description: Ensure minimal config defaults are valid and server-startable.
// Purpose: Confirm runtime defaults align with documented baseline behavior.
// Dependencies: system-tests helpers
// =============================================================================

use std::time::Duration;

use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::ServerTransport;
use helpers::harness::allocate_bind_addr;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn minimal_config_starts_http_server() -> Result<(), Box<dyn std::error::Error>> {
    let mut config: DecisionGateConfig = toml::from_str("")?;

    if config.server.transport != ServerTransport::Stdio {
        return Err("expected default server transport to be stdio".into());
    }
    if !config.validation.strict {
        return Err("validation.strict default should be true".into());
    }
    if !config.evidence.require_provider_opt_in {
        return Err("evidence.require_provider_opt_in default should be true".into());
    }

    let bind = allocate_bind_addr()?.to_string();
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some(bind);
    config.providers = vec![ProviderConfig {
        name: "time".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(10))?;
    wait_for_server_ready(&client, Duration::from_secs(10)).await?;
    server.shutdown().await;
    Ok(())
}
