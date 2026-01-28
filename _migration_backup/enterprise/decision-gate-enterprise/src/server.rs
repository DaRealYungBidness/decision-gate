// enterprise/decision-gate-enterprise/src/server.rs
// ============================================================================
// Module: Enterprise Server Builder
// Description: Build MCP server with enterprise overrides and audit sinks.
// Purpose: Provide a hardened server assembly path for managed deployments.
// ============================================================================

use std::sync::Arc;

use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::McpAuditSink;
use decision_gate_mcp::McpMetrics;
use decision_gate_mcp::McpServer;
use decision_gate_mcp::NoopMetrics;
use decision_gate_mcp::RunpackStorage;
use decision_gate_mcp::ServerOverrides;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::UsageMeter;
use decision_gate_mcp::server::McpServerError;
use decision_gate_store_enterprise::postgres_store::PostgresStoreConfig;
use decision_gate_store_enterprise::postgres_store::shared_postgres_store;

use crate::config::EnterpriseConfig;

/// Enterprise server overrides and sinks.
pub struct EnterpriseServerOptions {
    /// Tenant authorization policy.
    pub tenant_authorizer: Arc<dyn TenantAuthorizer>,
    /// Usage metering + quota enforcement.
    pub usage_meter: Arc<dyn UsageMeter>,
    /// Optional run state store override.
    pub run_state_store: Option<SharedRunStateStore>,
    /// Optional schema registry override.
    pub schema_registry: Option<SharedDataShapeRegistry>,
    /// Optional runpack storage backend override.
    pub runpack_storage: Option<Arc<dyn RunpackStorage>>,
    /// Audit sink for request logging.
    pub audit_sink: Arc<dyn McpAuditSink>,
    /// Metrics sink for observability.
    pub metrics: Arc<dyn McpMetrics>,
}

impl EnterpriseServerOptions {
    /// Builds default options with no-op metrics.
    #[must_use]
    pub fn new(
        tenant_authorizer: Arc<dyn TenantAuthorizer>,
        usage_meter: Arc<dyn UsageMeter>,
        audit_sink: Arc<dyn McpAuditSink>,
    ) -> Self {
        Self {
            tenant_authorizer,
            usage_meter,
            run_state_store: None,
            schema_registry: None,
            runpack_storage: None,
            audit_sink,
            metrics: Arc::new(NoopMetrics),
        }
    }

    /// Sets storage overrides for run state and schema registry.
    #[must_use]
    pub fn with_storage(
        mut self,
        run_state_store: SharedRunStateStore,
        schema_registry: SharedDataShapeRegistry,
    ) -> Self {
        self.run_state_store = Some(run_state_store);
        self.schema_registry = Some(schema_registry);
        self
    }

    /// Sets the runpack storage backend override.
    #[must_use]
    pub fn with_runpack_storage(mut self, runpack_storage: Arc<dyn RunpackStorage>) -> Self {
        self.runpack_storage = Some(runpack_storage);
        self
    }
}

/// Builds an MCP server with enterprise overrides.
///
/// # Errors
///
/// Returns [`McpServerError`] when initialization fails.
pub fn build_enterprise_server(
    config: DecisionGateConfig,
    options: EnterpriseServerOptions,
) -> Result<McpServer, McpServerError> {
    let overrides = ServerOverrides {
        tenant_authorizer: Some(options.tenant_authorizer),
        usage_meter: Some(options.usage_meter),
        run_state_store: options.run_state_store,
        schema_registry: options.schema_registry,
        runpack_storage: options.runpack_storage,
    };
    McpServer::from_config_with_observability_and_overrides(
        config,
        options.metrics,
        options.audit_sink,
        overrides,
    )
}

/// Builds an MCP server wired to a Postgres-backed store and registry.
///
/// # Errors
///
/// Returns [`McpServerError`] when initialization fails.
pub fn build_enterprise_server_with_postgres(
    config: DecisionGateConfig,
    options: EnterpriseServerOptions,
    store_config: &PostgresStoreConfig,
) -> Result<McpServer, McpServerError> {
    let (run_state_store, schema_registry) =
        shared_postgres_store(store_config).map_err(|err| McpServerError::Init(err.to_string()))?;
    let options = options.with_storage(run_state_store, schema_registry);
    build_enterprise_server(config, options)
}

/// Builds an MCP server from OSS + enterprise configs.
///
/// # Errors
///
/// Returns [`McpServerError`] when initialization fails.
pub fn build_enterprise_server_from_configs(
    config: DecisionGateConfig,
    enterprise_config: &EnterpriseConfig,
    tenant_authorizer: Arc<dyn TenantAuthorizer>,
    audit_sink: Arc<dyn McpAuditSink>,
    metrics: Arc<dyn McpMetrics>,
) -> Result<McpServer, McpServerError> {
    let options = enterprise_config
        .build_server_options_with_metrics(tenant_authorizer, audit_sink, metrics)
        .map_err(|err| McpServerError::Init(err.to_string()))?;
    build_enterprise_server(config, options)
}
