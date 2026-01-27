// enterprise-system-tests/tests/helpers/harness.rs
// ============================================================================
// Module: Enterprise MCP Server Harness
// Description: Helpers for spawning enterprise MCP servers.
// Purpose: Provide deterministic server startup and teardown for tests.
// Dependencies: decision-gate-enterprise, decision-gate-mcp, tokio
// ============================================================================

use std::net::SocketAddr;
use std::net::TcpListener;
use std::time::Duration;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_enterprise::config::EnterpriseConfig;
use decision_gate_enterprise::server::EnterpriseServerOptions;
use decision_gate_enterprise::server::build_enterprise_server;
use decision_gate_enterprise::server::build_enterprise_server_from_configs;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::McpAuditSink;
use decision_gate_mcp::McpMetrics;
use decision_gate_mcp::McpServer;
use decision_gate_mcp::NoopMetrics;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::config::AnchorPolicyConfig;
use decision_gate_mcp::config::DecisionGateConfig as McpConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::NamespaceConfig;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::SchemaRegistryConfig;
use decision_gate_mcp::config::ServerAuditConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::ServerLimitsConfig;
use decision_gate_mcp::config::ServerMode;
use decision_gate_mcp::config::ServerTlsConfig;
use decision_gate_mcp::config::ServerTransport;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::config::ValidationConfig;
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
    McpConfig {
        server: ServerConfig {
            transport: ServerTransport::Http,
            mode: ServerMode::Strict,
            bind: Some(bind.to_string()),
            max_body_bytes: 1024 * 1024,
            limits: ServerLimitsConfig::default(),
            auth: Some(ServerAuthConfig {
                mode: ServerAuthMode::LocalOnly,
                bearer_tokens: Vec::new(),
                mtls_subjects: Vec::new(),
                allowed_tools: Vec::new(),
                principals: vec![
                    tenant_admin_principal("loopback", "tenant-1", "default"),
                    tenant_admin_principal("stdio", "tenant-1", "default"),
                ],
            }),
            tls: None,
            audit: ServerAuditConfig::default(),
        },
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::new("tenant-1")],
            ..NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: builtin_providers(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        source_modified_at: None,
    }
}

/// Builds a base HTTP config with TLS enabled.
pub fn base_http_config_with_tls(
    bind: &str,
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
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
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
    ca_path: &std::path::Path,
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
        principals: vec![tenant_admin_principal(subject, "tenant-1", "default")],
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

fn tenant_admin_principal(
    subject: impl Into<String>,
    tenant_id: &str,
    namespace_id: &str,
) -> PrincipalConfig {
    PrincipalConfig {
        subject: subject.into(),
        policy_class: Some("prod".to_string()),
        roles: vec![PrincipalRoleConfig {
            name: "TenantAdmin".to_string(),
            tenant_id: Some(TenantId::new(tenant_id)),
            namespace_id: Some(NamespaceId::new(namespace_id)),
        }],
    }
}

/// Spawns an enterprise MCP server using explicit enterprise options.
pub async fn spawn_enterprise_server(
    config: DecisionGateConfig,
    options: EnterpriseServerOptions,
) -> Result<McpServerHandle, String> {
    let bind =
        config.server.bind.clone().ok_or_else(|| "missing bind for server config".to_string())?;
    let scheme = if config.server.tls.is_some() { "https" } else { "http" };
    let base_url = format!("{scheme}://{bind}/rpc");
    let server = tokio::task::spawn_blocking(move || build_enterprise_server(config, options))
        .await
        .map_err(|err| format!("enterprise server init join failed: {err}"))?
        .map_err(|err| err.to_string())?;
    let join = tokio::spawn(async move { server.serve().await });
    Ok(McpServerHandle {
        base_url,
        join,
    })
}

/// Spawns an enterprise MCP server using OSS + enterprise configs.
pub async fn spawn_enterprise_server_from_configs(
    config: DecisionGateConfig,
    enterprise_config: EnterpriseConfig,
    tenant_authorizer: std::sync::Arc<dyn TenantAuthorizer>,
    audit_sink: std::sync::Arc<dyn McpAuditSink>,
) -> Result<McpServerHandle, String> {
    let bind =
        config.server.bind.clone().ok_or_else(|| "missing bind for server config".to_string())?;
    let scheme = if config.server.tls.is_some() { "https" } else { "http" };
    let base_url = format!("{scheme}://{bind}/rpc");
    let server = tokio::task::spawn_blocking(move || {
        build_enterprise_server_from_configs(
            config,
            &enterprise_config,
            tenant_authorizer,
            audit_sink,
            std::sync::Arc::new(NoopMetrics),
        )
    })
    .await
    .map_err(|err| format!("enterprise server init join failed: {err}"))?
    .map_err(|err| err.to_string())?;
    let join = tokio::spawn(async move { server.serve().await });
    Ok(McpServerHandle {
        base_url,
        join,
    })
}

/// Builds an enterprise stdio server from configs (for stdio binary use).
pub fn build_enterprise_server_for_stdio(
    config: DecisionGateConfig,
    enterprise_config: EnterpriseConfig,
    tenant_authorizer: std::sync::Arc<dyn TenantAuthorizer>,
    audit_sink: std::sync::Arc<dyn McpAuditSink>,
    metrics: std::sync::Arc<dyn McpMetrics>,
) -> Result<McpServer, McpServerError> {
    build_enterprise_server_from_configs(
        config,
        &enterprise_config,
        tenant_authorizer,
        audit_sink,
        metrics,
    )
}
