// decision-gate-cli/src/serve_policy.rs
// ============================================================================
// Module: Serve Policy
// Description: Network exposure policy checks for the CLI server launcher.
// Purpose: Enforce safe-by-default bind behavior with explicit opt-in.
// Dependencies: decision-gate-mcp, std
// ============================================================================

//! ## Overview
//! Provides safety checks for binding the MCP server to non-loopback addresses.
//! The policy is fail-closed: explicit opt-in is required, and TLS + auth must
//! be configured before network exposure is allowed.

use std::env;
use std::net::SocketAddr;

use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerTlsConfig;
use decision_gate_mcp::config::ServerTransport;

use crate::t;

/// Environment variable enabling non-loopback server binds.
pub const ALLOW_NON_LOOPBACK_ENV: &str = "DECISION_GATE_ALLOW_NON_LOOPBACK";

/// Bind outcome metadata for transport warnings.
#[derive(Debug, Clone)]
pub struct BindOutcome {
    /// Selected transport.
    pub transport: ServerTransport,
    /// Bound socket address for HTTP/SSE transports.
    pub bind_addr: Option<SocketAddr>,
    /// True when the server is bound to a non-loopback address.
    pub network_exposed: bool,
    /// Effective auth mode.
    pub auth_mode: ServerAuthMode,
    /// TLS configuration when present.
    pub tls: Option<ServerTlsConfig>,
    /// Whether audit logging is enabled.
    pub audit_enabled: bool,
    /// Whether rate limiting is enabled.
    pub rate_limit_enabled: bool,
}

/// Serve policy failures for bind safety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServePolicyError {
    /// Environment variable was set to an invalid value.
    InvalidEnv {
        /// Raw environment value.
        value: String,
    },
    /// Bind string failed to parse.
    InvalidBind {
        /// Raw bind value.
        bind: String,
        /// Parse error message.
        error: String,
    },
    /// Non-loopback binding requires explicit opt-in.
    NonLoopbackOptInRequired {
        /// Bind address.
        bind: String,
    },
    /// Non-loopback binding requires auth.
    NonLoopbackAuthRequired {
        /// Bind address.
        bind: String,
    },
    /// Non-loopback binding requires TLS.
    NonLoopbackTlsRequired {
        /// Bind address.
        bind: String,
    },
    /// mTLS requires a client CA bundle.
    NonLoopbackMtlsClientCaRequired {
        /// Bind address.
        bind: String,
    },
    /// mTLS requires client cert enforcement.
    NonLoopbackMtlsClientCertRequired {
        /// Bind address.
        bind: String,
    },
}

impl std::fmt::Display for ServePolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidEnv {
                value,
            } => t!("serve.bind.allow_env_invalid", env = ALLOW_NON_LOOPBACK_ENV, value = value),
            Self::InvalidBind {
                bind,
                error,
            } => t!("serve.bind.parse_failed", bind = bind, error = error),
            Self::NonLoopbackOptInRequired {
                bind,
            } => t!("serve.bind.non_loopback_opt_in", bind = bind, env = ALLOW_NON_LOOPBACK_ENV),
            Self::NonLoopbackAuthRequired {
                bind,
            } => t!("serve.bind.non_loopback_auth_required", bind = bind),
            Self::NonLoopbackTlsRequired {
                bind,
            } => t!("serve.bind.non_loopback_tls_required", bind = bind),
            Self::NonLoopbackMtlsClientCaRequired {
                bind,
            } => t!("serve.bind.non_loopback_mtls_client_ca_required", bind = bind),
            Self::NonLoopbackMtlsClientCertRequired {
                bind,
            } => t!("serve.bind.non_loopback_mtls_client_cert_required", bind = bind),
        };
        write!(f, "{message}")
    }
}

/// Resolves the non-loopback opt-in flag from CLI and environment.
///
/// # Errors
/// Returns [`ServePolicyError::InvalidEnv`] when the environment value is invalid.
pub fn resolve_allow_non_loopback(flag: bool) -> Result<bool, ServePolicyError> {
    if flag {
        return Ok(true);
    }
    let Some(value) = env::var_os(ALLOW_NON_LOOPBACK_ENV) else {
        return Ok(false);
    };
    let value = value.to_string_lossy().to_string();
    parse_allow_non_loopback_value(&value)
}

/// Enforces local-only transport restrictions for the MCP server.
///
/// # Errors
/// Returns [`ServePolicyError`] when configuration violates security requirements.
pub fn enforce_local_only(
    config: &DecisionGateConfig,
    allow_non_loopback: bool,
) -> Result<BindOutcome, ServePolicyError> {
    let auth_mode = config.server.auth.as_ref().map_or(ServerAuthMode::LocalOnly, |auth| auth.mode);
    let audit_enabled = config.server.audit.enabled;
    let rate_limit_enabled = config.server.limits.rate_limit.is_some();
    match config.server.transport {
        ServerTransport::Stdio => Ok(BindOutcome {
            transport: ServerTransport::Stdio,
            bind_addr: None,
            network_exposed: false,
            auth_mode,
            tls: config.server.tls.clone(),
            audit_enabled,
            rate_limit_enabled,
        }),
        ServerTransport::Http | ServerTransport::Sse => {
            let bind = config.server.bind.as_deref().unwrap_or_default();
            let addr: SocketAddr = bind.parse().map_err(|err: std::net::AddrParseError| {
                ServePolicyError::InvalidBind {
                    bind: bind.to_string(),
                    error: err.to_string(),
                }
            })?;
            if addr.ip().is_loopback() {
                return Ok(BindOutcome {
                    transport: config.server.transport,
                    bind_addr: Some(addr),
                    network_exposed: false,
                    auth_mode,
                    tls: config.server.tls.clone(),
                    audit_enabled,
                    rate_limit_enabled,
                });
            }
            if !allow_non_loopback {
                return Err(ServePolicyError::NonLoopbackOptInRequired {
                    bind: bind.to_string(),
                });
            }
            if auth_mode == ServerAuthMode::LocalOnly {
                return Err(ServePolicyError::NonLoopbackAuthRequired {
                    bind: bind.to_string(),
                });
            }
            let tls = config.server.tls.as_ref().ok_or_else(|| {
                ServePolicyError::NonLoopbackTlsRequired {
                    bind: bind.to_string(),
                }
            })?;
            if auth_mode == ServerAuthMode::Mtls {
                let missing_client_ca =
                    tls.client_ca_path.as_ref().is_none_or(|value| value.trim().is_empty());
                if missing_client_ca {
                    return Err(ServePolicyError::NonLoopbackMtlsClientCaRequired {
                        bind: bind.to_string(),
                    });
                }
                if !tls.require_client_cert {
                    return Err(ServePolicyError::NonLoopbackMtlsClientCertRequired {
                        bind: bind.to_string(),
                    });
                }
            }
            Ok(BindOutcome {
                transport: config.server.transport,
                bind_addr: Some(addr),
                network_exposed: true,
                auth_mode,
                tls: config.server.tls.clone(),
                audit_enabled,
                rate_limit_enabled,
            })
        }
    }
}

/// Parses a bool-ish string (true/false/1/0/yes/no/on/off).
fn parse_boolish(value: &str) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "y" | "on" => Some(true),
        "0" | "false" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

/// Parses an env value for allow-non-loopback.
fn parse_allow_non_loopback_value(value: &str) -> Result<bool, ServePolicyError> {
    parse_boolish(value).map_or_else(
        || {
            Err(ServePolicyError::InvalidEnv {
                value: value.to_string(),
            })
        },
        Ok,
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "Test helpers use expect/expect_err for concise failure messages."
    )]
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use decision_gate_mcp::DecisionGateConfig;

    use super::ServePolicyError;
    use super::enforce_local_only;
    use super::parse_allow_non_loopback_value;

    fn write_config(contents: &str) -> PathBuf {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("time").as_nanos();
        let path = std::env::temp_dir().join(format!("dg-cli-test-{timestamp}.toml"));
        fs::write(&path, contents).expect("write config");
        path
    }

    fn load_config(contents: &str) -> DecisionGateConfig {
        let path = write_config(contents);
        let config = DecisionGateConfig::load(Some(&path)).expect("load config");
        let _ = fs::remove_file(path);
        config
    }


    #[test]
    fn non_loopback_requires_opt_in() {
        let config = load_config(
            r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
        );
        let err = enforce_local_only(&config, false).expect_err("expected opt-in error");
        assert!(matches!(err, ServePolicyError::NonLoopbackOptInRequired { .. }));
    }

    #[test]
    fn non_loopback_requires_auth() {
        let mut config = load_config(
            r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
        );
        config.server.auth = None;
        let err = enforce_local_only(&config, true).expect_err("expected auth error");
        assert!(matches!(err, ServePolicyError::NonLoopbackAuthRequired { .. }));
    }

    #[test]
    fn non_loopback_requires_tls() {
        let config = load_config(
            r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]
"#,
        );
        let err = enforce_local_only(&config, true).expect_err("expected tls error");
        assert!(matches!(err, ServePolicyError::NonLoopbackTlsRequired { .. }));
    }

    #[test]
    fn mtls_requires_client_ca() {
        let config = load_config(
            r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "mtls"
mtls_subjects = ["CN=test"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
require_client_cert = true
"#,
        );
        let err = enforce_local_only(&config, true).expect_err("expected mtls CA error");
        assert!(matches!(err, ServePolicyError::NonLoopbackMtlsClientCaRequired { .. }));
    }

    #[test]
    fn mtls_requires_client_cert_flag() {
        let config = load_config(
            r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "mtls"
mtls_subjects = ["CN=test"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
client_ca_path = "ca.pem"
require_client_cert = false
"#,
        );
        let err = enforce_local_only(&config, true).expect_err("expected mtls cert error");
        assert!(matches!(err, ServePolicyError::NonLoopbackMtlsClientCertRequired { .. }));
    }

    #[test]
    fn non_loopback_allows_bearer_with_tls() {
        let config = load_config(
            r#"
[server]
transport = "http"
bind = "0.0.0.0:8080"

[server.auth]
mode = "bearer_token"
bearer_tokens = ["token"]

[server.tls]
cert_path = "cert.pem"
key_path = "key.pem"
"#,
        );
        let outcome = enforce_local_only(&config, true).expect("expected success");
        assert!(outcome.network_exposed);
    }

    #[test]
    fn parse_allow_non_loopback_accepts_true() {
        let result = parse_allow_non_loopback_value("true").expect("parse env");
        assert!(result);
    }

    #[test]
    fn parse_allow_non_loopback_rejects_invalid() {
        let err = parse_allow_non_loopback_value("maybe").expect_err("expected invalid env");
        assert!(matches!(err, ServePolicyError::InvalidEnv { .. }));
    }
}
