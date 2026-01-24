// system-tests/tests/helpers/harness.rs
// ============================================================================
// Module: MCP Server Harness
// Description: Helpers for spawning MCP servers in system-tests.
// Purpose: Provide deterministic server startup and teardown for tests.
// Dependencies: decision-gate-mcp, tokio
// ============================================================================

use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_mcp::McpServer;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::ServerAuditConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::ServerLimitsConfig;
use decision_gate_mcp::config::ServerTlsConfig;
use decision_gate_mcp::config::ServerTransport;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::server::McpServerError;
use tokio::task::JoinHandle;

use super::mcp_client::McpHttpClient;

/// Handle for a spawned MCP server.
pub struct McpServerHandle {
    base_url: String,
    join: JoinHandle<Result<(), McpServerError>>,
}

impl McpServerHandle {
    /// Returns the MCP base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Builds an HTTP client for the server.
    pub fn client(&self, timeout: Duration) -> Result<McpHttpClient, String> {
        McpHttpClient::new(self.base_url.clone(), timeout)
    }

    /// Shuts down the server task.
    pub async fn shutdown(self) {
        self.join.abort();
        let _ = self.join.await;
    }
}

// Intentionally no Drop impl: allow runtime shutdown to cleanly tear down servers.

/// Returns a free loopback address for test servers.
pub fn allocate_bind_addr() -> Result<SocketAddr, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("failed to bind loopback: {err}"))?;
    let addr =
        listener.local_addr().map_err(|err| format!("failed to read listener address: {err}"))?;
    drop(listener);
    Ok(addr)
}

/// Builds a base Decision Gate config for HTTP transport.
pub fn base_http_config(bind: &str) -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig {
            transport: ServerTransport::Http,
            bind: Some(bind.to_string()),
            max_body_bytes: 1024 * 1024,
            limits: ServerLimitsConfig::default(),
            auth: None,
            tls: None,
            audit: ServerAuditConfig::default(),
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        providers: builtin_providers(),
    }
}

/// Builds a base HTTP config with bearer auth enabled.
pub fn base_http_config_with_bearer(bind: &str, token: &str) -> DecisionGateConfig {
    let mut config = base_http_config(bind);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
    });
    config
}

/// Builds a base HTTP config with TLS enabled.
pub fn base_http_config_with_tls(
    bind: &str,
    cert_path: &Path,
    key_path: &Path,
) -> DecisionGateConfig {
    let mut config = base_http_config(bind);
    config.server.tls = Some(ServerTlsConfig {
        cert_path: cert_path.display().to_string(),
        key_path: key_path.display().to_string(),
        client_ca_path: None,
        require_client_cert: true,
    });
    config
}

/// Builds a base HTTP config with TLS+mTLS enabled.
pub fn base_http_config_with_mtls_tls(
    bind: &str,
    cert_path: &Path,
    key_path: &Path,
    ca_path: &Path,
    require_client_cert: bool,
) -> DecisionGateConfig {
    let mut config = base_http_config(bind);
    config.server.tls = Some(ServerTlsConfig {
        cert_path: cert_path.display().to_string(),
        key_path: key_path.display().to_string(),
        client_ca_path: Some(ca_path.display().to_string()),
        require_client_cert,
    });
    config
}

/// Builds a base HTTP config with mTLS subject auth enabled.
pub fn base_http_config_with_mtls(bind: &str, subject: &str) -> DecisionGateConfig {
    let mut config = base_http_config(bind);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec![subject.to_string()],
        allowed_tools: Vec::new(),
    });
    config
}

/// Builds a base SSE config for MCP servers.
pub fn base_sse_config(bind: &str) -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig {
            transport: ServerTransport::Sse,
            bind: Some(bind.to_string()),
            max_body_bytes: 1024 * 1024,
            limits: ServerLimitsConfig::default(),
            auth: None,
            tls: None,
            audit: ServerAuditConfig::default(),
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        providers: builtin_providers(),
    }
}

/// Builds a base SSE config with bearer auth enabled.
pub fn base_sse_config_with_bearer(bind: &str, token: &str) -> DecisionGateConfig {
    let mut config = base_sse_config(bind);
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token.to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
    });
    config
}

/// Builds a config with a federated MCP provider.
pub fn config_with_provider(
    bind: &str,
    provider_name: &str,
    url: &str,
    capabilities_path: &Path,
) -> DecisionGateConfig {
    let mut config = base_http_config(bind);
    config.providers.push(ProviderConfig {
        name: provider_name.to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some(url.to_string()),
        allow_insecure_http: true,
        capabilities_path: Some(PathBuf::from(capabilities_path)),
        auth: None,
        trust: None,
        allow_raw: true,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    });
    config
}

/// Builds a config with a federated MCP provider using explicit timeouts.
pub fn config_with_provider_timeouts(
    bind: &str,
    provider_name: &str,
    url: &str,
    capabilities_path: &Path,
    timeouts: ProviderTimeoutConfig,
) -> DecisionGateConfig {
    let mut config = base_http_config(bind);
    config.providers.push(ProviderConfig {
        name: provider_name.to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some(url.to_string()),
        allow_insecure_http: true,
        capabilities_path: Some(PathBuf::from(capabilities_path)),
        auth: None,
        trust: None,
        allow_raw: true,
        timeouts,
        config: None,
    });
    config
}

fn builtin_providers() -> Vec<ProviderConfig> {
    vec![
        builtin_provider("time"),
        builtin_provider("env"),
        builtin_provider("json"),
        builtin_provider("http"),
    ]
}

fn builtin_provider(name: &str) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
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
    }
}

/// Spawns an MCP server in the background and returns a handle.
pub async fn spawn_mcp_server(config: DecisionGateConfig) -> Result<McpServerHandle, String> {
    let bind =
        config.server.bind.clone().ok_or_else(|| "missing bind for server config".to_string())?;
    let scheme = if config.server.tls.is_some() { "https" } else { "http" };
    let base_url = format!("{scheme}://{bind}/rpc");
    let server = tokio::task::spawn_blocking(move || McpServer::from_config(config))
        .await
        .map_err(|err| format!("mcp server init join failed: {err}"))?
        .map_err(|err| err.to_string())?;
    let join = tokio::spawn(async move { server.serve().await });
    Ok(McpServerHandle {
        base_url,
        join,
    })
}
