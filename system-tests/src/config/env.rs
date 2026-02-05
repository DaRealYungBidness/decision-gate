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
    /// Optional timeout override in seconds (positive integer).
    TimeoutSeconds,
    /// Allow reusing an existing run root (`true`/`false` or `1`/`0`).
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
    /// Optional timeout override in seconds (positive integer).
    pub timeout: Option<Duration>,
    /// Allow reusing an existing run root (`true`/`false` or `1`/`0`).
    pub allow_overwrite: bool,
}

impl SystemTestConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error when an environment value is not valid UTF-8, is empty,
    /// or fails validation (for example, an invalid timeout or boolean value).
    pub fn load() -> Result<Self, String> {
        let run_root = read_env_nonempty(SystemTestEnv::RunRoot.as_str())?.map(PathBuf::from);
        let http_bind = read_env_nonempty(SystemTestEnv::HttpBind.as_str())?;
        let provider_url = read_env_nonempty(SystemTestEnv::ProviderUrl.as_str())?;
        let timeout = read_env_nonempty(SystemTestEnv::TimeoutSeconds.as_str())?
            .map(|value| parse_timeout_seconds(SystemTestEnv::TimeoutSeconds.as_str(), &value))
            .transpose()?;
        let allow_overwrite = parse_bool_env(
            SystemTestEnv::AllowOverwrite.as_str(),
            read_env_nonempty(SystemTestEnv::AllowOverwrite.as_str())?,
        )?;
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

/// Reads an environment variable and rejects empty values.
///
/// # Errors
///
/// Returns an error when the variable is set but empty or whitespace.
fn read_env_nonempty(name: &str) -> Result<Option<String>, String> {
    match read_env_strict(name)? {
        Some(value) if value.trim().is_empty() => Err(format!("{name} must not be empty")),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

/// Parses a positive timeout value from an environment variable string.
///
/// # Errors
///
/// Returns an error when the value is missing, non-numeric, or zero.
fn parse_timeout_seconds(name: &str, raw: &str) -> Result<Duration, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{name} must be a positive integer number of seconds"));
    }
    let secs: u64 = trimmed
        .parse()
        .map_err(|_| format!("{name} must be a positive integer number of seconds"))?;
    if secs == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(Duration::from_secs(secs))
}

/// Parses a boolean environment variable with permissive defaults.
///
/// # Errors
///
/// Returns an error when the value is not a recognized boolean literal.
fn parse_bool_env(name: &str, raw: Option<String>) -> Result<bool, String> {
    let Some(value) = raw else {
        return Ok(false);
    };
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") || trimmed == "1" {
        return Ok(true);
    }
    if trimmed.eq_ignore_ascii_case("false") || trimmed == "0" {
        return Ok(false);
    }
    Err(format!("{name} must be 1, 0, true, or false"))
}
