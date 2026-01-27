// enterprise/decision-gate-enterprise/src/config.rs
// ============================================================================
// Module: Enterprise Config
// Description: Config loader + wiring for enterprise storage and usage.
// Purpose: Provide config-driven wiring for Postgres, S3, and usage ledgers.
// ============================================================================

//! Enterprise configuration loader and wiring helpers.

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use decision_gate_mcp::McpAuditSink;
use decision_gate_mcp::McpMetrics;
use decision_gate_mcp::NoopMetrics;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::UsageMeter;
use decision_gate_store_enterprise::postgres_store::PostgresStoreConfig;
use decision_gate_store_enterprise::postgres_store::shared_postgres_store;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStore;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStoreConfig;
use serde::Deserialize;
use thiserror::Error;

use crate::runpack_storage::S3RunpackStorage;
use crate::server::EnterpriseServerOptions;
use crate::usage::InMemoryUsageLedger;
use crate::usage::QuotaPolicy;
use crate::usage::UsageQuotaEnforcer;
use crate::usage_sqlite::SqliteUsageLedger;

/// Default enterprise config filename.
const DEFAULT_CONFIG_NAME: &str = "decision-gate-enterprise.toml";
/// Environment variable override for config path.
const CONFIG_ENV_VAR: &str = "DECISION_GATE_ENTERPRISE_CONFIG";
/// Maximum allowed config file size in bytes.
const MAX_CONFIG_FILE_SIZE: usize = 512 * 1024;
/// Maximum total path length for config-related paths.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;
/// Maximum length of a single path component.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;

/// Enterprise configuration file.
#[derive(Debug, Clone, Deserialize)]
pub struct EnterpriseConfig {
    /// Storage configuration.
    #[serde(default)]
    pub storage: EnterpriseStorageConfig,
    /// Runpack storage configuration.
    #[serde(default)]
    pub runpacks: EnterpriseRunpackConfig,
    /// Usage metering + quota configuration.
    #[serde(default)]
    pub usage: EnterpriseUsageConfig,
    /// Optional config source metadata (not serialized).
    #[serde(skip)]
    pub source_modified_at: Option<SystemTime>,
}

impl EnterpriseConfig {
    /// Loads enterprise configuration from disk.
    ///
    /// # Errors
    ///
    /// Returns [`EnterpriseConfigError`] when loading or validation fails.
    pub fn load(path: Option<&Path>) -> Result<Self, EnterpriseConfigError> {
        let resolved = resolve_path(path)?;
        validate_path(&resolved)?;
        let bytes =
            fs::read(&resolved).map_err(|err| EnterpriseConfigError::Io(err.to_string()))?;
        if bytes.len() > MAX_CONFIG_FILE_SIZE {
            return Err(EnterpriseConfigError::Invalid(
                "enterprise config file exceeds size limit".to_string(),
            ));
        }
        let content = std::str::from_utf8(&bytes).map_err(|_| {
            EnterpriseConfigError::Invalid("enterprise config must be utf-8".to_string())
        })?;
        let mut config: Self =
            toml::from_str(content).map_err(|err| EnterpriseConfigError::Parse(err.to_string()))?;
        config.source_modified_at = fs::metadata(&resolved).and_then(|meta| meta.modified()).ok();
        config.validate()?;
        Ok(config)
    }

    /// Validates enterprise configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EnterpriseConfigError`] when configuration is invalid.
    pub fn validate(&self) -> Result<(), EnterpriseConfigError> {
        if let Some(postgres) = &self.storage.postgres
            && postgres.connection.trim().is_empty()
        {
            return Err(EnterpriseConfigError::Invalid(
                "postgres connection string is required".to_string(),
            ));
        }
        if let Some(path) = &self.usage.ledger.sqlite_path {
            validate_store_path(path)?;
        }
        if self.usage.ledger.ledger_type == UsageLedgerType::Sqlite
            && self.usage.ledger.sqlite_path.is_none()
        {
            return Err(EnterpriseConfigError::Invalid(
                "sqlite usage ledger requires sqlite_path".to_string(),
            ));
        }
        Ok(())
    }

    /// Builds enterprise server options from configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EnterpriseConfigError`] when wiring fails.
    pub fn build_server_options(
        &self,
        tenant_authorizer: Arc<dyn TenantAuthorizer>,
        audit_sink: Arc<dyn McpAuditSink>,
    ) -> Result<EnterpriseServerOptions, EnterpriseConfigError> {
        self.build_server_options_with_metrics(tenant_authorizer, audit_sink, Arc::new(NoopMetrics))
    }

    /// Builds enterprise server options with a custom metrics sink.
    ///
    /// # Errors
    ///
    /// Returns [`EnterpriseConfigError`] when wiring fails.
    pub fn build_server_options_with_metrics(
        &self,
        tenant_authorizer: Arc<dyn TenantAuthorizer>,
        audit_sink: Arc<dyn McpAuditSink>,
        metrics: Arc<dyn McpMetrics>,
    ) -> Result<EnterpriseServerOptions, EnterpriseConfigError> {
        let usage_meter = build_usage_meter(&self.usage)?;
        let mut options = EnterpriseServerOptions::new(tenant_authorizer, usage_meter, audit_sink);
        options.metrics = metrics;
        if let Some(postgres_config) = &self.storage.postgres {
            let (run_state_store, schema_registry) = shared_postgres_store(postgres_config)
                .map_err(|err| EnterpriseConfigError::Storage(err.to_string()))?;
            options = options.with_storage(run_state_store, schema_registry);
        }
        if let Some(s3_config) = &self.runpacks.s3 {
            let s3_store = S3RunpackStore::new(s3_config.clone())
                .map_err(|err| EnterpriseConfigError::Storage(err.to_string()))?;
            let adapter = S3RunpackStorage::new(s3_store);
            options.runpack_storage = Some(Arc::new(adapter));
        }
        Ok(options)
    }
}

/// Storage configuration for enterprise wiring.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EnterpriseStorageConfig {
    /// Postgres store configuration.
    pub postgres: Option<PostgresStoreConfig>,
}

/// Runpack storage configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EnterpriseRunpackConfig {
    /// S3 runpack store configuration.
    pub s3: Option<S3RunpackStoreConfig>,
}

/// Usage metering configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EnterpriseUsageConfig {
    /// Usage ledger configuration.
    #[serde(default)]
    pub ledger: UsageLedgerConfig,
    /// Quota policy for usage metering.
    #[serde(default)]
    pub quotas: QuotaPolicy,
}

/// Usage ledger backend selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageLedgerType {
    /// In-memory ledger (dev/test only).
    Memory,
    /// SQLite-backed ledger.
    #[default]
    Sqlite,
}

/// Usage ledger configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct UsageLedgerConfig {
    /// Ledger backend type.
    #[serde(default)]
    pub ledger_type: UsageLedgerType,
    /// `SQLite` path for the ledger.
    #[serde(default)]
    pub sqlite_path: Option<PathBuf>,
}

impl Default for UsageLedgerConfig {
    fn default() -> Self {
        Self {
            ledger_type: UsageLedgerType::Sqlite,
            sqlite_path: None,
        }
    }
}

/// Enterprise config errors.
#[derive(Debug, Error)]
pub enum EnterpriseConfigError {
    /// I/O error.
    #[error("enterprise config io error: {0}")]
    Io(String),
    /// Parse error.
    #[error("enterprise config parse error: {0}")]
    Parse(String),
    /// Invalid configuration.
    #[error("enterprise config invalid: {0}")]
    Invalid(String),
    /// Storage wiring error.
    #[error("enterprise storage error: {0}")]
    Storage(String),
}

fn build_usage_meter(
    config: &EnterpriseUsageConfig,
) -> Result<Arc<dyn UsageMeter>, EnterpriseConfigError> {
    let meter: Arc<dyn UsageMeter> = match config.ledger.ledger_type {
        UsageLedgerType::Memory => {
            Arc::new(UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), config.quotas.clone()))
        }
        UsageLedgerType::Sqlite => {
            let path = config.ledger.sqlite_path.as_ref().ok_or_else(|| {
                EnterpriseConfigError::Invalid("sqlite_path is required".to_string())
            })?;
            let ledger = SqliteUsageLedger::new(path)
                .map_err(|err| EnterpriseConfigError::Storage(err.to_string()))?;
            Arc::new(UsageQuotaEnforcer::new(ledger, config.quotas.clone()))
        }
    };
    Ok(meter)
}

/// Resolves the config path from explicit input or environment.
fn resolve_path(path: Option<&Path>) -> Result<PathBuf, EnterpriseConfigError> {
    if let Some(path) = path {
        return Ok(path.to_path_buf());
    }
    if let Ok(env_path) = env::var(CONFIG_ENV_VAR) {
        if env_path.len() > MAX_TOTAL_PATH_LENGTH {
            return Err(EnterpriseConfigError::Invalid(
                "enterprise config path exceeds max length".to_string(),
            ));
        }
        return Ok(PathBuf::from(env_path));
    }
    Ok(PathBuf::from(DEFAULT_CONFIG_NAME))
}

/// Validates the config file path length and components.
fn validate_path(path: &Path) -> Result<(), EnterpriseConfigError> {
    let text = path.to_string_lossy();
    if text.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(EnterpriseConfigError::Invalid(
            "enterprise config path exceeds max length".to_string(),
        ));
    }
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if value.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(EnterpriseConfigError::Invalid(
                "enterprise config path component too long".to_string(),
            ));
        }
    }
    Ok(())
}

/// Validates store paths used by enterprise storage backends.
fn validate_store_path(path: &Path) -> Result<(), EnterpriseConfigError> {
    let text = path.to_string_lossy();
    if text.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(EnterpriseConfigError::Invalid(
            "enterprise store path exceeds max length".to_string(),
        ));
    }
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if value.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(EnterpriseConfigError::Invalid(
                "enterprise store path component too long".to_string(),
            ));
        }
    }
    Ok(())
}
