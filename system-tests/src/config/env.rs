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
    /// Allow reusing an existing run root.
    AllowOverwrite,
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
            Self::AllowOverwrite => "DECISION_GATE_SYSTEM_TEST_ALLOW_OVERWRITE",
        }
    }
}

// ============================================================================
// SECTION: Config Types
// ============================================================================

/// Typed system test configuration derived from environment variables.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SystemTestConfig {
    /// Optional run root override.
    pub run_root: Option<PathBuf>,
    /// Optional HTTP bind override.
    pub http_bind: Option<String>,
    /// Optional external provider URL override.
    pub provider_url: Option<String>,
    /// Optional timeout override.
    pub timeout: Option<Duration>,
    /// Allow reusing an existing run root.
    pub allow_overwrite: bool,
}

impl SystemTestConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error when an environment value is not valid UTF-8.
    pub fn load() -> Result<Self, String> {
        let run_root = read_env_strict(SystemTestEnv::RunRoot.as_str())?.map(PathBuf::from);
        let http_bind = read_env_strict(SystemTestEnv::HttpBind.as_str())?;
        let provider_url = read_env_strict(SystemTestEnv::ProviderUrl.as_str())?;
        let timeout = read_env_strict(SystemTestEnv::TimeoutSeconds.as_str())?
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        let allow_overwrite = read_env_strict(SystemTestEnv::AllowOverwrite.as_str())?
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
        Ok(Self {
            run_root,
            http_bind,
            provider_url,
            timeout,
            allow_overwrite,
        })
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Reads an environment variable and enforces UTF-8 validity.
///
/// # Errors
///
/// Returns an error when the environment variable contains invalid UTF-8.
pub fn read_env_strict(name: &str) -> Result<Option<String>, String> {
    std::env::var_os(name).map_or(Ok(None), |raw| {
        raw.into_string().map(Some).map_err(|_| format!("{name} must be valid UTF-8"))
    })
}
