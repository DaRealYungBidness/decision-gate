//! Enterprise runpack hardening system tests.
// enterprise-system-tests/tests/runpack_hardening.rs
// ============================================================================
// Module: Runpack Export Hardening Tests
// Description: Validate runpack export temporary cleanup behavior.
// Purpose: Ensure managed runpack export does not leak temp artifacts.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_enterprise::server::EnterpriseServerOptions;
use decision_gate_enterprise::tenant_authz::MappedTenantAuthorizer;
use decision_gate_enterprise::tenant_authz::NamespaceScope;
use decision_gate_enterprise::tenant_authz::PrincipalScope;
use decision_gate_enterprise::tenant_authz::TenantAuthzPolicy;
use decision_gate_enterprise::tenant_authz::TenantScope;
use decision_gate_enterprise::usage::InMemoryUsageLedger;
use decision_gate_enterprise::usage::QuotaPolicy;
use decision_gate_enterprise::usage::UsageQuotaEnforcer;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::RunpackStorage;
use decision_gate_mcp::RunpackStorageError;
use decision_gate_mcp::RunpackStorageKey;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_enterprise_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;

#[tokio::test(flavor = "multi_thread")]
async fn runpack_export_temporary_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_export_temporary_cleanup")?;

    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);

    let tenant_policy = TenantAuthzPolicy {
        principals: vec![PrincipalScope {
            principal_id: "loopback".to_string(),
            tenants: vec![TenantScope {
                tenant_id: TenantId::new("tenant-1"),
                namespaces: NamespaceScope::All,
            }],
        }],
        require_tenant: true,
    };
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(tenant_policy));
    let usage_meter =
        Arc::new(UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), QuotaPolicy::default()));
    let audit_sink = Arc::new(McpNoopAuditSink);

    let tracker = Arc::new(TempTrackingRunpackStorage::default());
    let options = EnterpriseServerOptions::new(tenant_authorizer, usage_meter, audit_sink)
        .with_runpack_storage(tracker.clone());

    let server = spawn_enterprise_server(config, options).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("cleanup", "run-1", 0);
    fixture.spec.default_tenant_id = Some(TenantId::new("tenant-1"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let _: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", serde_json::to_value(&define_request)?).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: fixture.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(0),
        issue_entry_packets: false,
    };
    let _: serde_json::Value =
        client.call_tool("scenario_start", serde_json::to_value(&start_request)?).await?;

    let export_request = RunpackExportRequest {
        scenario_id: fixture.scenario_id.clone(),
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: fixture.run_id.clone(),
        output_dir: None,
        manifest_name: None,
        generated_at: Timestamp::Logical(0),
        include_verification: false,
    };
    let _: serde_json::Value =
        client.call_tool("runpack_export", serde_json::to_value(&export_request)?).await?;

    let temp_path = tracker.last_path().ok_or("runpack storage did not record temp dir")?;
    if temp_path.exists() {
        return Err("expected runpack temp directory to be removed".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["runpack export temp cleanup verified".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[derive(Clone, Default)]
struct TempTrackingRunpackStorage {
    last_path: Arc<Mutex<Option<PathBuf>>>,
}

impl TempTrackingRunpackStorage {
    fn last_path(&self) -> Option<PathBuf> {
        self.last_path.lock().ok().and_then(|guard| guard.clone())
    }
}

impl RunpackStorage for TempTrackingRunpackStorage {
    fn store_runpack(
        &self,
        _key: &RunpackStorageKey,
        source_dir: &std::path::Path,
    ) -> Result<Option<String>, RunpackStorageError> {
        if !source_dir.exists() {
            return Err(RunpackStorageError::Backend("runpack temp dir missing".to_string()));
        }
        let mut guard = self
            .last_path
            .lock()
            .map_err(|_| RunpackStorageError::Backend("temp tracker lock poisoned".to_string()))?;
        *guard = Some(source_dir.to_path_buf());
        Ok(None)
    }
}
