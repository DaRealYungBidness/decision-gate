// system-tests/tests/suites/sqlite_registry_runpack.rs
// ============================================================================
// Module: SQLite Registry + Runpack Tests
// Description: End-to-end persistence checks for sqlite registry and run state.
// Purpose: Ensure registry entries and run state survive restarts and runpack export works.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! SQLite registry + runpack persistence tests for Decision Gate system-tests.

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::Timestamp;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::RunStateStoreType;
use decision_gate_mcp::config::SchemaRegistryType;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn sqlite_registry_and_runpack_persist_across_restart()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sqlite_registry_and_runpack_persist_across_restart")?;
    let temp_dir = TempDir::new()?;
    let run_state_path = temp_dir.path().join("run_state.sqlite");
    let registry_path = temp_dir.path().join("registry.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: Some(run_state_path.clone()),
        busy_timeout_ms: 5_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };
    config.schema_registry.registry_type = SchemaRegistryType::Sqlite;
    config.schema_registry.path = Some(registry_path.clone());
    config.schema_registry.max_schema_bytes = 1024;

    let server = spawn_mcp_server(config.clone()).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("sqlite-runpack", "run-1", 0);
    let record = DataShapeRecord {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: DataShapeId::new("persisted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "value": { "type": "string" }
            },
            "required": ["value"]
        }),
        description: Some("sqlite registry schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    client
        .call_tool_typed::<serde_json::Value>(
            "schemas_register",
            serde_json::to_value(&register_request)?,
        )
        .await?;

    let oversize_record = DataShapeRecord {
        schema: json!({
            "type": "object",
            "description": "x".repeat(2048)
        }),
        ..record.clone()
    };
    let oversize_request = SchemasRegisterRequest {
        record: oversize_record,
    };
    let Err(err) =
        client.call_tool("schemas_register", serde_json::to_value(&oversize_request)?).await
    else {
        return Err("expected oversize schema rejection".into());
    };
    if !err.contains("schema exceeds size limit") {
        return Err(format!("unexpected oversize schema error: {err}").into());
    }

    let mut spec = fixture.spec.clone();
    spec.default_tenant_id = Some(fixture.tenant_id.clone());
    let define_request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec: spec.clone(),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            serde_json::to_value(&define_request)?,
        )
        .await?;
    let start_request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: spec.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(2),
        issue_entry_packets: false,
    };
    client
        .call_tool_typed::<decision_gate_core::RunState>(
            "scenario_start",
            serde_json::to_value(&start_request)?,
        )
        .await?;
    let trigger_request = decision_gate_mcp::tools::ScenarioTriggerRequest {
        scenario_id: spec.scenario_id.clone(),
        trigger: fixture.trigger_event("trigger-1", Timestamp::Logical(3)),
    };
    client
        .call_tool_typed::<decision_gate_core::runtime::TriggerResult>(
            "scenario_trigger",
            serde_json::to_value(&trigger_request)?,
        )
        .await?;

    server.shutdown().await;

    let bind2 = allocate_bind_addr()?.to_string();
    config.server.bind = Some(bind2.clone());
    let server2 = spawn_mcp_server(config).await?;
    let client2 = server2.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client2, std::time::Duration::from_secs(5)).await?;

    let define_request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec: spec.clone(),
    };
    client2
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            serde_json::to_value(&define_request)?,
        )
        .await?;

    let get_request = SchemasGetRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: record.schema_id.clone(),
        version: record.version.clone(),
    };
    client2
        .call_tool_typed::<decision_gate_mcp::tools::SchemasGetResponse>(
            "schemas_get",
            serde_json::to_value(&get_request)?,
        )
        .await?;

    let runpack_request = RunpackExportRequest {
        scenario_id: spec.scenario_id.clone(),
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        run_id: fixture.run_id.clone(),
        output_dir: Some(reporter.artifacts().runpack_dir().display().to_string()),
        manifest_name: Some("runpack.json".to_string()),
        generated_at: Timestamp::Logical(4),
        include_verification: true,
    };
    let response: decision_gate_mcp::tools::RunpackExportResponse =
        client2.call_tool_typed("runpack_export", serde_json::to_value(&runpack_request)?).await?;
    if response.report.is_none() {
        return Err("expected runpack verification report".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client2.transcript())?;
    reporter.finish(
        "pass",
        vec!["sqlite registry and runpack state persist across restart".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    server2.shutdown().await;
    Ok(())
}
