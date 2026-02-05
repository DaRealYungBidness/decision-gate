//! Server config validation tests for decision-gate-config.
// crates/decision-gate-config/tests/server_validation.rs
// =============================================================================
// Module: Server Config Validation Tests
// Description: Validate server transport, auth, TLS, and rate-limit constraints.
// Purpose: Ensure MCP server settings fail closed and enforce limits.
// =============================================================================

use decision_gate_config::ConfigError;
use decision_gate_config::RateLimitConfig;
use decision_gate_config::ServerAuditConfig;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerAuthMode;
use decision_gate_config::ServerLimitsConfig;
use decision_gate_config::ServerTlsConfig;
use decision_gate_config::ServerTransport;

mod common;

type TestResult = Result<(), String>;

fn assert_invalid(result: Result<(), ConfigError>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            let message = error.to_string();
            if message.contains(needle) {
                Ok(())
            } else {
                Err(format!("error {message} did not contain {needle}"))
            }
        }
        Ok(()) => Err("expected invalid config".to_string()),
    }
}

#[test]
fn http_transport_requires_bind() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = None;
    assert_invalid(config.validate(), "http/sse transport requires bind address")?;
    Ok(())
}

#[test]
fn http_transport_rejects_non_loopback_without_auth() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("0.0.0.0:8080".to_string());
    config.server.auth = None;
    assert_invalid(config.validate(), "non-loopback bind disallowed without auth policy")?;
    Ok(())
}

#[test]
fn stdio_transport_rejects_non_local_auth() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Stdio;
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    assert_invalid(config.validate(), "stdio transport only supports local_only auth")?;
    Ok(())
}

#[test]
fn stdio_transport_rejects_tls() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Stdio;
    config.server.tls = Some(ServerTlsConfig {
        cert_path: "server.pem".to_string(),
        key_path: "server.key".to_string(),
        client_ca_path: None,
        require_client_cert: true,
    });
    assert_invalid(config.validate(), "stdio transport does not support tls")?;
    Ok(())
}

#[test]
fn tls_rejects_empty_paths() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.tls = Some(ServerTlsConfig {
        cert_path: "   ".to_string(),
        key_path: String::new(),
        client_ca_path: None,
        require_client_cert: true,
    });
    assert_invalid(config.validate(), "tls.cert_path must be non-empty")?;
    Ok(())
}

#[test]
fn audit_rejects_empty_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some("  ".to_string()),
        log_precheck_payloads: false,
    };
    assert_invalid(config.validate(), "audit.path must be non-empty")?;
    Ok(())
}

#[test]
fn auth_bearer_requires_tokens() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    assert_invalid(config.validate(), "bearer_token auth requires bearer_tokens")?;
    Ok(())
}

#[test]
fn auth_rejects_unknown_tool_in_allowlist() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: vec!["not_a_tool".to_string()],
        principals: Vec::new(),
    });
    assert_invalid(config.validate(), "unknown tool in allowlist")?;
    Ok(())
}

#[test]
fn auth_rejects_token_with_whitespace() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![" bad ".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    assert_invalid(config.validate(), "auth token must not contain whitespace")?;
    Ok(())
}

#[test]
fn rate_limit_rejects_out_of_range_values() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.limits = ServerLimitsConfig {
        max_inflight: 1,
        rate_limit: Some(RateLimitConfig {
            max_requests: 0,
            window_ms: 50,
            max_entries: 0,
        }),
    };
    assert_invalid(config.validate(), "rate_limit max_requests must be greater than zero")?;
    Ok(())
}
