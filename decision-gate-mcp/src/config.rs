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

use decision_gate_contract::ToolName;
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
/// Maximum number of server auth tokens.
const MAX_AUTH_TOKENS: usize = 64;
/// Maximum length of a server auth token.
const MAX_AUTH_TOKEN_LENGTH: usize = 256;
/// Maximum number of allowed tool entries in auth config.
const MAX_AUTH_TOOL_RULES: usize = 128;
/// Maximum length of an mTLS subject string.
const MAX_AUTH_SUBJECT_LENGTH: usize = 512;
/// Minimum MCP provider connect timeout in milliseconds.
const MIN_PROVIDER_CONNECT_TIMEOUT_MS: u64 = 100;
/// Maximum MCP provider connect timeout in milliseconds.
const MAX_PROVIDER_CONNECT_TIMEOUT_MS: u64 = 10_000;
/// Minimum MCP provider request timeout in milliseconds.
const MIN_PROVIDER_REQUEST_TIMEOUT_MS: u64 = 500;
/// Maximum MCP provider request timeout in milliseconds.
const MAX_PROVIDER_REQUEST_TIMEOUT_MS: u64 = 30_000;

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
    /// Optional authentication configuration for inbound tool calls.
    #[serde(default)]
    pub auth: Option<ServerAuthConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: ServerTransport::Stdio,
            bind: None,
            max_body_bytes: default_max_body_bytes(),
            auth: None,
        }
    }
}

impl ServerConfig {
    /// Validates server transport configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.max_body_bytes == 0 {
            return Err(ConfigError::Invalid(
                "max_body_bytes must be greater than zero".to_string(),
            ));
        }
        if let Some(auth) = &self.auth {
            auth.validate()?;
        }
        let auth_mode =
            self.auth.as_ref().map(|auth| auth.mode).unwrap_or(ServerAuthMode::LocalOnly);
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
                if !addr.ip().is_loopback() && auth_mode == ServerAuthMode::LocalOnly {
                    return Err(ConfigError::Invalid(
                        "non-loopback bind disallowed without auth policy".to_string(),
                    ));
                }
            }
            ServerTransport::Stdio => {
                if auth_mode != ServerAuthMode::LocalOnly {
                    return Err(ConfigError::Invalid(
                        "stdio transport only supports local_only auth".to_string(),
                    ));
                }
            }
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

/// Inbound auth modes for MCP server tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerAuthMode {
    /// Local-only loopback or stdio access.
    #[default]
    LocalOnly,
    /// Bearer token authentication.
    BearerToken,
    /// mTLS subject allowlist via trusted proxy headers.
    Mtls,
}

/// Server authentication configuration for inbound tool calls.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerAuthConfig {
    /// Auth mode for inbound MCP tool calls.
    #[serde(default)]
    pub mode: ServerAuthMode,
    /// Accepted bearer tokens (required for bearer_token mode).
    #[serde(default)]
    pub bearer_tokens: Vec<String>,
    /// Allowed mTLS subjects (required for mtls mode).
    #[serde(default)]
    pub mtls_subjects: Vec<String>,
    /// Optional tool allowlist (per-tool authorization).
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

impl ServerAuthConfig {
    /// Validates auth configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.bearer_tokens.len() > MAX_AUTH_TOKENS {
            return Err(ConfigError::Invalid("too many auth tokens".to_string()));
        }
        for token in &self.bearer_tokens {
            if token.trim().is_empty() {
                return Err(ConfigError::Invalid("auth token must be non-empty".to_string()));
            }
            if token.len() > MAX_AUTH_TOKEN_LENGTH {
                return Err(ConfigError::Invalid("auth token too long".to_string()));
            }
            if token.trim() != token {
                return Err(ConfigError::Invalid(
                    "auth token must not contain whitespace".to_string(),
                ));
            }
        }
        if self.mtls_subjects.len() > MAX_AUTH_TOKENS {
            return Err(ConfigError::Invalid("too many mTLS subjects".to_string()));
        }
        for subject in &self.mtls_subjects {
            if subject.trim().is_empty() {
                return Err(ConfigError::Invalid("mTLS subject must be non-empty".to_string()));
            }
            if subject.len() > MAX_AUTH_SUBJECT_LENGTH {
                return Err(ConfigError::Invalid("mTLS subject too long".to_string()));
            }
        }
        if self.allowed_tools.len() > MAX_AUTH_TOOL_RULES {
            return Err(ConfigError::Invalid("too many tool allowlist entries".to_string()));
        }
        for tool_name in &self.allowed_tools {
            if ToolName::parse(tool_name).is_none() {
                return Err(ConfigError::Invalid(format!(
                    "unknown tool in allowlist: {tool_name}"
                )));
            }
        }
        match self.mode {
            ServerAuthMode::LocalOnly => Ok(()),
            ServerAuthMode::BearerToken => {
                if self.bearer_tokens.is_empty() {
                    return Err(ConfigError::Invalid(
                        "bearer_token auth requires bearer_tokens".to_string(),
                    ));
                }
                Ok(())
            }
            ServerAuthMode::Mtls => {
                if self.mtls_subjects.is_empty() {
                    return Err(ConfigError::Invalid(
                        "mtls auth requires mtls_subjects".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
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

/// Timeout configuration for MCP provider HTTP requests.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderTimeoutConfig {
    /// Maximum time to establish the HTTP connection.
    #[serde(default = "default_provider_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// Maximum end-to-end request time (connect + body).
    #[serde(default = "default_provider_request_timeout_ms")]
    pub request_timeout_ms: u64,
}

impl Default for ProviderTimeoutConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: default_provider_connect_timeout_ms(),
            request_timeout_ms: default_provider_request_timeout_ms(),
        }
    }
}

impl ProviderTimeoutConfig {
    /// Validates provider timeout configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        validate_timeout_range(
            "providers.timeouts.connect_timeout_ms",
            self.connect_timeout_ms,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        )?;
        validate_timeout_range(
            "providers.timeouts.request_timeout_ms",
            self.request_timeout_ms,
            MIN_PROVIDER_REQUEST_TIMEOUT_MS,
            MAX_PROVIDER_REQUEST_TIMEOUT_MS,
        )?;
        if self.request_timeout_ms < self.connect_timeout_ms {
            return Err(ConfigError::Invalid(
                "providers.timeouts.request_timeout_ms must be >= connect_timeout_ms".to_string(),
            ));
        }
        Ok(())
    }
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
    /// Provider timeout overrides (HTTP MCP providers).
    #[serde(default)]
    pub timeouts: ProviderTimeoutConfig,
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
        self.timeouts.validate()?;
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

/// Default MCP provider connect timeout in milliseconds.
const fn default_provider_connect_timeout_ms() -> u64 {
    2_000
}

/// Default MCP provider request timeout in milliseconds.
const fn default_provider_request_timeout_ms() -> u64 {
    10_000
}

/// Validates a timeout value against bounds.
fn validate_timeout_range(
    field: &str,
    value_ms: u64,
    min_ms: u64,
    max_ms: u64,
) -> Result<(), ConfigError> {
    if value_ms < min_ms || value_ms > max_ms {
        return Err(ConfigError::Invalid(format!(
            "{field} must be between {min_ms} and {max_ms} milliseconds",
        )));
    }
    Ok(())
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
