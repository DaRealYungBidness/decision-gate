// decision-gate-config/src/config.rs
// ============================================================================
// Module: Decision Gate Configuration
// Description: Configuration loading and validation for Decision Gate.
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
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use decision_gate_core::AnchorRequirement;
use decision_gate_core::EvidenceAnchorPolicy;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderAnchorPolicy;
use decision_gate_core::ProviderId;
use decision_gate_core::TenantId;
use decision_gate_core::ToolName;
use decision_gate_core::TrustLane;
use decision_gate_core::TrustRequirement;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::policy::DispatchPolicy;
use crate::policy::PolicyEngine;
use crate::policy::StaticPolicyConfig;
// ============================================================================
// SECTION: Constants
// ============================================================================

/// Default configuration filename when no path is specified.
const DEFAULT_CONFIG_NAME: &str = "decision-gate.toml";
/// Environment variable used to override the config path.
pub(crate) const CONFIG_ENV_VAR: &str = "DECISION_GATE_CONFIG";
/// Maximum configuration file size in bytes.
pub(crate) const MAX_CONFIG_FILE_SIZE: usize = 1024 * 1024;
/// Maximum length of a single path component.
pub(crate) const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length.
pub(crate) const MAX_TOTAL_PATH_LENGTH: usize = 4096;
/// Maximum number of server auth tokens.
pub(crate) const MAX_AUTH_TOKENS: usize = 64;
/// Maximum length of a server auth token.
pub(crate) const MAX_AUTH_TOKEN_LENGTH: usize = 256;
/// Maximum number of allowed tool entries in auth config.
pub(crate) const MAX_AUTH_TOOL_RULES: usize = 128;
/// Maximum length of an mTLS subject string.
pub(crate) const MAX_AUTH_SUBJECT_LENGTH: usize = 512;
/// Maximum number of principal role bindings.
pub(crate) const MAX_PRINCIPAL_ROLES: usize = 128;
/// Maximum number of tool visibility entries.
pub(crate) const MAX_TOOL_VISIBILITY_RULES: usize = 128;
/// Maximum number of registry ACL rules.
pub(crate) const MAX_REGISTRY_ACL_RULES: usize = 256;
/// Default maximum inflight requests for MCP servers.
pub(crate) const DEFAULT_MAX_INFLIGHT: usize = 256;
/// Minimum allowed rate limit window in milliseconds.
pub(crate) const MIN_RATE_LIMIT_WINDOW_MS: u64 = 100;
/// Maximum allowed rate limit window in milliseconds.
pub(crate) const MAX_RATE_LIMIT_WINDOW_MS: u64 = 60_000;
/// Maximum allowed requests per rate limit window.
pub(crate) const MAX_RATE_LIMIT_REQUESTS: u32 = 100_000;
/// Maximum number of tracked rate limit entries.
pub(crate) const MAX_RATE_LIMIT_ENTRIES: usize = 65_536;
/// Default max requests per window when rate limiting is enabled.
pub(crate) const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u32 = 1_000;
/// Default rate limit window in milliseconds when enabled.
pub(crate) const DEFAULT_RATE_LIMIT_WINDOW_MS: u64 = 1_000;
/// Default max tracked rate limit entries when enabled.
pub(crate) const DEFAULT_RATE_LIMIT_MAX_ENTRIES: usize = 4_096;
/// Minimum MCP provider connect timeout in milliseconds.
pub(crate) const MIN_PROVIDER_CONNECT_TIMEOUT_MS: u64 = 100;
/// Maximum MCP provider connect timeout in milliseconds.
pub(crate) const MAX_PROVIDER_CONNECT_TIMEOUT_MS: u64 = 10_000;
/// Minimum MCP provider request timeout in milliseconds.
pub(crate) const MIN_PROVIDER_REQUEST_TIMEOUT_MS: u64 = 500;
/// Maximum MCP provider request timeout in milliseconds.
pub(crate) const MAX_PROVIDER_REQUEST_TIMEOUT_MS: u64 = 30_000;
/// Default max schema size accepted by registry (bytes).
pub(crate) const DEFAULT_SCHEMA_MAX_BYTES: usize = 1024 * 1024;
/// Maximum allowed schema size in bytes.
pub(crate) const MAX_SCHEMA_MAX_BYTES: usize = 10 * 1024 * 1024;
/// Default namespace authority connect timeout in milliseconds.
pub(crate) const DEFAULT_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS: u64 = 500;
/// Default namespace authority request timeout in milliseconds.
pub(crate) const DEFAULT_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS: u64 = 2_000;
/// Minimum namespace authority connect timeout in milliseconds.
pub(crate) const MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS: u64 = 100;
/// Maximum namespace authority connect timeout in milliseconds.
pub(crate) const MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS: u64 = 10_000;
/// Minimum namespace authority request timeout in milliseconds.
pub(crate) const MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS: u64 = 500;
/// Maximum namespace authority request timeout in milliseconds.
pub(crate) const MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS: u64 = 30_000;
/// Default maximum provider discovery response size in bytes.
pub(crate) const DEFAULT_PROVIDER_DISCOVERY_MAX_BYTES: usize = 1024 * 1024;
/// Default maximum size for a single docs entry in bytes.
pub(crate) const DEFAULT_DOC_MAX_BYTES: usize = 256 * 1024;
/// Default maximum total docs bytes for the catalog.
pub(crate) const DEFAULT_DOC_MAX_TOTAL_BYTES: usize = 1024 * 1024;
/// Default maximum number of docs in the catalog.
pub(crate) const DEFAULT_DOC_MAX_DOCS: usize = 32;
/// Default maximum sections returned by docs search.
pub(crate) const DEFAULT_DOC_MAX_SECTIONS: u32 = 10;
/// Maximum allowed size for a single docs entry in bytes.
pub(crate) const MAX_DOC_MAX_BYTES: usize = 1024 * 1024;
/// Maximum allowed total docs bytes for the catalog.
pub(crate) const MAX_DOC_MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
/// Maximum allowed number of docs in the catalog.
pub(crate) const MAX_DOC_MAX_DOCS: usize = 256;
/// Maximum allowed docs search sections.
pub(crate) const MAX_DOC_MAX_SECTIONS: u32 = 10;
/// Maximum number of extra docs paths.
pub(crate) const MAX_DOC_EXTRA_PATHS: usize = 64;

// ============================================================================
// SECTION: Configuration Types
// ============================================================================

/// Decision Gate MCP configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DecisionGateConfig {
    /// Server configuration.
    #[serde(default)]
    pub server: ServerConfig,
    /// Namespace policy configuration.
    #[serde(default)]
    pub namespace: NamespaceConfig,
    /// Development-mode overrides (explicit opt-in only).
    #[serde(default)]
    pub dev: DevConfig,
    /// Trust and policy configuration.
    #[serde(default)]
    pub trust: TrustConfig,
    /// Evidence disclosure policy configuration.
    #[serde(default)]
    pub evidence: EvidencePolicyConfig,
    /// Evidence anchor policy configuration.
    #[serde(default)]
    pub anchors: AnchorPolicyConfig,
    /// Provider contract discovery configuration.
    #[serde(default)]
    pub provider_discovery: ProviderDiscoveryConfig,
    /// Validation configuration for scenario and precheck inputs.
    #[serde(default)]
    pub validation: ValidationConfig,
    /// Dispatch policy configuration.
    #[serde(default)]
    pub policy: PolicyConfig,
    /// Run state store configuration.
    #[serde(default)]
    pub run_state_store: RunStateStoreConfig,
    /// Data shape registry configuration.
    #[serde(default)]
    pub schema_registry: SchemaRegistryConfig,
    /// Evidence provider configuration entries.
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    /// Documentation search and resources configuration.
    #[serde(default)]
    pub docs: DocsConfig,
    /// Optional runpack storage configuration.
    #[serde(default)]
    pub runpack_storage: Option<RunpackStorageConfig>,
    /// Optional config source metadata (not serialized).
    #[serde(skip)]
    pub source_modified_at: Option<SystemTime>,
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
        config.source_modified_at = fs::metadata(&resolved).and_then(|meta| meta.modified()).ok();
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
        self.namespace.validate()?;
        self.dev.validate(self.server.mode, self.namespace.authority.mode)?;
        self.validation.validate()?;
        self.policy.validate()?;
        self.run_state_store.validate()?;
        self.schema_registry.validate()?;
        self.anchors.validate()?;
        self.provider_discovery.validate()?;
        self.docs.validate()?;
        if let Some(storage) = &self.runpack_storage {
            storage.validate()?;
        }
        for provider in &self.providers {
            provider.validate()?;
        }
        Ok(())
    }

    /// Returns the effective trust requirement for the configured mode.
    #[must_use]
    pub fn effective_trust_requirement(&self) -> TrustRequirement {
        if self.is_dev_permissive() {
            TrustRequirement {
                min_lane: TrustLane::Asserted,
            }
        } else {
            TrustRequirement {
                min_lane: self.trust.min_lane,
            }
        }
    }

    /// Returns whether the default namespace is allowed.
    #[must_use]
    pub const fn allow_default_namespace(&self) -> bool {
        self.namespace.allow_default
    }

    /// Returns true when dev-permissive mode is enabled (explicit or legacy).
    #[must_use]
    pub fn is_dev_permissive(&self) -> bool {
        self.dev.permissive || self.server.mode == ServerMode::DevPermissive
    }
}

/// Runpack storage configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunpackStorageConfig {
    /// Object-store backed runpack storage.
    ObjectStore(ObjectStoreConfig),
}

impl RunpackStorageConfig {
    /// Validates runpack storage configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        match self {
            Self::ObjectStore(config) => config.validate(),
        }
    }
}

/// Supported object-store providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectStoreProvider {
    /// Amazon S3 compatible object storage.
    S3,
}

/// Object-store configuration for runpack storage.
#[derive(Debug, Clone, Deserialize)]
pub struct ObjectStoreConfig {
    /// Provider selection for the object store.
    pub provider: ObjectStoreProvider,
    /// Bucket name for runpack storage.
    pub bucket: String,
    /// Optional region (S3-only, defaults to environment).
    #[serde(default)]
    pub region: Option<String>,
    /// Optional object-store endpoint (S3-compatible).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Optional key prefix inside the bucket.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Force path-style addressing (S3-compatible).
    #[serde(default)]
    pub force_path_style: bool,
    /// Allow non-TLS endpoints (explicit opt-in).
    #[serde(default)]
    pub allow_http: bool,
}

impl ObjectStoreConfig {
    /// Validates object-store configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when object-store settings are invalid.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.bucket.trim().is_empty() {
            return Err(ConfigError::Invalid("runpack_storage.bucket must be set".to_string()));
        }
        if let Some(endpoint) = &self.endpoint {
            let trimmed = endpoint.trim();
            if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
                return Err(ConfigError::Invalid(
                    "runpack_storage.endpoint must include http:// or https://".to_string(),
                ));
            }
            if trimmed.starts_with("http://") && !self.allow_http {
                return Err(ConfigError::Invalid(
                    "runpack_storage.endpoint uses http:// without allow_http".to_string(),
                ));
            }
        }
        if let Some(prefix) = &self.prefix {
            validate_object_store_prefix(prefix)?;
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
    /// Operational mode for the server.
    #[serde(default)]
    pub mode: ServerMode,
    /// Bind address for HTTP or SSE transports.
    #[serde(default)]
    pub bind: Option<String>,
    /// Maximum request body size in bytes.
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
    /// Request limits (rate/concurrency).
    #[serde(default)]
    pub limits: ServerLimitsConfig,
    /// Optional authentication configuration for inbound tool calls.
    #[serde(default)]
    pub auth: Option<ServerAuthConfig>,
    /// Optional TLS configuration for HTTP/SSE transports.
    #[serde(default)]
    pub tls: Option<ServerTlsConfig>,
    /// Audit logging configuration.
    #[serde(default)]
    pub audit: ServerAuditConfig,
    /// Feedback disclosure configuration for tool responses.
    #[serde(default)]
    pub feedback: ServerFeedbackConfig,
    /// Tool visibility configuration for MCP tool listings.
    #[serde(default)]
    pub tools: ServerToolsConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: ServerTransport::Stdio,
            mode: ServerMode::Strict,
            bind: None,
            max_body_bytes: default_max_body_bytes(),
            limits: ServerLimitsConfig::default(),
            auth: None,
            tls: None,
            audit: ServerAuditConfig::default(),
            feedback: ServerFeedbackConfig::default(),
            tools: ServerToolsConfig::default(),
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
        self.limits.validate()?;
        if let Some(auth) = &self.auth {
            auth.validate()?;
        }
        if let Some(tls) = &self.tls {
            tls.validate()?;
        }
        self.audit.validate()?;
        self.feedback.validate()?;
        self.tools.validate()?;
        let auth_mode = self.auth.as_ref().map_or(ServerAuthMode::LocalOnly, |auth| auth.mode);
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
                if self.tls.is_some() {
                    return Err(ConfigError::Invalid(
                        "stdio transport does not support tls".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Feedback levels for tool responses.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackLevel {
    /// Summary-only feedback.
    #[default]
    Summary,
    /// Gate-level trace feedback without evidence values.
    Trace,
    /// Evidence-inclusive feedback (subject to disclosure policy).
    Evidence,
}

impl FeedbackLevel {
    /// Returns a ranking for ordering feedback levels.
    const fn rank(self) -> u8 {
        match self {
            Self::Summary => 0,
            Self::Trace => 1,
            Self::Evidence => 2,
        }
    }
}

/// Feedback configuration for MCP tools.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerFeedbackConfig {
    /// Feedback policy for `scenario_next`.
    #[serde(default)]
    pub scenario_next: ScenarioNextFeedbackConfig,
}

impl ServerFeedbackConfig {
    /// Validates feedback configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        self.scenario_next.validate()
    }
}

/// Feedback policy for `scenario_next` responses.
#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioNextFeedbackConfig {
    /// Default feedback level for non-local requests.
    #[serde(default = "default_scenario_next_feedback_default")]
    pub default: FeedbackLevel,
    /// Default feedback level for local-only requests.
    #[serde(default = "default_scenario_next_feedback_local_only")]
    pub local_only_default: FeedbackLevel,
    /// Maximum feedback level permitted.
    #[serde(default = "default_scenario_next_feedback_max")]
    pub max: FeedbackLevel,
    /// Subjects allowed to request trace feedback.
    #[serde(default = "default_scenario_next_trace_subjects")]
    pub trace_subjects: Vec<String>,
    /// Roles allowed to request trace feedback.
    #[serde(default)]
    pub trace_roles: Vec<String>,
    /// Subjects allowed to request evidence feedback.
    #[serde(default)]
    pub evidence_subjects: Vec<String>,
    /// Roles allowed to request evidence feedback.
    #[serde(default)]
    pub evidence_roles: Vec<String>,
}

impl Default for ScenarioNextFeedbackConfig {
    fn default() -> Self {
        Self {
            default: default_scenario_next_feedback_default(),
            local_only_default: default_scenario_next_feedback_local_only(),
            max: default_scenario_next_feedback_max(),
            trace_subjects: default_scenario_next_trace_subjects(),
            trace_roles: Vec::new(),
            evidence_subjects: Vec::new(),
            evidence_roles: Vec::new(),
        }
    }
}

impl ScenarioNextFeedbackConfig {
    /// Validates `scenario_next` feedback configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.default.rank() > self.max.rank() {
            return Err(ConfigError::Invalid(
                "server.feedback.scenario_next.default exceeds max".to_string(),
            ));
        }
        if self.local_only_default.rank() > self.max.rank() {
            return Err(ConfigError::Invalid(
                "server.feedback.scenario_next.local_only_default exceeds max".to_string(),
            ));
        }
        Ok(())
    }
}

/// Development-mode configuration (explicit opt-in only).
#[derive(Debug, Clone, Deserialize)]
pub struct DevConfig {
    /// Enable dev-permissive mode (asserted evidence allowed for scoped providers).
    #[serde(default)]
    pub permissive: bool,
    /// Dev-permissive scope selection.
    #[serde(default)]
    pub permissive_scope: DevPermissiveScope,
    /// Optional TTL in days for dev-permissive warnings.
    #[serde(default)]
    pub permissive_ttl_days: Option<u64>,
    /// Emit warnings when dev-permissive is enabled or expired.
    #[serde(default = "default_dev_permissive_warn")]
    pub permissive_warn: bool,
    /// Provider ids exempt from dev-permissive relaxations.
    #[serde(default = "default_dev_permissive_exempt_providers")]
    pub permissive_exempt_providers: Vec<String>,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            permissive: false,
            permissive_scope: DevPermissiveScope::default(),
            permissive_ttl_days: None,
            permissive_warn: default_dev_permissive_warn(),
            permissive_exempt_providers: default_dev_permissive_exempt_providers(),
        }
    }
}

impl DevConfig {
    /// Validates dev configuration.
    fn validate(
        &mut self,
        server_mode: ServerMode,
        namespace_mode: NamespaceAuthorityMode,
    ) -> Result<(), ConfigError> {
        if self.permissive_ttl_days == Some(0) {
            return Err(ConfigError::Invalid(
                "dev.permissive_ttl_days must be greater than zero".to_string(),
            ));
        }
        if server_mode == ServerMode::DevPermissive && !self.permissive {
            // Legacy compatibility: treat server.mode=dev_permissive as dev.permissive=true.
            self.permissive = true;
        }
        if (self.permissive || server_mode == ServerMode::DevPermissive)
            && namespace_mode == NamespaceAuthorityMode::AssetcoreHttp
        {
            return Err(ConfigError::Invalid(
                "dev.permissive not allowed when namespace.authority.mode=assetcore_http"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// Dev-permissive scope selection.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DevPermissiveScope {
    /// Allow asserted evidence only (non-exempt providers).
    #[default]
    AssertedEvidenceOnly,
}

/// Request limits for the MCP server.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerLimitsConfig {
    /// Maximum inflight requests.
    #[serde(default = "default_max_inflight")]
    pub max_inflight: usize,
    /// Optional rate limit configuration.
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
}

impl Default for ServerLimitsConfig {
    fn default() -> Self {
        Self {
            max_inflight: default_max_inflight(),
            rate_limit: None,
        }
    }
}

impl ServerLimitsConfig {
    /// Validates request limits.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.max_inflight == 0 {
            return Err(ConfigError::Invalid("max_inflight must be greater than zero".to_string()));
        }
        if let Some(rate_limit) = &self.rate_limit {
            rate_limit.validate()?;
        }
        Ok(())
    }
}

/// Rate limit configuration for MCP server requests.
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per time window.
    #[serde(default = "default_rate_limit_max_requests")]
    pub max_requests: u32,
    /// Window duration in milliseconds.
    #[serde(default = "default_rate_limit_window_ms")]
    pub window_ms: u64,
    /// Maximum number of distinct rate limit entries.
    #[serde(default = "default_rate_limit_max_entries")]
    pub max_entries: usize,
}

impl RateLimitConfig {
    /// Validates rate limit settings.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.max_requests == 0 {
            return Err(ConfigError::Invalid(
                "rate_limit max_requests must be greater than zero".to_string(),
            ));
        }
        if self.max_requests > MAX_RATE_LIMIT_REQUESTS {
            return Err(ConfigError::Invalid("rate_limit max_requests too large".to_string()));
        }
        if self.window_ms < MIN_RATE_LIMIT_WINDOW_MS || self.window_ms > MAX_RATE_LIMIT_WINDOW_MS {
            return Err(ConfigError::Invalid(format!(
                "rate_limit window_ms must be between {MIN_RATE_LIMIT_WINDOW_MS} and \
                 {MAX_RATE_LIMIT_WINDOW_MS}",
            )));
        }
        if self.max_entries == 0 {
            return Err(ConfigError::Invalid(
                "rate_limit max_entries must be greater than zero".to_string(),
            ));
        }
        if self.max_entries > MAX_RATE_LIMIT_ENTRIES {
            return Err(ConfigError::Invalid("rate_limit max_entries too large".to_string()));
        }
        Ok(())
    }
}

/// TLS configuration for MCP HTTP/SSE transports.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerTlsConfig {
    /// Server certificate chain (PEM).
    pub cert_path: String,
    /// Server private key (PEM).
    pub key_path: String,
    /// Optional client CA bundle (PEM) for mTLS.
    #[serde(default)]
    pub client_ca_path: Option<String>,
    /// Require client certificates when a client CA bundle is configured.
    #[serde(default = "default_tls_require_client_cert")]
    pub require_client_cert: bool,
}

impl ServerTlsConfig {
    /// Validates TLS configuration paths.
    fn validate(&self) -> Result<(), ConfigError> {
        validate_path_string("tls.cert_path", &self.cert_path)?;
        validate_path_string("tls.key_path", &self.key_path)?;
        if let Some(path) = &self.client_ca_path {
            validate_path_string("tls.client_ca_path", path)?;
        }
        Ok(())
    }
}

/// Audit logging configuration for MCP server requests.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerAuditConfig {
    /// Enable structured audit logging.
    #[serde(default = "default_audit_enabled")]
    pub enabled: bool,
    /// Optional audit log path (JSON lines).
    #[serde(default)]
    pub path: Option<String>,
    /// Log raw precheck request/response payloads (explicit opt-in).
    #[serde(default)]
    pub log_precheck_payloads: bool,
}

impl Default for ServerAuditConfig {
    fn default() -> Self {
        Self {
            enabled: default_audit_enabled(),
            path: None,
            log_precheck_payloads: false,
        }
    }
}

impl ServerAuditConfig {
    /// Validates audit configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if let Some(path) = &self.path {
            validate_path_string("audit.path", path)?;
        }
        Ok(())
    }
}

/// Server operating modes for security posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerMode {
    /// Strict mode (verified-only evidence, default namespace blocked).
    #[default]
    Strict,
    /// Dev-permissive mode (asserted evidence allowed).
    DevPermissive,
}

/// Supported MCP transport types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
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
    /// Accepted bearer tokens (required for `bearer_token` mode).
    #[serde(default)]
    pub bearer_tokens: Vec<String>,
    /// Allowed mTLS subjects (required for mtls mode).
    #[serde(default)]
    pub mtls_subjects: Vec<String>,
    /// Optional tool allowlist (per-tool authorization).
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Optional principal role mappings for registry ACL.
    #[serde(default)]
    pub principals: Vec<PrincipalConfig>,
}

/// Tool visibility configuration for MCP tool listings.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerToolsConfig {
    /// Visibility mode for tools/list.
    #[serde(default)]
    pub mode: ToolVisibilityMode,
    /// Optional allowlist of visible tools.
    #[serde(default)]
    pub allowlist: Vec<String>,
    /// Optional denylist of visible tools.
    #[serde(default)]
    pub denylist: Vec<String>,
}

impl ServerToolsConfig {
    /// Validates tool visibility configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.allowlist.len() > MAX_TOOL_VISIBILITY_RULES {
            return Err(ConfigError::Invalid("too many tool allowlist entries".to_string()));
        }
        if self.denylist.len() > MAX_TOOL_VISIBILITY_RULES {
            return Err(ConfigError::Invalid("too many tool denylist entries".to_string()));
        }
        for tool_name in self.allowlist.iter().chain(self.denylist.iter()) {
            if tool_name.trim().is_empty() {
                return Err(ConfigError::Invalid(
                    "server.tools allowlist/denylist entries must be non-empty".to_string(),
                ));
            }
            if ToolName::parse(tool_name).is_none() {
                return Err(ConfigError::Invalid(format!(
                    "unknown tool in server.tools list: {tool_name}"
                )));
            }
        }
        Ok(())
    }
}

/// Tool visibility modes for MCP tool listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolVisibilityMode {
    /// Filter tools/list output by allowlist/denylist.
    #[default]
    Filter,
    /// Do not filter tools/list output (legacy compatibility).
    Passthrough,
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
        if self.principals.len() > MAX_AUTH_TOKENS {
            return Err(ConfigError::Invalid("too many principal mappings".to_string()));
        }
        for principal in &self.principals {
            principal.validate()?;
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

/// Principal mapping for registry ACL enforcement.
#[derive(Debug, Clone, Deserialize)]
pub struct PrincipalConfig {
    /// Principal identifier (subject or token fingerprint label).
    pub subject: String,
    /// Optional policy class label (e.g., prod, scratch).
    #[serde(default)]
    pub policy_class: Option<String>,
    /// Role bindings for this principal.
    #[serde(default)]
    pub roles: Vec<PrincipalRoleConfig>,
}

impl PrincipalConfig {
    /// Validates principal configuration constraints.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.subject.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "auth.principals.subject must be non-empty".to_string(),
            ));
        }
        if let Some(policy_class) = &self.policy_class
            && policy_class.trim().is_empty()
        {
            return Err(ConfigError::Invalid(
                "auth.principals.policy_class must be non-empty".to_string(),
            ));
        }
        if self.roles.len() > MAX_PRINCIPAL_ROLES {
            return Err(ConfigError::Invalid(
                "auth.principals.roles exceeds max entries".to_string(),
            ));
        }
        for role in &self.roles {
            role.validate()?;
        }
        Ok(())
    }
}

/// Role binding for a principal (optional tenant/namespace scope).
#[derive(Debug, Clone, Deserialize)]
pub struct PrincipalRoleConfig {
    /// Role name (e.g., `NamespaceAdmin`).
    pub name: String,
    /// Optional tenant scope.
    #[serde(default)]
    pub tenant_id: Option<TenantId>,
    /// Optional namespace scope.
    #[serde(default)]
    pub namespace_id: Option<NamespaceId>,
}

impl PrincipalRoleConfig {
    /// Validates role configuration constraints.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.name.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "auth.principals.roles.name must be non-empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Trust configuration for evidence providers.
#[derive(Debug, Clone, Deserialize)]
pub struct TrustConfig {
    /// Default trust policy for providers.
    #[serde(default = "default_trust_policy")]
    pub default_policy: TrustPolicy,
    /// Minimum evidence trust lane accepted by the control plane.
    #[serde(default = "default_trust_lane")]
    pub min_lane: TrustLane,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            default_policy: TrustPolicy::Audit,
            min_lane: default_trust_lane(),
        }
    }
}

/// Namespace policy configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct NamespaceConfig {
    /// Allow the default namespace identifier (id = 1).
    #[serde(default)]
    pub allow_default: bool,
    /// Explicit tenant allowlist for the default namespace id.
    #[serde(default)]
    pub default_tenants: Vec<TenantId>,
    /// Namespace authority configuration.
    #[serde(default)]
    pub authority: NamespaceAuthorityConfig,
}

impl NamespaceConfig {
    /// Validates namespace policy configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the namespace policy is invalid.
    fn validate(&self) -> Result<(), ConfigError> {
        self.authority.validate()?;
        if self.allow_default && self.default_tenants.is_empty() {
            return Err(ConfigError::Invalid(
                "namespace.allow_default requires namespace.default_tenants".to_string(),
            ));
        }
        Ok(())
    }
}

/// Namespace authority selection.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceAuthorityMode {
    /// No external namespace authority.
    #[default]
    None,
    /// Asset Core namespace authority via HTTP.
    AssetcoreHttp,
}

/// Namespace authority configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceAuthorityConfig {
    /// Authority mode selection.
    #[serde(default)]
    pub mode: NamespaceAuthorityMode,
    /// Asset Core authority configuration.
    #[serde(default)]
    pub assetcore: Option<AssetCoreNamespaceAuthorityConfig>,
}

impl Default for NamespaceAuthorityConfig {
    fn default() -> Self {
        Self {
            mode: NamespaceAuthorityMode::None,
            assetcore: None,
        }
    }
}

impl NamespaceAuthorityConfig {
    /// Validates namespace authority configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the authority configuration is invalid.
    fn validate(&self) -> Result<(), ConfigError> {
        match self.mode {
            NamespaceAuthorityMode::None => {
                if self.assetcore.is_some() {
                    return Err(ConfigError::Invalid(
                        "namespace.authority.assetcore only allowed when mode=assetcore_http"
                            .to_string(),
                    ));
                }
            }
            NamespaceAuthorityMode::AssetcoreHttp => {
                let Some(assetcore) = &self.assetcore else {
                    return Err(ConfigError::Invalid(
                        "namespace.authority.mode=assetcore_http requires \
                         namespace.authority.assetcore"
                            .to_string(),
                    ));
                };
                assetcore.validate()?;
            }
        }
        Ok(())
    }
}

/// Asset Core namespace authority configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetCoreNamespaceAuthorityConfig {
    /// Base URL for Asset Core write daemon.
    pub base_url: String,
    /// Optional bearer token for namespace queries.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Connect timeout in milliseconds.
    #[serde(default = "default_namespace_auth_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// Request timeout in milliseconds.
    #[serde(default = "default_namespace_auth_request_timeout_ms")]
    pub request_timeout_ms: u64,
}

impl AssetCoreNamespaceAuthorityConfig {
    /// Validates Asset Core namespace authority configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the Asset Core settings are invalid.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.base_url.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "namespace.authority.assetcore.base_url is required".to_string(),
            ));
        }
        if let Some(token) = &self.auth_token
            && token.trim().is_empty()
        {
            return Err(ConfigError::Invalid(
                "namespace.authority.assetcore.auth_token must be non-empty".to_string(),
            ));
        }
        validate_timeout_range(
            "namespace.authority.assetcore.connect_timeout_ms",
            self.connect_timeout_ms,
            MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
        )?;
        validate_timeout_range(
            "namespace.authority.assetcore.request_timeout_ms",
            self.request_timeout_ms,
            MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
            MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        )?;
        Ok(())
    }
}

/// Policy engine configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PolicyConfig {
    /// Policy engine selection.
    #[serde(default)]
    pub engine: PolicyEngine,
    /// Static policy configuration.
    #[serde(default, rename = "static")]
    pub static_policy: Option<StaticPolicyConfig>,
}

impl PolicyConfig {
    /// Validates policy configuration for internal consistency.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when policy settings are invalid.
    fn validate(&self) -> Result<(), ConfigError> {
        match self.engine {
            PolicyEngine::Static => {
                let Some(static_policy) = &self.static_policy else {
                    return Err(ConfigError::Invalid(
                        "policy.engine=static requires policy.static".to_string(),
                    ));
                };
                static_policy.validate().map_err(ConfigError::Invalid)?;
            }
            PolicyEngine::PermitAll | PolicyEngine::DenyAll => {
                if self.static_policy.is_some() {
                    return Err(ConfigError::Invalid(
                        "policy.static only allowed when engine=static".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Builds the runtime dispatch policy adapter.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the configuration is missing static policy data.
    pub fn dispatch_policy(&self) -> Result<DispatchPolicy, ConfigError> {
        match self.engine {
            PolicyEngine::PermitAll => Ok(DispatchPolicy::PermitAll),
            PolicyEngine::DenyAll => Ok(DispatchPolicy::DenyAll),
            PolicyEngine::Static => {
                let static_policy = self.static_policy.clone().ok_or_else(|| {
                    ConfigError::Invalid("policy.static is required for static engine".to_string())
                })?;
                Ok(DispatchPolicy::Static(static_policy))
            }
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

/// Documentation search and resources configuration.
#[allow(
    clippy::struct_excessive_bools,
    reason = "Explicit toggles improve configuration clarity for docs surfaces."
)]
#[derive(Debug, Clone, Deserialize)]
pub struct DocsConfig {
    /// Enable docs surfaces globally.
    #[serde(default = "default_docs_enabled")]
    pub enabled: bool,
    /// Enable docs search tool.
    #[serde(default = "default_docs_enable_search")]
    pub enable_search: bool,
    /// Enable MCP resources list/read.
    #[serde(default = "default_docs_enable_resources")]
    pub enable_resources: bool,
    /// Include the embedded default docs set.
    #[serde(default = "default_docs_include_default")]
    pub include_default_docs: bool,
    /// Extra doc paths to ingest (files or directories).
    #[serde(default)]
    pub extra_paths: Vec<String>,
    /// Maximum size for a single doc entry in bytes.
    #[serde(default = "default_doc_max_bytes")]
    pub max_doc_bytes: usize,
    /// Maximum total docs bytes for the catalog.
    #[serde(default = "default_doc_max_total_bytes")]
    pub max_total_bytes: usize,
    /// Maximum number of docs in the catalog.
    #[serde(default = "default_doc_max_docs")]
    pub max_docs: usize,
    /// Maximum sections returned by docs search.
    #[serde(default = "default_doc_max_sections")]
    pub max_sections: u32,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            enabled: default_docs_enabled(),
            enable_search: default_docs_enable_search(),
            enable_resources: default_docs_enable_resources(),
            include_default_docs: default_docs_include_default(),
            extra_paths: Vec::new(),
            max_doc_bytes: default_doc_max_bytes(),
            max_total_bytes: default_doc_max_total_bytes(),
            max_docs: default_doc_max_docs(),
            max_sections: default_doc_max_sections(),
        }
    }
}

impl DocsConfig {
    /// Validates docs configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.max_doc_bytes == 0 || self.max_doc_bytes > MAX_DOC_MAX_BYTES {
            return Err(ConfigError::Invalid("docs.max_doc_bytes out of range".to_string()));
        }
        if self.max_total_bytes == 0 || self.max_total_bytes > MAX_DOC_MAX_TOTAL_BYTES {
            return Err(ConfigError::Invalid("docs.max_total_bytes out of range".to_string()));
        }
        if self.max_docs == 0 || self.max_docs > MAX_DOC_MAX_DOCS {
            return Err(ConfigError::Invalid("docs.max_docs out of range".to_string()));
        }
        if self.max_sections == 0 || self.max_sections > MAX_DOC_MAX_SECTIONS {
            return Err(ConfigError::Invalid("docs.max_sections out of range".to_string()));
        }
        if self.extra_paths.len() > MAX_DOC_EXTRA_PATHS {
            return Err(ConfigError::Invalid("docs.extra_paths too many entries".to_string()));
        }
        for path in &self.extra_paths {
            validate_path_string("docs.extra_paths", path)?;
        }
        Ok(())
    }
}

impl Default for EvidencePolicyConfig {
    fn default() -> Self {
        Self {
            allow_raw_values: false,
            require_provider_opt_in: true,
        }
    }
}

/// Provider contract discovery configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderDiscoveryConfig {
    /// Optional provider allowlist for contract/schema disclosure.
    #[serde(default)]
    pub allowlist: Vec<String>,
    /// Provider denylist for contract/schema disclosure.
    #[serde(default)]
    pub denylist: Vec<String>,
    /// Maximum response size for provider discovery tools.
    #[serde(default = "default_provider_discovery_max_bytes")]
    pub max_response_bytes: usize,
}

impl Default for ProviderDiscoveryConfig {
    fn default() -> Self {
        Self {
            allowlist: Vec::new(),
            denylist: Vec::new(),
            max_response_bytes: default_provider_discovery_max_bytes(),
        }
    }
}

impl ProviderDiscoveryConfig {
    /// Returns true when a provider is allowed to be disclosed.
    #[must_use]
    pub fn is_allowed(&self, provider_id: &str) -> bool {
        if self.denylist.iter().any(|item| item == provider_id) {
            return false;
        }
        if self.allowlist.is_empty() {
            return true;
        }
        self.allowlist.iter().any(|item| item == provider_id)
    }

    /// Validates provider discovery configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        for entry in self.allowlist.iter().chain(self.denylist.iter()) {
            if entry.trim().is_empty() {
                return Err(ConfigError::Invalid(
                    "provider_discovery allow/deny entries must be non-empty".to_string(),
                ));
            }
        }
        if self.max_response_bytes == 0 {
            return Err(ConfigError::Invalid(
                "provider_discovery.max_response_bytes must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Evidence anchor policy configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AnchorPolicyConfig {
    /// Provider-specific anchor requirements.
    #[serde(default)]
    pub providers: Vec<AnchorProviderConfig>,
}

impl AnchorPolicyConfig {
    /// Validates anchor policy configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when provider entries are invalid.
    fn validate(&self) -> Result<(), ConfigError> {
        for provider in &self.providers {
            provider.validate()?;
        }
        Ok(())
    }

    /// Builds the runtime evidence anchor policy for the control plane.
    #[must_use]
    pub fn to_policy(&self) -> EvidenceAnchorPolicy {
        let mut providers = Vec::new();
        for provider in &self.providers {
            providers.push(ProviderAnchorPolicy {
                provider_id: ProviderId::new(&provider.provider_id),
                requirement: AnchorRequirement {
                    anchor_type: provider.anchor_type.clone(),
                    required_fields: normalize_required_fields(&provider.required_fields),
                },
            });
        }
        EvidenceAnchorPolicy {
            providers,
        }
    }
}

/// Provider-specific anchor requirement configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AnchorProviderConfig {
    /// Provider identifier.
    pub provider_id: String,
    /// Anchor type required for evidence results.
    pub anchor_type: String,
    /// Required fields inside the anchor payload.
    #[serde(default)]
    pub required_fields: Vec<String>,
}

impl AnchorProviderConfig {
    /// Validates provider-specific anchor configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the anchor requirement is invalid.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.provider_id.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "anchors.providers.provider_id must be non-empty".to_string(),
            ));
        }
        if self.anchor_type.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "anchors.providers.anchor_type must be non-empty".to_string(),
            ));
        }
        if normalize_required_fields(&self.required_fields).is_empty() {
            return Err(ConfigError::Invalid(
                "anchors.providers.required_fields must be non-empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Validation configuration for strict comparator enforcement.
#[allow(clippy::struct_excessive_bools, reason = "Config flags mirror user-facing toggles.")]
#[derive(Debug, Clone, Deserialize)]
pub struct ValidationConfig {
    /// Enforce strict comparator validation (default on).
    #[serde(default = "default_validation_strict")]
    pub strict: bool,
    /// Validation profile name.
    #[serde(default)]
    pub profile: ValidationProfile,
    /// Allow permissive mode when strict is disabled.
    #[serde(default)]
    pub allow_permissive: bool,
    /// Enable lexicographic comparator family.
    #[serde(default)]
    pub enable_lexicographic: bool,
    /// Enable deep equality comparator family.
    #[serde(default)]
    pub enable_deep_equals: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            strict: default_validation_strict(),
            profile: ValidationProfile::default(),
            allow_permissive: false,
            enable_lexicographic: false,
            enable_deep_equals: false,
        }
    }
}

impl ValidationConfig {
    /// Validates validation configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if !self.strict && !self.allow_permissive {
            return Err(ConfigError::Invalid(
                "validation.strict=false requires validation.allow_permissive=true".to_string(),
            ));
        }
        Ok(())
    }
}

/// Validation profile identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationProfile {
    /// Strict comparator validation profile v1.
    #[default]
    StrictCoreV1,
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

/// Data shape registry configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaRegistryConfig {
    /// Registry backend type.
    #[serde(rename = "type", default)]
    pub registry_type: SchemaRegistryType,
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
    /// Maximum schema payload size in bytes.
    #[serde(default = "default_schema_max_bytes")]
    pub max_schema_bytes: usize,
    /// Optional max schemas per tenant + namespace.
    #[serde(default)]
    pub max_entries: Option<u64>,
    /// Registry ACL configuration.
    #[serde(default)]
    pub acl: RegistryAclConfig,
}

impl Default for SchemaRegistryConfig {
    fn default() -> Self {
        Self {
            registry_type: SchemaRegistryType::default(),
            path: None,
            busy_timeout_ms: default_store_busy_timeout_ms(),
            journal_mode: SqliteStoreMode::default(),
            sync_mode: SqliteSyncMode::default(),
            max_schema_bytes: default_schema_max_bytes(),
            max_entries: None,
            acl: RegistryAclConfig::default(),
        }
    }
}

impl SchemaRegistryConfig {
    /// Validates schema registry configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.max_schema_bytes == 0 || self.max_schema_bytes > MAX_SCHEMA_MAX_BYTES {
            return Err(ConfigError::Invalid(
                "schema_registry max_schema_bytes out of range".to_string(),
            ));
        }
        if self.max_entries == Some(0) {
            return Err(ConfigError::Invalid(
                "schema_registry max_entries must be greater than zero".to_string(),
            ));
        }
        match self.registry_type {
            SchemaRegistryType::Memory => {
                if self.path.is_some() {
                    return Err(ConfigError::Invalid(
                        "memory schema_registry must not set path".to_string(),
                    ));
                }
                self.acl.validate()?;
                Ok(())
            }
            SchemaRegistryType::Sqlite => {
                let path = self.path.as_ref().ok_or_else(|| {
                    ConfigError::Invalid("sqlite schema_registry requires path".to_string())
                })?;
                validate_store_path(path)?;
                self.acl.validate()?;
                Ok(())
            }
        }
    }
}

/// Registry ACL mode selection.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RegistryAclMode {
    /// Built-in role-based defaults.
    #[default]
    Builtin,
    /// Custom rule set.
    Custom,
}

/// Registry ACL default effect.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RegistryAclDefault {
    /// Deny by default.
    #[default]
    Deny,
    /// Allow by default.
    Allow,
}

/// Registry ACL rule effect.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryAclEffect {
    /// Allow access.
    Allow,
    /// Deny access.
    Deny,
}

/// Registry ACL action types.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryAclAction {
    /// Register schema.
    Register,
    /// List schemas.
    List,
    /// Get schema.
    Get,
}

/// Registry ACL rule definition.
#[derive(Debug, Clone, Deserialize)]
pub struct RegistryAclRule {
    /// Rule effect.
    pub effect: RegistryAclEffect,
    /// Actions covered by this rule (empty = any).
    #[serde(default)]
    pub actions: Vec<RegistryAclAction>,
    /// Tenant identifiers (empty = any).
    #[serde(default)]
    pub tenants: Vec<TenantId>,
    /// Namespace identifiers (empty = any).
    #[serde(default)]
    pub namespaces: Vec<NamespaceId>,
    /// Principal subjects (empty = any).
    #[serde(default)]
    pub subjects: Vec<String>,
    /// Role names (empty = any).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Policy classes (empty = any).
    #[serde(default)]
    pub policy_classes: Vec<String>,
}

impl RegistryAclRule {
    /// Validates ACL rule configuration constraints.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.subjects.iter().any(|s| s.trim().is_empty()) {
            return Err(ConfigError::Invalid(
                "schema_registry.acl.rules.subjects must be non-empty".to_string(),
            ));
        }
        if self.roles.iter().any(|r| r.trim().is_empty()) {
            return Err(ConfigError::Invalid(
                "schema_registry.acl.rules.roles must be non-empty".to_string(),
            ));
        }
        if self.policy_classes.iter().any(|p| p.trim().is_empty()) {
            return Err(ConfigError::Invalid(
                "schema_registry.acl.rules.policy_classes must be non-empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Registry ACL configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RegistryAclConfig {
    /// ACL mode selection.
    #[serde(default)]
    pub mode: RegistryAclMode,
    /// Default effect when no rules match.
    #[serde(default)]
    pub default: RegistryAclDefault,
    /// Allow local-only subjects to access the registry when using built-in ACL.
    #[serde(default = "default_registry_acl_allow_local_only")]
    pub allow_local_only: bool,
    /// Require schema signing metadata.
    #[serde(default)]
    pub require_signing: bool,
    /// Custom ACL rules.
    #[serde(default)]
    pub rules: Vec<RegistryAclRule>,
}

impl RegistryAclConfig {
    /// Validates ACL configuration constraints.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.rules.len() > MAX_REGISTRY_ACL_RULES {
            return Err(ConfigError::Invalid(
                "schema_registry.acl.rules exceeds max entries".to_string(),
            ));
        }
        for rule in &self.rules {
            rule.validate()?;
        }
        Ok(())
    }
}

impl Default for RegistryAclConfig {
    fn default() -> Self {
        Self {
            mode: RegistryAclMode::default(),
            default: RegistryAclDefault::default(),
            allow_local_only: default_registry_acl_allow_local_only(),
            require_signing: false,
            rules: Vec::new(),
        }
    }
}

/// Schema registry backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SchemaRegistryType {
    /// Use the in-memory registry.
    #[default]
    Memory,
    /// Use `SQLite`-backed durable registry.
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
        if let Some(auth) = &self.auth {
            auth.validate()?;
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

    /// Parses the provider-specific config payload or returns the default.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the config payload cannot be deserialized.
    pub fn parse_config<T: for<'de> Deserialize<'de> + Default>(&self) -> Result<T, ConfigError> {
        self.config.as_ref().map_or_else(
            || Ok(T::default()),
            |value| {
                value
                    .clone()
                    .try_into::<T>()
                    .map_err(|err| ConfigError::Invalid(format!("provider config error: {err}")))
            },
        )
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

impl ProviderAuthConfig {
    /// Validates provider auth configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if let Some(token) = &self.bearer_token
            && token.trim().is_empty()
        {
            return Err(ConfigError::Invalid(
                "providers.auth.bearer_token must be non-empty".to_string(),
            ));
        }
        Ok(())
    }
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

/// Validates a path string against length constraints.
fn validate_path_string(field: &str, value: &str) -> Result<(), ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::Invalid(format!("{field} must be non-empty")));
    }
    if trimmed.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ConfigError::Invalid(format!("{field} exceeds max length")));
    }
    let path = Path::new(trimmed);
    for component in path.components() {
        let component_value = component.as_os_str().to_string_lossy();
        if component_value.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(ConfigError::Invalid(format!("{field} path component too long")));
        }
    }
    Ok(())
}

/// Validates the object-store prefix string.
fn validate_object_store_prefix(value: &str) -> Result<(), ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::Invalid("runpack_storage.prefix must be non-empty".to_string()));
    }
    if trimmed.contains('\\') {
        return Err(ConfigError::Invalid(
            "runpack_storage.prefix must not contain backslashes".to_string(),
        ));
    }
    if trimmed.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ConfigError::Invalid("runpack_storage.prefix exceeds max length".to_string()));
    }
    if trimmed.starts_with('/') {
        return Err(ConfigError::Invalid("runpack_storage.prefix must be relative".to_string()));
    }
    let normalized = trimmed.strip_suffix('/').unwrap_or(trimmed);
    let path = Path::new(normalized);
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let segment = value.to_string_lossy();
                if segment.len() > MAX_PATH_COMPONENT_LENGTH {
                    return Err(ConfigError::Invalid(
                        "runpack_storage.prefix segment too long".to_string(),
                    ));
                }
                if segment == "." || segment == ".." || segment.contains(['/', '\\']) {
                    return Err(ConfigError::Invalid(
                        "runpack_storage.prefix segment invalid".to_string(),
                    ));
                }
            }
            _ => {
                return Err(ConfigError::Invalid(
                    "runpack_storage.prefix must be relative without traversal".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Default maximum request body size in bytes.
pub(crate) const fn default_max_body_bytes() -> usize {
    1024 * 1024
}

/// Default feedback level for non-local requests.
pub(crate) const fn default_scenario_next_feedback_default() -> FeedbackLevel {
    FeedbackLevel::Summary
}

/// Default feedback level for local-only requests.
pub(crate) const fn default_scenario_next_feedback_local_only() -> FeedbackLevel {
    FeedbackLevel::Trace
}

/// Maximum feedback level allowed by default.
pub(crate) const fn default_scenario_next_feedback_max() -> FeedbackLevel {
    FeedbackLevel::Trace
}

/// Default subjects permitted to request trace feedback.
pub(crate) fn default_scenario_next_trace_subjects() -> Vec<String> {
    vec!["loopback".to_string(), "stdio".to_string()]
}

/// Default maximum inflight requests.
pub(crate) const fn default_max_inflight() -> usize {
    DEFAULT_MAX_INFLIGHT
}

/// Default max requests per rate limit window.
pub(crate) const fn default_rate_limit_max_requests() -> u32 {
    DEFAULT_RATE_LIMIT_MAX_REQUESTS
}

/// Default rate limit window in milliseconds.
pub(crate) const fn default_rate_limit_window_ms() -> u64 {
    DEFAULT_RATE_LIMIT_WINDOW_MS
}

/// Default max entries for rate limiting.
pub(crate) const fn default_rate_limit_max_entries() -> usize {
    DEFAULT_RATE_LIMIT_MAX_ENTRIES
}

/// Default to requiring client certificates when configured.
pub(crate) const fn default_tls_require_client_cert() -> bool {
    true
}

/// Default audit logging enabled.
pub(crate) const fn default_audit_enabled() -> bool {
    true
}

/// Default dev-permissive warning enabled.
pub(crate) const fn default_dev_permissive_warn() -> bool {
    true
}

/// Default provider ids exempt from dev-permissive relaxations.
pub(crate) fn default_dev_permissive_exempt_providers() -> Vec<String> {
    vec!["assetcore_read".to_string(), "assetcore".to_string()]
}

/// Default value for requiring provider opt-in to raw evidence.
pub(crate) const fn default_require_provider_opt_in() -> bool {
    true
}

/// Default maximum response size for provider discovery tooling.
pub(crate) const fn default_provider_discovery_max_bytes() -> usize {
    DEFAULT_PROVIDER_DISCOVERY_MAX_BYTES
}

/// Default to enabling docs surfaces.
pub(crate) const fn default_docs_enabled() -> bool {
    true
}

/// Default to enabling docs search.
pub(crate) const fn default_docs_enable_search() -> bool {
    true
}

/// Default to enabling resources/list and resources/read.
pub(crate) const fn default_docs_enable_resources() -> bool {
    true
}

/// Default to including the embedded docs corpus.
pub(crate) const fn default_docs_include_default() -> bool {
    true
}

/// Default maximum size for a single doc entry.
pub(crate) const fn default_doc_max_bytes() -> usize {
    DEFAULT_DOC_MAX_BYTES
}

/// Default maximum total docs size.
pub(crate) const fn default_doc_max_total_bytes() -> usize {
    DEFAULT_DOC_MAX_TOTAL_BYTES
}

/// Default maximum docs count.
pub(crate) const fn default_doc_max_docs() -> usize {
    DEFAULT_DOC_MAX_DOCS
}

/// Default maximum sections returned by docs search.
pub(crate) const fn default_doc_max_sections() -> u32 {
    DEFAULT_DOC_MAX_SECTIONS
}

/// Default busy timeout for the `SQLite` store (ms).
pub(crate) const fn default_store_busy_timeout_ms() -> u64 {
    5_000
}

/// Default max schema size for registry payloads.
pub(crate) const fn default_schema_max_bytes() -> usize {
    DEFAULT_SCHEMA_MAX_BYTES
}

/// Default allow-local-only registry ACL behavior.
pub(crate) const fn default_registry_acl_allow_local_only() -> bool {
    false
}

/// Default trust policy for providers.
pub(crate) const fn default_trust_policy() -> TrustPolicy {
    TrustPolicy::Audit
}

/// Default minimum evidence trust lane.
pub(crate) const fn default_trust_lane() -> TrustLane {
    TrustLane::Verified
}

/// Default strict validation toggle.
pub(crate) const fn default_validation_strict() -> bool {
    true
}

/// Default MCP provider connect timeout in milliseconds.
pub(crate) const fn default_provider_connect_timeout_ms() -> u64 {
    2_000
}

/// Default MCP provider request timeout in milliseconds.
pub(crate) const fn default_provider_request_timeout_ms() -> u64 {
    10_000
}

/// Default namespace authority connect timeout in milliseconds.
pub(crate) const fn default_namespace_auth_connect_timeout_ms() -> u64 {
    DEFAULT_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS
}

/// Default namespace authority request timeout in milliseconds.
pub(crate) const fn default_namespace_auth_request_timeout_ms() -> u64 {
    DEFAULT_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS
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

/// Normalizes required field names by trimming and deduplicating.
fn normalize_required_fields(fields: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = fields
        .iter()
        .map(|field| field.trim().to_string())
        .filter(|field| !field.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
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

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "Test fixtures use explicit asserts and unwraps for clarity."
    )]

    use super::*;

    // ============================================================================
    // SECTION: DocsConfig::validate() Tests (24 tests)
    // ============================================================================

    #[test]
    fn docs_config_validate_accepts_default() {
        let config = DocsConfig::default();
        assert!(config.validate().is_ok(), "default DocsConfig should pass validation");
    }

    #[test]
    fn docs_config_validate_accepts_disabled() {
        let config = DocsConfig {
            enabled: false,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "disabled DocsConfig should pass validation");
    }

    #[test]
    fn docs_config_validate_accepts_custom_limits() {
        let config = DocsConfig {
            max_doc_bytes: 128 * 1024,
            max_total_bytes: 512 * 1024,
            max_docs: 16,
            max_sections: 5,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "custom limits within bounds should pass");
    }

    // max_doc_bytes boundary tests
    #[test]
    fn docs_config_validate_rejects_max_doc_bytes_zero() {
        let config = DocsConfig {
            max_doc_bytes: 0,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_doc_bytes=0 should fail validation");
        assert!(result.unwrap_err().to_string().contains("max_doc_bytes"));
    }

    #[test]
    fn docs_config_validate_rejects_max_doc_bytes_too_large() {
        let config = DocsConfig {
            max_doc_bytes: MAX_DOC_MAX_BYTES + 1,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_doc_bytes exceeding limit should fail");
        assert!(result.unwrap_err().to_string().contains("max_doc_bytes"));
    }

    #[test]
    fn docs_config_validate_accepts_max_doc_bytes_at_max() {
        let config = DocsConfig {
            max_doc_bytes: MAX_DOC_MAX_BYTES,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_doc_bytes at maximum should pass");
    }

    #[test]
    fn docs_config_validate_accepts_max_doc_bytes_at_min() {
        let config = DocsConfig {
            max_doc_bytes: 1,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_doc_bytes=1 should pass");
    }

    // max_total_bytes boundary tests
    #[test]
    fn docs_config_validate_rejects_max_total_bytes_zero() {
        let config = DocsConfig {
            max_total_bytes: 0,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_total_bytes=0 should fail validation");
        assert!(result.unwrap_err().to_string().contains("max_total_bytes"));
    }

    #[test]
    fn docs_config_validate_rejects_max_total_bytes_too_large() {
        let config = DocsConfig {
            max_total_bytes: MAX_DOC_MAX_TOTAL_BYTES + 1,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_total_bytes exceeding limit should fail");
        assert!(result.unwrap_err().to_string().contains("max_total_bytes"));
    }

    #[test]
    fn docs_config_validate_accepts_max_total_bytes_at_max() {
        let config = DocsConfig {
            max_total_bytes: MAX_DOC_MAX_TOTAL_BYTES,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_total_bytes at maximum should pass");
    }

    #[test]
    fn docs_config_validate_accepts_max_total_bytes_at_min() {
        let config = DocsConfig {
            max_total_bytes: 1,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_total_bytes=1 should pass");
    }

    // max_docs boundary tests
    #[test]
    fn docs_config_validate_rejects_max_docs_zero() {
        let config = DocsConfig {
            max_docs: 0,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_docs=0 should fail validation");
        assert!(result.unwrap_err().to_string().contains("max_docs"));
    }

    #[test]
    fn docs_config_validate_rejects_max_docs_too_large() {
        let config = DocsConfig {
            max_docs: MAX_DOC_MAX_DOCS + 1,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_docs exceeding limit should fail");
        assert!(result.unwrap_err().to_string().contains("max_docs"));
    }

    #[test]
    fn docs_config_validate_accepts_max_docs_at_max() {
        let config = DocsConfig {
            max_docs: MAX_DOC_MAX_DOCS,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_docs at maximum should pass");
    }

    #[test]
    fn docs_config_validate_accepts_max_docs_at_min() {
        let config = DocsConfig {
            max_docs: 1,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_docs=1 should pass");
    }

    // max_sections boundary tests
    #[test]
    fn docs_config_validate_rejects_max_sections_zero() {
        let config = DocsConfig {
            max_sections: 0,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_sections=0 should fail validation");
        assert!(result.unwrap_err().to_string().contains("max_sections"));
    }

    #[test]
    fn docs_config_validate_rejects_max_sections_too_large() {
        let config = DocsConfig {
            max_sections: MAX_DOC_MAX_SECTIONS + 1,
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "max_sections exceeding limit should fail");
        assert!(result.unwrap_err().to_string().contains("max_sections"));
    }

    #[test]
    fn docs_config_validate_accepts_max_sections_at_max() {
        let config = DocsConfig {
            max_sections: MAX_DOC_MAX_SECTIONS,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_sections at maximum should pass");
    }

    #[test]
    fn docs_config_validate_accepts_max_sections_at_min() {
        let config = DocsConfig {
            max_sections: 1,
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "max_sections=1 should pass");
    }

    // extra_paths validation tests
    #[test]
    fn docs_config_validate_rejects_too_many_extra_paths() {
        let config = DocsConfig {
            extra_paths: vec!["path".to_string(); MAX_DOC_EXTRA_PATHS + 1],
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "extra_paths exceeding limit should fail");
        assert!(result.unwrap_err().to_string().contains("extra_paths"));
    }

    #[test]
    fn docs_config_validate_accepts_max_extra_paths() {
        let config = DocsConfig {
            extra_paths: vec!["./docs".to_string(); MAX_DOC_EXTRA_PATHS],
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "extra_paths at maximum should pass");
    }

    #[test]
    fn docs_config_validate_accepts_empty_extra_paths() {
        let config = DocsConfig {
            extra_paths: Vec::new(),
            ..DocsConfig::default()
        };
        assert!(config.validate().is_ok(), "empty extra_paths should pass");
    }

    #[test]
    fn docs_config_validate_rejects_empty_path_in_extra_paths() {
        let config = DocsConfig {
            extra_paths: vec![String::new()],
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "empty path in extra_paths should fail");
    }

    #[test]
    fn docs_config_validate_rejects_whitespace_only_path() {
        let config = DocsConfig {
            extra_paths: vec!["   ".to_string()],
            ..DocsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "whitespace-only path should fail");
    }

    // ============================================================================
    // SECTION: ServerToolsConfig::validate() Tests (16 tests)
    // ============================================================================

    #[test]
    fn server_tools_validate_accepts_default() {
        let config = ServerToolsConfig::default();
        assert!(config.validate().is_ok(), "default ServerToolsConfig should pass");
    }

    #[test]
    fn server_tools_validate_accepts_filter_mode() {
        let config = ServerToolsConfig {
            mode: ToolVisibilityMode::Filter,
            allowlist: vec!["scenario_define".to_string()],
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "filter mode with allowlist should pass");
    }

    #[test]
    fn server_tools_validate_accepts_passthrough_mode() {
        let config = ServerToolsConfig {
            mode: ToolVisibilityMode::Passthrough,
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "passthrough mode should pass");
    }

    // Allowlist validation tests
    #[test]
    fn server_tools_validate_rejects_too_many_allowlist() {
        let config = ServerToolsConfig {
            allowlist: vec!["scenario_define".to_string(); MAX_TOOL_VISIBILITY_RULES + 1],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "too many allowlist entries should fail");
        assert!(result.unwrap_err().to_string().contains("allowlist"));
    }

    #[test]
    fn server_tools_validate_accepts_max_allowlist() {
        let config = ServerToolsConfig {
            allowlist: vec!["scenario_define".to_string(); MAX_TOOL_VISIBILITY_RULES],
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "allowlist at max should pass");
    }

    #[test]
    fn server_tools_validate_rejects_empty_tool_name_allowlist() {
        let config = ServerToolsConfig {
            allowlist: vec![String::new()],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "empty tool name should fail");
        assert!(result.unwrap_err().to_string().contains("non-empty"));
    }

    #[test]
    fn server_tools_validate_rejects_invalid_tool_name_allowlist() {
        let config = ServerToolsConfig {
            allowlist: vec!["invalid_tool_name".to_string()],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "invalid tool name should fail");
        assert!(result.unwrap_err().to_string().contains("unknown tool"));
    }

    #[test]
    fn server_tools_validate_accepts_valid_tool_names_allowlist() {
        let config = ServerToolsConfig {
            allowlist: vec!["scenario_define".to_string(), "scenario_start".to_string()],
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "valid tool names should pass");
    }

    // Denylist validation tests
    #[test]
    fn server_tools_validate_rejects_too_many_denylist() {
        let config = ServerToolsConfig {
            denylist: vec!["scenario_define".to_string(); MAX_TOOL_VISIBILITY_RULES + 1],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "too many denylist entries should fail");
        assert!(result.unwrap_err().to_string().contains("denylist"));
    }

    #[test]
    fn server_tools_validate_accepts_max_denylist() {
        let config = ServerToolsConfig {
            denylist: vec!["scenario_define".to_string(); MAX_TOOL_VISIBILITY_RULES],
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "denylist at max should pass");
    }

    #[test]
    fn server_tools_validate_rejects_empty_tool_name_denylist() {
        let config = ServerToolsConfig {
            denylist: vec![String::new()],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "empty tool name in denylist should fail");
    }

    #[test]
    fn server_tools_validate_rejects_invalid_tool_name_denylist() {
        let config = ServerToolsConfig {
            denylist: vec!["not_a_real_tool".to_string()],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "invalid tool name in denylist should fail");
        assert!(result.unwrap_err().to_string().contains("unknown tool"));
    }

    #[test]
    fn server_tools_validate_accepts_valid_tool_names_denylist() {
        let config = ServerToolsConfig {
            denylist: vec!["scenario_define".to_string()],
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "valid tool name in denylist should pass");
    }

    // Combined validation tests
    #[test]
    fn server_tools_validate_accepts_both_lists_populated() {
        let config = ServerToolsConfig {
            allowlist: vec!["scenario_define".to_string()],
            denylist: vec!["scenario_start".to_string()],
            ..ServerToolsConfig::default()
        };
        assert!(config.validate().is_ok(), "both lists populated with valid tools should pass");
    }

    #[test]
    fn server_tools_validate_accepts_tool_in_both_lists() {
        let config = ServerToolsConfig {
            allowlist: vec!["scenario_define".to_string()],
            denylist: vec!["scenario_define".to_string()],
            ..ServerToolsConfig::default()
        };
        // Validation allows this - denylist takes precedence at runtime
        assert!(config.validate().is_ok(), "tool in both lists should pass validation");
    }

    #[test]
    fn server_tools_validate_whitespace_only_tool_name() {
        let config = ServerToolsConfig {
            allowlist: vec!["   ".to_string()],
            ..ServerToolsConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "whitespace-only tool name should fail");
    }

    // ============================================================================
    // SECTION: validate_timeout_range() Tests (18 tests)
    // ============================================================================

    #[test]
    fn validate_timeout_range_accepts_minimum() {
        let result = validate_timeout_range(
            "test_timeout",
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_ok(), "minimum value should pass");
    }

    #[test]
    fn validate_timeout_range_accepts_maximum() {
        let result = validate_timeout_range(
            "test_timeout",
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_ok(), "maximum value should pass");
    }

    #[test]
    fn validate_timeout_range_accepts_midpoint() {
        let mid = u64::midpoint(MIN_PROVIDER_CONNECT_TIMEOUT_MS, MAX_PROVIDER_CONNECT_TIMEOUT_MS);
        let result = validate_timeout_range(
            "test_timeout",
            mid,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_ok(), "midpoint value should pass");
    }

    #[test]
    fn validate_timeout_range_rejects_below_minimum() {
        let result = validate_timeout_range(
            "test_timeout",
            MIN_PROVIDER_CONNECT_TIMEOUT_MS - 1,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_err(), "value below minimum should fail");
    }

    #[test]
    fn validate_timeout_range_rejects_above_maximum() {
        let result = validate_timeout_range(
            "test_timeout",
            MAX_PROVIDER_CONNECT_TIMEOUT_MS + 1,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_err(), "value above maximum should fail");
    }

    #[test]
    fn validate_timeout_range_rejects_zero_when_min_nonzero() {
        let result = validate_timeout_range(
            "test_timeout",
            0,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_err(), "zero should fail when minimum is non-zero");
    }

    #[test]
    fn validate_timeout_range_min_equals_max_valid() {
        let result = validate_timeout_range("test_timeout", 1000, 1000, 1000);
        assert!(result.is_ok(), "value should pass when min=max=value");
    }

    #[test]
    fn validate_timeout_range_error_includes_field_name() {
        let result = validate_timeout_range(
            "my_custom_field",
            0,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("my_custom_field"), "error should include field name");
    }

    #[test]
    fn validate_timeout_range_error_includes_min_max() {
        let result = validate_timeout_range(
            "test_timeout",
            0,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains(&MIN_PROVIDER_CONNECT_TIMEOUT_MS.to_string()));
        assert!(err_msg.contains(&MAX_PROVIDER_CONNECT_TIMEOUT_MS.to_string()));
    }

    // Test all specific timeout fields
    #[test]
    fn validate_provider_connect_timeout_accepts_valid_range() {
        for timeout_ms in [MIN_PROVIDER_CONNECT_TIMEOUT_MS, 1000, MAX_PROVIDER_CONNECT_TIMEOUT_MS] {
            let result = validate_timeout_range(
                "provider.connect_timeout_ms",
                timeout_ms,
                MIN_PROVIDER_CONNECT_TIMEOUT_MS,
                MAX_PROVIDER_CONNECT_TIMEOUT_MS,
            );
            assert!(result.is_ok(), "provider connect timeout {timeout_ms} should be valid");
        }
    }

    #[test]
    fn validate_provider_request_timeout_accepts_valid_range() {
        for timeout_ms in [MIN_PROVIDER_REQUEST_TIMEOUT_MS, 5000, MAX_PROVIDER_REQUEST_TIMEOUT_MS] {
            let result = validate_timeout_range(
                "provider.request_timeout_ms",
                timeout_ms,
                MIN_PROVIDER_REQUEST_TIMEOUT_MS,
                MAX_PROVIDER_REQUEST_TIMEOUT_MS,
            );
            assert!(result.is_ok(), "provider request timeout {timeout_ms} should be valid");
        }
    }

    #[test]
    fn validate_namespace_auth_connect_timeout_accepts_valid_range() {
        for timeout_ms in
            [MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS, 1000, MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS]
        {
            let result = validate_timeout_range(
                "namespace_auth.connect_timeout_ms",
                timeout_ms,
                MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
                MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            );
            assert!(result.is_ok(), "namespace auth connect timeout {timeout_ms} should be valid");
        }
    }

    #[test]
    fn validate_namespace_auth_request_timeout_accepts_valid_range() {
        for timeout_ms in
            [MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS, 5000, MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS]
        {
            let result = validate_timeout_range(
                "namespace_auth.request_timeout_ms",
                timeout_ms,
                MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
                MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
            );
            assert!(result.is_ok(), "namespace auth request timeout {timeout_ms} should be valid");
        }
    }

    #[test]
    fn validate_provider_connect_timeout_rejects_too_small() {
        let result = validate_timeout_range(
            "provider.connect_timeout_ms",
            MIN_PROVIDER_CONNECT_TIMEOUT_MS - 1,
            MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            MAX_PROVIDER_CONNECT_TIMEOUT_MS,
        );
        assert!(result.is_err(), "too small connect timeout should fail");
    }

    #[test]
    fn validate_provider_request_timeout_rejects_too_large() {
        let result = validate_timeout_range(
            "provider.request_timeout_ms",
            MAX_PROVIDER_REQUEST_TIMEOUT_MS + 1,
            MIN_PROVIDER_REQUEST_TIMEOUT_MS,
            MAX_PROVIDER_REQUEST_TIMEOUT_MS,
        );
        assert!(result.is_err(), "too large request timeout should fail");
    }

    #[test]
    fn validate_timeout_accepts_exact_boundaries() {
        // Test exact minimum
        assert!(validate_timeout_range("test", 100, 100, 1000).is_ok());
        // Test exact maximum
        assert!(validate_timeout_range("test", 1000, 100, 1000).is_ok());
        // Test one below min fails
        assert!(validate_timeout_range("test", 99, 100, 1000).is_err());
        // Test one above max fails
        assert!(validate_timeout_range("test", 1001, 100, 1000).is_err());
    }

    #[test]
    fn validate_rate_limit_window_accepts_valid_range() {
        for window_ms in [MIN_RATE_LIMIT_WINDOW_MS, 1000, MAX_RATE_LIMIT_WINDOW_MS] {
            let result = validate_timeout_range(
                "rate_limit.window_ms",
                window_ms,
                MIN_RATE_LIMIT_WINDOW_MS,
                MAX_RATE_LIMIT_WINDOW_MS,
            );
            assert!(result.is_ok(), "rate limit window {window_ms} should be valid");
        }
    }

    // ============================================================================
    // SECTION: Path Validation Tests (14 tests)
    // ============================================================================

    #[test]
    fn validate_path_string_accepts_valid_path() {
        let result = validate_path_string("test_path", "./docs/config");
        assert!(result.is_ok(), "valid path should pass");
    }

    #[test]
    fn validate_path_string_rejects_empty_string() {
        let result = validate_path_string("test_path", "");
        assert!(result.is_err(), "empty path should fail");
        assert!(result.unwrap_err().to_string().contains("non-empty"));
    }

    #[test]
    fn validate_path_string_rejects_whitespace_only() {
        let result = validate_path_string("test_path", "   ");
        assert!(result.is_err(), "whitespace-only path should fail");
    }

    #[test]
    fn validate_path_string_rejects_exceeds_max_length() {
        let long_path = "a".repeat(MAX_TOTAL_PATH_LENGTH + 1);
        let result = validate_path_string("test_path", &long_path);
        assert!(result.is_err(), "path exceeding max length should fail");
        assert!(result.unwrap_err().to_string().contains("max length"));
    }

    #[test]
    fn validate_path_string_accepts_at_max_length() {
        // Create a path at max length with multiple valid components
        let component = "a".repeat(MAX_PATH_COMPONENT_LENGTH);
        let mut path_parts = Vec::new();
        let mut current_len = 0;

        while current_len + component.len() < MAX_TOTAL_PATH_LENGTH {
            path_parts.push(component.clone());
            current_len += component.len() + 1; // +1 for the separator
        }

        // Add final component to reach exactly max length
        if current_len < MAX_TOTAL_PATH_LENGTH {
            let remaining = MAX_TOTAL_PATH_LENGTH - current_len - 1;
            if remaining > 0 {
                path_parts.push("a".repeat(remaining));
            }
        }

        let max_path = path_parts.join("/");
        let result = validate_path_string("test_path", &max_path);
        assert!(result.is_ok(), "path at max length should pass (len={})", max_path.len());
    }

    #[test]
    fn validate_path_string_rejects_component_too_long() {
        let long_component = "a".repeat(MAX_PATH_COMPONENT_LENGTH + 1);
        let path = format!("./{long_component}");
        let result = validate_path_string("test_path", &path);
        assert!(result.is_err(), "path with too-long component should fail");
        assert!(result.unwrap_err().to_string().contains("component too long"));
    }

    #[test]
    fn validate_path_string_accepts_component_at_max() {
        let max_component = "a".repeat(MAX_PATH_COMPONENT_LENGTH);
        let path = format!("./{max_component}");
        let result = validate_path_string("test_path", &path);
        assert!(result.is_ok(), "path with max-length component should pass");
    }

    #[test]
    fn validate_path_string_accepts_multiple_components() {
        let result = validate_path_string("test_path", "./foo/bar/baz/config.toml");
        assert!(result.is_ok(), "multi-component path should pass");
    }

    #[test]
    fn validate_path_string_trims_before_validation() {
        let result = validate_path_string("test_path", "  ./docs/config  ");
        assert!(result.is_ok(), "trimmed valid path should pass");
    }

    #[test]
    fn validate_path_string_error_includes_field_name() {
        let result = validate_path_string("my_custom_path", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("my_custom_path"));
    }

    #[test]
    fn validate_object_store_prefix_accepts_valid_prefix() {
        let result = validate_object_store_prefix("runpacks/prod");
        assert!(result.is_ok(), "valid prefix should pass");
    }

    #[test]
    fn validate_object_store_prefix_rejects_backslash() {
        let result = validate_object_store_prefix("runpacks\\prod");
        assert!(result.is_err(), "prefix with backslash should fail");
        assert!(result.unwrap_err().to_string().contains("backslash"));
    }

    #[test]
    fn validate_object_store_prefix_rejects_absolute_path() {
        let result = validate_object_store_prefix("/runpacks/prod");
        assert!(result.is_err(), "absolute path should fail");
    }

    #[test]
    fn validate_object_store_prefix_rejects_parent_traversal() {
        let result = validate_object_store_prefix("../runpacks");
        assert!(result.is_err(), "parent traversal should fail");
    }

    // ============================================================================
    // SECTION: ProviderDiscoveryConfig::is_allowed() Tests (12 tests)
    // ============================================================================

    #[test]
    fn is_allowed_returns_true_when_allowlist_empty_and_not_denied() {
        let config = ProviderDiscoveryConfig::default();
        assert!(config.is_allowed("test_provider"), "should allow when no lists configured");
    }

    #[test]
    fn is_allowed_returns_true_when_in_allowlist() {
        let config = ProviderDiscoveryConfig {
            allowlist: vec!["allowed_provider".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(config.is_allowed("allowed_provider"), "should allow when in allowlist");
    }

    #[test]
    fn is_allowed_returns_false_when_in_denylist() {
        let config = ProviderDiscoveryConfig {
            denylist: vec!["denied_provider".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(!config.is_allowed("denied_provider"), "should deny when in denylist");
    }

    #[test]
    fn is_allowed_denylist_takes_precedence_over_allowlist() {
        let config = ProviderDiscoveryConfig {
            allowlist: vec!["provider1".to_string()],
            denylist: vec!["provider1".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(!config.is_allowed("provider1"), "denylist should take precedence");
    }

    #[test]
    fn is_allowed_case_sensitive_match() {
        let config = ProviderDiscoveryConfig {
            allowlist: vec!["MyProvider".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(config.is_allowed("MyProvider"), "exact case should match");
        assert!(!config.is_allowed("myprovider"), "different case should not match");
    }

    #[test]
    fn is_allowed_empty_provider_id() {
        let config = ProviderDiscoveryConfig::default();
        assert!(config.is_allowed(""), "empty id allowed when no restrictions");
    }

    #[test]
    fn is_allowed_both_lists_empty() {
        let config = ProviderDiscoveryConfig {
            allowlist: Vec::new(),
            denylist: Vec::new(),
            ..ProviderDiscoveryConfig::default()
        };
        assert!(config.is_allowed("anything"), "should allow all when both lists empty");
    }

    #[test]
    fn is_allowed_allowlist_only_with_match() {
        let config = ProviderDiscoveryConfig {
            allowlist: vec!["provider1".to_string(), "provider2".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(config.is_allowed("provider1"), "should allow when in allowlist");
    }

    #[test]
    fn is_allowed_allowlist_only_without_match() {
        let config = ProviderDiscoveryConfig {
            allowlist: vec!["provider1".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(!config.is_allowed("provider2"), "should deny when not in allowlist");
    }

    #[test]
    fn is_allowed_denylist_only_with_match() {
        let config = ProviderDiscoveryConfig {
            denylist: vec!["provider1".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(!config.is_allowed("provider1"), "should deny when in denylist");
    }

    #[test]
    fn is_allowed_denylist_only_without_match() {
        let config = ProviderDiscoveryConfig {
            denylist: vec!["provider1".to_string()],
            ..ProviderDiscoveryConfig::default()
        };
        assert!(config.is_allowed("provider2"), "should allow when not in denylist");
    }

    #[test]
    fn is_allowed_fails_closed_on_allowlist() {
        let config = ProviderDiscoveryConfig {
            allowlist: vec!["provider1".to_string()],
            denylist: Vec::new(),
            ..ProviderDiscoveryConfig::default()
        };
        assert!(!config.is_allowed("provider2"), "should fail-closed with allowlist");
    }

    // ============================================================================
    // SECTION: Default Trait Implementation Tests (10 tests)
    // ============================================================================

    #[test]
    fn docs_config_default_passes_validation() {
        let config = DocsConfig::default();
        assert!(config.validate().is_ok(), "DocsConfig::default() should pass validation");
    }

    #[test]
    fn server_tools_config_default_passes_validation() {
        let config = ServerToolsConfig::default();
        assert!(config.validate().is_ok(), "ServerToolsConfig::default() should pass validation");
    }

    #[test]
    fn docs_config_default_has_correct_values() {
        let config = DocsConfig::default();
        assert!(config.enabled, "docs should be enabled by default");
        assert!(config.enable_search, "docs search should be enabled by default");
        assert!(config.enable_resources, "docs resources should be enabled by default");
        assert!(config.include_default_docs, "should include default docs by default");
        assert_eq!(config.max_doc_bytes, DEFAULT_DOC_MAX_BYTES);
        assert_eq!(config.max_total_bytes, DEFAULT_DOC_MAX_TOTAL_BYTES);
        assert_eq!(config.max_docs, DEFAULT_DOC_MAX_DOCS);
        assert_eq!(config.max_sections, DEFAULT_DOC_MAX_SECTIONS);
        assert!(config.extra_paths.is_empty(), "should have no extra paths by default");
    }

    #[test]
    fn server_tools_config_default_has_correct_values() {
        let config = ServerToolsConfig::default();
        assert_eq!(config.mode, ToolVisibilityMode::Filter);
        assert!(config.allowlist.is_empty());
        assert!(config.denylist.is_empty());
    }

    #[test]
    fn tool_visibility_mode_default_is_filter() {
        let mode = ToolVisibilityMode::default();
        assert_eq!(mode, ToolVisibilityMode::Filter);
    }

    #[test]
    fn provider_discovery_config_default_passes_validation() {
        let config = ProviderDiscoveryConfig::default();
        assert!(config.validate().is_ok(), "ProviderDiscoveryConfig::default() should pass");
    }

    #[test]
    fn provider_discovery_config_default_has_empty_lists() {
        let config = ProviderDiscoveryConfig::default();
        assert!(config.allowlist.is_empty());
        assert!(config.denylist.is_empty());
        assert_eq!(config.max_response_bytes, DEFAULT_PROVIDER_DISCOVERY_MAX_BYTES);
    }

    #[test]
    fn decision_gate_config_has_valid_docs_config() {
        // DecisionGateConfig doesn't have Default, so construct with defaults for each field
        let mut config = DecisionGateConfig {
            server: ServerConfig::default(),
            namespace: NamespaceConfig::default(),
            dev: DevConfig::default(),
            trust: TrustConfig::default(),
            evidence: EvidencePolicyConfig::default(),
            anchors: AnchorPolicyConfig::default(),
            provider_discovery: ProviderDiscoveryConfig::default(),
            validation: ValidationConfig::default(),
            policy: PolicyConfig::default(),
            run_state_store: RunStateStoreConfig::default(),
            schema_registry: SchemaRegistryConfig::default(),
            providers: Vec::new(),
            docs: DocsConfig::default(),
            runpack_storage: None,
            source_modified_at: None,
        };
        assert!(config.docs.validate().is_ok(), "default docs config should be valid");
        assert!(config.validate().is_ok(), "constructed config should pass validation");
    }

    #[test]
    fn decision_gate_config_has_valid_server_tools() {
        let config = DecisionGateConfig {
            server: ServerConfig::default(),
            namespace: NamespaceConfig::default(),
            dev: DevConfig::default(),
            trust: TrustConfig::default(),
            evidence: EvidencePolicyConfig::default(),
            anchors: AnchorPolicyConfig::default(),
            provider_discovery: ProviderDiscoveryConfig::default(),
            validation: ValidationConfig::default(),
            policy: PolicyConfig::default(),
            run_state_store: RunStateStoreConfig::default(),
            schema_registry: SchemaRegistryConfig::default(),
            providers: Vec::new(),
            docs: DocsConfig::default(),
            runpack_storage: None,
            source_modified_at: None,
        };
        assert!(config.server.tools.validate().is_ok(), "default server.tools should be valid");
    }

    #[test]
    fn all_defaults_are_fail_closed() {
        // Verify that defaults are secure by default
        let config = DecisionGateConfig {
            server: ServerConfig::default(),
            namespace: NamespaceConfig::default(),
            dev: DevConfig::default(),
            trust: TrustConfig::default(),
            evidence: EvidencePolicyConfig::default(),
            anchors: AnchorPolicyConfig::default(),
            provider_discovery: ProviderDiscoveryConfig::default(),
            validation: ValidationConfig::default(),
            policy: PolicyConfig::default(),
            run_state_store: RunStateStoreConfig::default(),
            schema_registry: SchemaRegistryConfig::default(),
            providers: Vec::new(),
            docs: DocsConfig::default(),
            runpack_storage: None,
            source_modified_at: None,
        };
        assert!(!config.is_dev_permissive(), "should not be dev-permissive by default");
        assert!(!config.allow_default_namespace(), "should not allow default namespace by default");
    }
}
