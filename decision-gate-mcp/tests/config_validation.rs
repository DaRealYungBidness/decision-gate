// decision-gate-mcp/tests/config_validation.rs
// ============================================================================
// Module: Configuration Validation Tests
// Description: Tests for MCP config loading and validation.
// Purpose: Verify security constraints are enforced during config parsing.
// Dependencies: decision-gate-mcp
// ============================================================================

//! ## Overview
//! Tests configuration validation including loopback enforcement, path limits,
//! and provider configuration requirements.
//!
//! Security posture: Configuration is untrusted input - all limits must be enforced.
//! Threat model: TM-CFG-001 - Configuration injection or bypass.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

use std::path::PathBuf;

use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::ServerTransport;
use decision_gate_mcp::config::TrustConfig;
use tempfile::TempDir;

/// Validates a standalone server config via the public config validator.
fn validate_server_config(
    server: ServerConfig,
) -> Result<(), decision_gate_mcp::config::ConfigError> {
    let mut config = DecisionGateConfig {
        server,
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        providers: Vec::new(),
    };
    config.validate()
}

/// Validates a standalone provider config via the public config validator.
fn validate_provider_config(
    provider: ProviderConfig,
) -> Result<(), decision_gate_mcp::config::ConfigError> {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        providers: vec![provider],
    };
    config.validate()
}

// ============================================================================
// SECTION: Server Config Validation Tests
// ============================================================================

/// Verifies stdio transport requires no bind address.
#[test]
fn server_stdio_no_bind_required() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        bind: None,
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies max_body_bytes must be non-zero.
#[test]
fn server_max_body_bytes_zero_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        bind: None,
        max_body_bytes: 0,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("max_body_bytes"));
}

/// Verifies HTTP transport requires bind address.
#[test]
fn server_http_requires_bind() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: None,
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("bind address"));
}

/// Verifies SSE transport requires bind address.
#[test]
fn server_sse_requires_bind() {
    let config = ServerConfig {
        transport: ServerTransport::Sse,
        bind: None,
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("bind address"));
}

/// Verifies HTTP transport allows loopback bind.
#[test]
fn server_http_loopback_allowed() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies HTTP transport allows IPv6 loopback.
#[test]
fn server_http_ipv6_loopback_allowed() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("[::1]:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies HTTP transport rejects non-loopback bind.
#[test]
fn server_http_non_loopback_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("0.0.0.0:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("loopback"));
}

/// Verifies HTTP transport rejects external IP bind.
#[test]
fn server_http_external_ip_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("192.168.1.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies invalid bind address format rejected.
#[test]
fn server_invalid_bind_format_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("not-an-address".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("invalid bind"));
}

/// Verifies empty bind address rejected.
#[test]
fn server_empty_bind_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("   ".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: None,
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies non-loopback bind is allowed with bearer auth configured.
#[test]
fn server_http_non_loopback_allowed_with_bearer_auth() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("0.0.0.0:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token-1".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
        }),
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies stdio transport rejects bearer auth mode.
#[test]
fn server_stdio_rejects_bearer_auth() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        bind: None,
        max_body_bytes: 1024 * 1024,
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token-1".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
        }),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies bearer auth requires at least one token.
#[test]
fn server_auth_bearer_requires_token() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: Vec::new(),
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
        }),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies tool allowlist rejects unknown tools.
#[test]
fn server_auth_rejects_unknown_tool_in_allowlist() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token-1".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: vec!["invalid_tool".to_string()],
        }),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies mTLS auth requires at least one subject.
#[test]
fn server_auth_mtls_requires_subjects() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::Mtls,
            bearer_tokens: Vec::new(),
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
        }),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Provider Config Validation Tests
// ============================================================================

/// Verifies builtin provider with name is valid.
#[test]
fn provider_builtin_valid() {
    let config = ProviderConfig {
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
    };
    assert!(validate_provider_config(config).is_ok());
}

/// Verifies empty provider name rejected.
#[test]
fn provider_empty_name_rejected() {
    let config = ProviderConfig {
        name: String::new(),
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("name"));
}

/// Verifies whitespace-only provider name rejected.
#[test]
fn provider_whitespace_name_rejected() {
    let config = ProviderConfig {
        name: "   ".to_string(),
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
}

/// Verifies MCP provider requires command or URL.
#[test]
fn provider_mcp_requires_command_or_url() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("command or url"));
}

/// Verifies MCP provider with command is valid.
#[test]
fn provider_mcp_with_command_valid() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["./provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    assert!(validate_provider_config(config).is_ok());
}

/// Verifies MCP provider with HTTPS URL is valid.
#[test]
fn provider_mcp_with_https_url_valid() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    assert!(validate_provider_config(config).is_ok());
}

/// Verifies provider timeouts reject out-of-range connect values.
#[test]
fn provider_timeouts_reject_connect_out_of_range() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: 50,
            request_timeout_ms: 1_000,
        },
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("connect_timeout_ms"));
}

/// Verifies provider timeouts require request timeout >= connect timeout.
#[test]
fn provider_timeouts_reject_request_below_connect() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: 2_000,
            request_timeout_ms: 1_000,
        },
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("request_timeout_ms"));
}

// ============================================================================
// SECTION: Run State Store Validation Tests
// ============================================================================

/// Verifies sqlite run_state_store requires a path.
#[test]
fn run_state_store_sqlite_requires_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Sqlite,
            path: None,
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: None,
        },
        providers: Vec::new(),
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("run_state_store"));
}

/// Verifies memory run_state_store rejects a path.
#[test]
fn run_state_store_memory_rejects_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Memory,
            path: Some(PathBuf::from("store.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: None,
        },
        providers: Vec::new(),
    };
    let result = config.validate();
    assert!(result.is_err());
}

/// Verifies sqlite run_state_store accepts a valid path.
#[test]
fn run_state_store_sqlite_accepts_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Sqlite,
            path: Some(PathBuf::from("store.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: Some(10),
        },
        providers: Vec::new(),
    };
    let result = config.validate();
    assert!(result.is_ok());
}

/// Verifies sqlite run_state_store rejects max_versions of zero.
#[test]
fn run_state_store_sqlite_rejects_zero_retention() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Sqlite,
            path: Some(PathBuf::from("store.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: Some(0),
        },
        providers: Vec::new(),
    };
    let result = config.validate();
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Config Load Validation Tests
// ============================================================================

/// Verifies loading rejects MCP providers missing capabilities_path.
#[test]
fn config_load_rejects_mcp_without_capabilities_path() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("decision-gate.toml");
    let config = r#"
[server]
transport = "stdio"

[[providers]]
name = "echo"
type = "mcp"
command = ["echo-provider"]
"#;
    std::fs::write(&config_path, config.as_bytes()).unwrap();

    let result = DecisionGateConfig::load(Some(&config_path));
    let err = result.expect_err("expected missing capabilities_path rejection");
    assert!(err.to_string().contains("capabilities_path"));
}

/// Verifies loading accepts MCP providers with capabilities_path.
#[test]
fn config_load_accepts_mcp_with_capabilities_path() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    std::fs::write(&contract_path, "{}").unwrap();
    let config_path = temp.path().join("decision-gate.toml");
    let contract_path = contract_path.to_string_lossy().replace('\\', "/");
    let config = format!(
        r#"
[server]
transport = "stdio"

[[providers]]
name = "echo"
type = "mcp"
command = ["echo-provider"]
capabilities_path = "{}"
"#,
        contract_path
    );
    std::fs::write(&config_path, config.as_bytes()).unwrap();

    let result = DecisionGateConfig::load(Some(&config_path));
    assert!(result.is_ok());
}

/// Verifies MCP provider rejects HTTP without `allow_insecure` flag.
#[test]
fn provider_mcp_http_rejected_without_flag() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("http://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("insecure http"));
}

/// Verifies MCP provider allows HTTP with `allow_insecure` flag.
#[test]
fn provider_mcp_http_allowed_with_flag() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("http://localhost:8080/mcp".to_string()),
        allow_insecure_http: true,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    assert!(validate_provider_config(config).is_ok());
}

// ============================================================================
// SECTION: Default Value Tests
// ============================================================================

/// Verifies default server config uses stdio.
#[test]
fn default_server_is_stdio() {
    let config = ServerConfig::default();
    assert_eq!(config.transport, ServerTransport::Stdio);
}

/// Verifies default max body bytes is 1MB.
#[test]
fn default_max_body_bytes_is_1mb() {
    let config = ServerConfig::default();
    assert_eq!(config.max_body_bytes, 1024 * 1024);
}

/// Verifies default evidence policy redacts raw values.
#[test]
fn default_evidence_policy_redacts() {
    let config = EvidencePolicyConfig::default();
    assert!(!config.allow_raw_values);
    assert!(config.require_provider_opt_in);
}
