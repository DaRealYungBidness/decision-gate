// decision-gate-cli/src/serve_policy.rs
// ============================================================================
// Module: Serve Policy
// Description: Network exposure policy checks for the CLI server launcher.
// Purpose: Enforce safe-by-default bind behavior with explicit opt-in.
// Dependencies: decision-gate-mcp, std
// ============================================================================

//! ## Overview
//! Provides safety checks for binding the MCP server to non-loopback addresses.
//! The policy is fail-closed: explicit opt-in is required, and TLS (or upstream
//! TLS termination) + auth must be configured before network exposure is allowed.
//!
//! Security posture: fail closed on unsafe bind configuration; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::env;
use std::net::SocketAddr;

use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerTlsConfig;
use decision_gate_mcp::config::ServerTlsTermination;
use decision_gate_mcp::config::ServerTransport;

use crate::t;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Environment variable enabling non-loopback server binds.
pub const ALLOW_NON_LOOPBACK_ENV: &str = "DECISION_GATE_ALLOW_NON_LOOPBACK";

// ============================================================================
// SECTION: Types
// ============================================================================

/// Bind outcome metadata for transport warnings.
///
/// # Invariants
/// - `network_exposed` is `true` only when a non-loopback bind is selected.
/// - `bind_addr` is `None` for stdio transports.
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
    /// TLS termination mode for the server.
    pub tls_termination: ServerTlsTermination,
    /// Whether audit logging is enabled.
    pub audit_enabled: bool,
    /// Whether rate limiting is enabled.
    pub rate_limit_enabled: bool,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Serve policy failures for bind safety.
///
/// # Invariants
/// - Variants are stable for CLI error mapping and tests.
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
            } => {
                t!("serve.bind.allow_env_invalid", env = ALLOW_NON_LOOPBACK_ENV, value = value)
            }
            Self::InvalidBind {
                bind,
                error,
            } => {
                t!("serve.bind.parse_failed", bind = bind, error = error)
            }
            Self::NonLoopbackOptInRequired {
                bind,
            } => {
                t!("serve.bind.non_loopback_opt_in", bind = bind, env = ALLOW_NON_LOOPBACK_ENV)
            }
            Self::NonLoopbackAuthRequired {
                bind,
            } => {
                t!("serve.bind.non_loopback_auth_required", bind = bind)
            }
            Self::NonLoopbackTlsRequired {
                bind,
            } => {
                t!("serve.bind.non_loopback_tls_required", bind = bind)
            }
            Self::NonLoopbackMtlsClientCaRequired {
                bind,
            } => {
                t!("serve.bind.non_loopback_mtls_client_ca_required", bind = bind)
            }
            Self::NonLoopbackMtlsClientCertRequired {
                bind,
            } => {
                t!("serve.bind.non_loopback_mtls_client_cert_required", bind = bind)
            }
        };
        write!(f, "{message}")
    }
}

// ============================================================================
// SECTION: Policy
// ============================================================================

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
    let tls_termination = config.server.tls_termination;
    let audit_enabled = config.server.audit.enabled;
    let rate_limit_enabled = config.server.limits.rate_limit.is_some();
    match config.server.transport {
        ServerTransport::Stdio => Ok(BindOutcome {
            transport: ServerTransport::Stdio,
            bind_addr: None,
            network_exposed: false,
            auth_mode,
            tls: config.server.tls.clone(),
            tls_termination,
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
                    tls_termination,
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
            let tls = config.server.tls.as_ref();
            if tls.is_none() && tls_termination != ServerTlsTermination::Upstream {
                return Err(ServePolicyError::NonLoopbackTlsRequired {
                    bind: bind.to_string(),
                });
            }
            if auth_mode == ServerAuthMode::Mtls
                && let Some(tls) = tls
            {
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
                tls_termination,
                audit_enabled,
                rate_limit_enabled,
            })
        }
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

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
pub(crate) fn parse_allow_non_loopback_value(value: &str) -> Result<bool, ServePolicyError> {
    parse_boolish(value).map_or_else(
        || {
            Err(ServePolicyError::InvalidEnv {
                value: value.to_string(),
            })
        },
        Ok,
    )
}
