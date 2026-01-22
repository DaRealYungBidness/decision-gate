// system-tests/src/config/env.rs
// ============================================================================
// Module: System Test Environment
// Description: Environment-backed configuration for system tests.
// Purpose: Centralize env parsing with strict UTF-8 validation.
// Dependencies: std
// ============================================================================

//! ## Overview
//! Environment values are parsed with strict UTF-8 enforcement to avoid silent
//! misconfiguration. Invalid UTF-8 fails closed.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// SECTION: Environment Constants
// ============================================================================

/// Environment keys for system test configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTestEnv {
    /// Optional run root override.
    RunRoot,
    /// Optional HTTP bind override for MCP server.
    HttpBind,
    /// Optional override for external MCP provider URL.
    ProviderUrl,
    /// Optional timeout override (seconds).
    TimeoutSeconds,
}

impl SystemTestEnv {
    /// Returns the canonical environment variable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RunRoot => "DECISION_GATE_SYSTEM_TEST_RUN_ROOT",
            Self::HttpBind => "DECISION_GATE_SYSTEM_TEST_HTTP_BIND",
            Self::ProviderUrl => "DECISION_GATE_SYSTEM_TEST_PROVIDER_URL",
            Self::TimeoutSeconds => "DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC",
        }
    }
}

// ============================================================================
// SECTION: Config Types
// ============================================================================

/// Typed system test configuration derived from environment variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemTestConfig {
    /// Optional run root override.
    pub run_root: Option<PathBuf>,
    /// Optional HTTP bind override.
    pub http_bind: Option<String>,
    /// Optional external provider URL override.
    pub provider_url: Option<String>,
    /// Optional timeout override.
    pub timeout: Option<Duration>,
}

impl Default for SystemTestConfig {
    fn default() -> Self {
        Self {
            run_root: None,
            http_bind: None,
            provider_url: None,
            timeout: None,
        }
    }
}

impl SystemTestConfig {
    /// Loads configuration from environment variables.
    #[must_use]
    pub fn load() -> Self {
        let run_root = read_env_strict(SystemTestEnv::RunRoot.as_str()).map(PathBuf::from);
        let http_bind = read_env_strict(SystemTestEnv::HttpBind.as_str());
        let provider_url = read_env_strict(SystemTestEnv::ProviderUrl.as_str());
        let timeout = read_env_strict(SystemTestEnv::TimeoutSeconds.as_str())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        Self {
            run_root,
            http_bind,
            provider_url,
            timeout,
        }
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Reads an environment variable and enforces UTF-8 validity.
///
/// # Panics
///
/// Panics when the environment variable contains invalid UTF-8.
#[must_use]
pub fn read_env_strict(name: &str) -> Option<String> {
    if let Some(raw) = std::env::var_os(name) {
        match raw.into_string() {
            Ok(value) => Some(value),
            Err(_) => panic!("{name} must be valid UTF-8"),
        }
    } else {
        None
    }
}
