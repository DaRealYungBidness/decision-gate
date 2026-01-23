// decision-gate-mcp/src/config.rs
// ============================================================================
// Module: MCP Configuration
// Description: Configuration loading and validation for Decision Gate MCP.
// Purpose: Provide strict, fail-closed config parsing with hard limits.
// Dependencies: decision-gate-core, serde, toml
// ============================================================================

//! ## Overview
//! Configuration is loaded from a TOML file with strict size and path limits.
//! Missing or invalid configuration fails closed to preserve security posture.
//! Security posture: config inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;

use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use serde::Deserialize;
use thiserror::Error;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Default configuration filename when no path is specified.
const DEFAULT_CONFIG_NAME: &str = "decision-gate.toml";
/// Environment variable used to override the config path.
const CONFIG_ENV_VAR: &str = "DECISION_GATE_CONFIG";
/// Maximum configuration file size in bytes.
const MAX_CONFIG_FILE_SIZE: usize = 1024 * 1024;
/// Maximum length of a single path component.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;

// ============================================================================
// SECTION: Configuration Types
// ============================================================================

/// Decision Gate MCP configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DecisionGateConfig {
    /// Server configuration.
    #[serde(default)]
    pub server: ServerConfig,
    /// Trust and policy configuration.
    #[serde(default)]
    pub trust: TrustConfig,
    /// Evidence disclosure policy configuration.
    #[serde(default)]
    pub evidence: EvidencePolicyConfig,
    /// Run state store configuration.
    #[serde(default)]
    pub run_state_store: RunStateStoreConfig,
    /// Evidence provider configuration entries.
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
}

impl DecisionGateConfig {
    /// Loads configuration from disk using the default resolution rules.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when loading or validation fails.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let resolved = resolve_path(path)?;
        validate_path(&resolved)?;
        let bytes = fs::read(&resolved).map_err(|err| ConfigError::Io(err.to_string()))?;
        if bytes.len() > MAX_CONFIG_FILE_SIZE {
            return Err(ConfigError::Invalid("config file exceeds size limit".to_string()));
        }
        let content = std::str::from_utf8(&bytes)
            .map_err(|_| ConfigError::Invalid("config file must be utf-8".to_string()))?;
        let mut config: Self =
            toml::from_str(content).map_err(|err| ConfigError::Parse(err.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Validates the configuration for internal consistency.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when configuration is invalid.
    pub fn validate(&mut self) -> Result<(), ConfigError> {
        self.server.validate()?;
        self.run_state_store.validate()?;
        for provider in &self.providers {
            provider.validate()?;
        }
        Ok(())
    }
}

/// Server configuration for MCP transports.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Transport type for MCP.
    #[serde(default)]
    pub transport: ServerTransport,
    /// Bind address for HTTP or SSE transports.
    #[serde(default)]
    pub bind: Option<String>,
    /// Maximum request body size in bytes.
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: ServerTransport::Stdio,
            bind: None,
            max_body_bytes: default_max_body_bytes(),
        }
    }
}

impl ServerConfig {
    /// Validates server transport configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        match self.transport {
            ServerTransport::Http | ServerTransport::Sse => {
                let bind = self.bind.as_deref().unwrap_or_default().trim();
                if bind.is_empty() {
                    return Err(ConfigError::Invalid(
                        "http/sse transport requires bind address".to_string(),
                    ));
                }
                let addr: SocketAddr = bind
                    .parse()
                    .map_err(|_| ConfigError::Invalid("invalid bind address".to_string()))?;
                if !addr.ip().is_loopback() {
                    return Err(ConfigError::Invalid(
                        "non-loopback bind disallowed without auth policy".to_string(),
                    ));
                }
            }
            ServerTransport::Stdio => {}
        }
        Ok(())
    }
}

/// Supported MCP transport types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerTransport {
    /// Use stdin/stdout transport.
    #[default]
    Stdio,
    /// Use HTTP JSON-RPC transport.
    Http,
    /// Use SSE transport for responses.
    Sse,
}

/// Trust configuration for evidence providers.
#[derive(Debug, Clone, Deserialize)]
pub struct TrustConfig {
    /// Default trust policy for providers.
    #[serde(default = "default_trust_policy")]
    pub default_policy: TrustPolicy,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            default_policy: TrustPolicy::Audit,
        }
    }
}

/// Evidence disclosure policy configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct EvidencePolicyConfig {
    /// Allow raw evidence values to be returned via `evidence_query`.
    #[serde(default)]
    pub allow_raw_values: bool,
    /// Require provider opt-in for raw value disclosure.
    #[serde(default = "default_require_provider_opt_in")]
    pub require_provider_opt_in: bool,
}

impl Default for EvidencePolicyConfig {
    fn default() -> Self {
        Self {
            allow_raw_values: false,
            require_provider_opt_in: true,
        }
    }
}

/// Run state store configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RunStateStoreConfig {
    /// Store backend type.
    #[serde(rename = "type", default)]
    pub store_type: RunStateStoreType,
    /// `SQLite` database path when using the sqlite backend.
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// Busy timeout in milliseconds.
    #[serde(default = "default_store_busy_timeout_ms")]
    pub busy_timeout_ms: u64,
    /// `SQLite` journal mode.
    #[serde(default)]
    pub journal_mode: SqliteStoreMode,
    /// `SQLite` synchronous mode.
    #[serde(default)]
    pub sync_mode: SqliteSyncMode,
    /// Optional max versions to retain per run.
    #[serde(default)]
    pub max_versions: Option<u64>,
}

impl Default for RunStateStoreConfig {
    fn default() -> Self {
        Self {
            store_type: RunStateStoreType::default(),
            path: None,
            busy_timeout_ms: default_store_busy_timeout_ms(),
            journal_mode: SqliteStoreMode::default(),
            sync_mode: SqliteSyncMode::default(),
            max_versions: None,
        }
    }
}

impl RunStateStoreConfig {
    /// Validates run state store configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        match self.store_type {
            RunStateStoreType::Memory => {
                if self.path.is_some() {
                    return Err(ConfigError::Invalid(
                        "memory run_state_store must not set path".to_string(),
                    ));
                }
                Ok(())
            }
            RunStateStoreType::Sqlite => {
                let path = self.path.as_ref().ok_or_else(|| {
                    ConfigError::Invalid("sqlite run_state_store requires path".to_string())
                })?;
                validate_store_path(path)?;
                if self.max_versions == Some(0) {
                    return Err(ConfigError::Invalid(
                        "run_state_store max_versions must be greater than zero".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Run state store backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStateStoreType {
    /// Use the in-memory store.
    #[default]
    Memory,
    /// Use `SQLite`-backed durable store.
    Sqlite,
}

/// Provider trust policy configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustPolicy {
    /// Audit mode without signature enforcement.
    Audit,
    /// Require signatures with the provided public keys.
    RequireSignature {
        /// Public key paths or identifiers.
        keys: Vec<String>,
    },
}

/// Provider configuration entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Provider identifier.
    pub name: String,
    /// Provider type.
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    /// Command used to spawn MCP providers (stdio transport).
    #[serde(default)]
    pub command: Vec<String>,
    /// HTTP URL for MCP providers using HTTP transport.
    #[serde(default)]
    pub url: Option<String>,
    /// Allow insecure HTTP for MCP providers.
    #[serde(default)]
    pub allow_insecure_http: bool,
    /// Path to the provider capability contract JSON.
    #[serde(default)]
    pub capabilities_path: Option<PathBuf>,
    /// Optional authentication configuration for the provider.
    #[serde(default)]
    pub auth: Option<ProviderAuthConfig>,
    /// Optional trust override for this provider.
    #[serde(default)]
    pub trust: Option<TrustPolicy>,
    /// Provider opt-in for raw evidence disclosure.
    #[serde(default)]
    pub allow_raw: bool,
    /// Provider-specific configuration blob for built-ins.
    #[serde(default)]
    pub config: Option<toml::Value>,
}

impl ProviderConfig {
    /// Validates provider configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.name.trim().is_empty() {
            return Err(ConfigError::Invalid("provider name is empty".to_string()));
        }
        match self.provider_type {
            ProviderType::Builtin => {
                if self.capabilities_path.is_some() {
                    return Err(ConfigError::Invalid(
                        "builtin provider does not accept capabilities_path".to_string(),
                    ));
                }
                Ok(())
            }
            ProviderType::Mcp => {
                if self.command.is_empty() && self.url.as_deref().unwrap_or_default().is_empty() {
                    return Err(ConfigError::Invalid(
                        "mcp provider requires command or url".to_string(),
                    ));
                }
                if self.capabilities_path.is_none() {
                    return Err(ConfigError::Invalid(
                        "mcp provider requires capabilities_path".to_string(),
                    ));
                }
                if let Some(url) = &self.url
                    && url.starts_with("http://")
                    && !self.allow_insecure_http
                {
                    return Err(ConfigError::Invalid(
                        "insecure http requires allow_insecure_http".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Provider type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// Built-in provider.
    Builtin,
    /// External MCP provider.
    Mcp,
}

/// Provider authentication configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderAuthConfig {
    /// Bearer token for provider authentication.
    #[serde(default)]
    pub bearer_token: Option<String>,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Configuration loading or validation errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// I/O failure while reading configuration.
    #[error("config io error: {0}")]
    Io(String),
    /// TOML parsing error.
    #[error("config parse error: {0}")]
    Parse(String),
    /// Invalid configuration data.
    #[error("invalid config: {0}")]
    Invalid(String),
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Resolves the config path from CLI or environment defaults.
fn resolve_path(path: Option<&Path>) -> Result<PathBuf, ConfigError> {
    if let Some(path) = path {
        return Ok(path.to_path_buf());
    }
    if let Ok(env_path) = env::var(CONFIG_ENV_VAR) {
        if env_path.len() > MAX_TOTAL_PATH_LENGTH {
            return Err(ConfigError::Invalid("config path exceeds max length".to_string()));
        }
        return Ok(PathBuf::from(env_path));
    }
    Ok(PathBuf::from(DEFAULT_CONFIG_NAME))
}

/// Validates the resolved path against security limits.
fn validate_path(path: &Path) -> Result<(), ConfigError> {
    let text = path.to_string_lossy();
    if text.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ConfigError::Invalid("config path exceeds max length".to_string()));
    }
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if value.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(ConfigError::Invalid("config path component too long".to_string()));
        }
    }
    Ok(())
}

/// Default maximum request body size in bytes.
const fn default_max_body_bytes() -> usize {
    1024 * 1024
}

/// Default value for requiring provider opt-in to raw evidence.
const fn default_require_provider_opt_in() -> bool {
    true
}

/// Default busy timeout for the `SQLite` store (ms).
const fn default_store_busy_timeout_ms() -> u64 {
    5_000
}

/// Default trust policy for providers.
const fn default_trust_policy() -> TrustPolicy {
    TrustPolicy::Audit
}

/// Validates run state store paths against security limits.
fn validate_store_path(path: &Path) -> Result<(), ConfigError> {
    let text = path.to_string_lossy();
    if text.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ConfigError::Invalid("run_state_store path exceeds max length".to_string()));
    }
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if value.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(ConfigError::Invalid(
                "run_state_store path component too long".to_string(),
            ));
        }
    }
    Ok(())
}
