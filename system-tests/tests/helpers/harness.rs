// system-tests/tests/helpers/harness.rs
// ============================================================================
// Module: MCP Server Harness
// Description: Helpers for spawning MCP servers in system-tests.
// Purpose: Provide deterministic server startup and teardown for tests.
// Dependencies: decision-gate-mcp, tokio
// ============================================================================

//! ## Overview
//! Helpers for spawning MCP servers in system-tests.
//! Purpose: Provide deterministic server startup and teardown for tests.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;

use decision_gate_core::HashAlgorithm;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::core::hashing::hash_bytes;
use decision_gate_mcp::McpServer;
use decision_gate_mcp::ServerOverrides;
use decision_gate_mcp::config::AnchorPolicyConfig;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::DocsConfig;
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
use decision_gate_mcp::config::ServerFeedbackConfig;
use decision_gate_mcp::config::ServerLimitsConfig;
use decision_gate_mcp::config::ServerMode;
use decision_gate_mcp::config::ServerTlsConfig;
use decision_gate_mcp::config::ServerTlsTermination;
use decision_gate_mcp::config::ServerToolsConfig;
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

// Intentionally no Drop impl: allow runtime shutdown to cleanly tear down servers.

/// Returns a free loopback address for test servers.
pub fn allocate_bind_addr() -> Result<SocketAddr, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("failed to bind loopback: {err}"))?;
    let addr =
        listener.local_addr().map_err(|err| format!("failed to read listener address: {err}"))?;
    reserve_port(addr.port(), listener)?;
    Ok(addr)
}

fn reserve_port(port: u16, listener: TcpListener) -> Result<(), String> {
    port_reservations()
        .lock()
        .map_err(|_| "port reservation mutex poisoned".to_string())?
        .insert(port, listener);
    Ok(())
}

fn release_reserved_port(bind: &str) {
    let addr: SocketAddr = match bind.parse() {
        Ok(addr) => addr,
        Err(_) => return,
    };
    let Ok(mut guard) = port_reservations().lock() else {
        return;
    };
    if let Some(listener) = guard.remove(&addr.port()) {
        drop(listener);
        #[cfg(windows)]
        {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Releases a reserved bind address allocated by `allocate_bind_addr`.
pub fn release_bind_addr(bind: &str) {
    release_reserved_port(bind);
}

fn port_reservations() -> &'static Mutex<HashMap<u16, TcpListener>> {
    static PORT_RESERVATIONS: OnceLock<Mutex<HashMap<u16, TcpListener>>> = OnceLock::new();
    PORT_RESERVATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Builds a base Decision Gate config for HTTP transport.
pub fn base_http_config(bind: &str) -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig {
            transport: ServerTransport::Http,
            mode: ServerMode::Strict,
            tls_termination: ServerTlsTermination::Server,
            bind: Some(bind.to_string()),
            max_body_bytes: 1024 * 1024,
            limits: ServerLimitsConfig::default(),
            auth: Some(ServerAuthConfig {
                mode: ServerAuthMode::LocalOnly,
                bearer_tokens: Vec::new(),
                mtls_subjects: Vec::new(),
                allowed_tools: Vec::new(),
                principals: vec![
                    tenant_admin_principal("loopback", 1, 1),
                    tenant_admin_principal("stdio", 1, 1),
                ],
            }),
            tls: None,
            audit: ServerAuditConfig::default(),
            feedback: ServerFeedbackConfig::default(),
            tools: ServerToolsConfig::default(),
        },
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::new(NonZeroU64::MIN)],
            ..NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: builtin_providers(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
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
        principals: vec![tenant_admin_principal(token_principal(token), 1, 1)],
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
        principals: vec![tenant_admin_principal(subject, 1, 1)],
    });
    config
}

/// Builds a base SSE config for MCP servers.
pub fn base_sse_config(bind: &str) -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig {
            transport: ServerTransport::Sse,
            mode: ServerMode::Strict,
            tls_termination: ServerTlsTermination::Server,
            bind: Some(bind.to_string()),
            max_body_bytes: 1024 * 1024,
            limits: ServerLimitsConfig::default(),
            auth: Some(ServerAuthConfig {
                mode: ServerAuthMode::LocalOnly,
                bearer_tokens: Vec::new(),
                mtls_subjects: Vec::new(),
                allowed_tools: Vec::new(),
                principals: vec![
                    tenant_admin_principal("loopback", 1, 1),
                    tenant_admin_principal("stdio", 1, 1),
                ],
            }),
            tls: None,
            audit: ServerAuditConfig::default(),
            feedback: ServerFeedbackConfig::default(),
            tools: ServerToolsConfig::default(),
        },
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::new(NonZeroU64::MIN)],
            ..NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: builtin_providers(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
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
        principals: vec![tenant_admin_principal(token_principal(token), 1, 1)],
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
    let config = match name {
        "json" => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
            let mut table = toml::value::Table::new();
            table.insert(
                "root".to_string(),
                toml::Value::String(root.to_string_lossy().to_string()),
            );
            table.insert(
                "root_id".to_string(),
                toml::Value::String("system-tests-fixtures".to_string()),
            );
            table.insert("allow_yaml".to_string(), toml::Value::Boolean(true));
            table.insert("max_bytes".to_string(), toml::Value::Integer(1024 * 1024));
            Some(toml::Value::Table(table))
        }
        _ => None,
    };
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
        config,
    }
}

fn token_principal(token: &str) -> String {
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    format!("token:{}", digest.value)
}

fn tenant_admin_principal(
    subject: impl Into<String>,
    tenant_id: u64,
    namespace_id: u64,
) -> PrincipalConfig {
    let tenant = TenantId::from_raw(tenant_id);
    assert!(tenant.is_some(), "tenant_id must be nonzero");
    let tenant = tenant.unwrap_or(TenantId::new(NonZeroU64::MIN));
    let namespace = NamespaceId::from_raw(namespace_id);
    assert!(namespace.is_some(), "namespace_id must be nonzero");
    let namespace = namespace.unwrap_or(NamespaceId::new(NonZeroU64::MIN));
    PrincipalConfig {
        subject: subject.into(),
        policy_class: Some("prod".to_string()),
        roles: vec![PrincipalRoleConfig {
            name: "TenantAdmin".to_string(),
            tenant_id: Some(tenant),
            namespace_id: Some(namespace),
        }],
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
        .map_err(|err| format!("mcp server init join failed: {err}"))?;
    release_reserved_port(&bind);
    let server = server.map_err(|err| err.to_string())?;
    let join = tokio::spawn(async move { server.serve().await });
    Ok(McpServerHandle {
        base_url,
        join,
    })
}

/// Spawns an MCP server with overrides in the background and returns a handle.
pub async fn spawn_mcp_server_with_overrides(
    config: DecisionGateConfig,
    overrides: ServerOverrides,
) -> Result<McpServerHandle, String> {
    let bind =
        config.server.bind.clone().ok_or_else(|| "missing bind for server config".to_string())?;
    let scheme = if config.server.tls.is_some() { "https" } else { "http" };
    let base_url = format!("{scheme}://{bind}/rpc");
    let server = tokio::task::spawn_blocking(move || {
        McpServer::from_config_with_overrides(config, overrides)
    })
    .await
    .map_err(|err| format!("mcp server init join failed: {err}"))?;
    release_reserved_port(&bind);
    let server = server.map_err(|err| err.to_string())?;
    let join = tokio::spawn(async move { server.serve().await });
    Ok(McpServerHandle {
        base_url,
        join,
    })
}
