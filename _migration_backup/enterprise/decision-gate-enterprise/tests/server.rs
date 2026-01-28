// enterprise/decision-gate-enterprise/tests/server.rs
// ============================================================================
// Module: Enterprise Server Tests
// Description: Unit tests for enterprise server options builder.
// Purpose: Validate builder pattern and default field population.
// ============================================================================

//! Enterprise server builder unit tests.

use std::sync::Arc;

use decision_gate_enterprise::server::EnterpriseServerOptions;
use decision_gate_enterprise::usage::InMemoryUsageLedger;
use decision_gate_enterprise::usage::QuotaPolicy;
use decision_gate_enterprise::usage::UsageQuotaEnforcer;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::NoopTenantAuthorizer;
use decision_gate_mcp::RunpackStorage;
use decision_gate_mcp::RunpackStorageError;
use decision_gate_mcp::RunpackStorageKey;

fn noop_meter() -> Arc<UsageQuotaEnforcer<InMemoryUsageLedger>> {
    Arc::new(UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), QuotaPolicy::default()))
}

struct StubRunpackStorage;
impl RunpackStorage for StubRunpackStorage {
    fn store_runpack(
        &self,
        _key: &RunpackStorageKey,
        _source_dir: &std::path::Path,
    ) -> Result<Option<String>, RunpackStorageError> {
        Ok(None)
    }
}

#[test]
fn enterprise_server_options_defaults() {
    let options = EnterpriseServerOptions::new(
        Arc::new(NoopTenantAuthorizer),
        noop_meter(),
        Arc::new(McpNoopAuditSink),
    );
    assert!(options.run_state_store.is_none());
    assert!(options.schema_registry.is_none());
    assert!(options.runpack_storage.is_none());
}

#[test]
fn enterprise_server_options_with_runpack_storage() {
    let options = EnterpriseServerOptions::new(
        Arc::new(NoopTenantAuthorizer),
        noop_meter(),
        Arc::new(McpNoopAuditSink),
    )
    .with_runpack_storage(Arc::new(StubRunpackStorage));
    assert!(options.runpack_storage.is_some());
}
