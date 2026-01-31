// system-tests/tests/suites/store_persistence.rs
// ============================================================================
// Module: Store Persistence Tests
// Description: End-to-end persistence validation for SQLite store backend.
// Purpose: Ensure runs survive server restarts with durable storage enabled.
// Dependencies: system-tests helpers
// ============================================================================

//! `SQLite` run state persistence tests.

use decision_gate_core::RunState;
use decision_gate_core::Timestamp;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::RunStateStoreType;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn sqlite_run_state_persists_across_restart() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sqlite_run_state_persists_across_restart")?;
    let temp = TempDir::new()?;
    let db_path = temp.path().join("run_state.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: Some(db_path.clone()),
        busy_timeout_ms: 5_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };

    let server = spawn_mcp_server(config.clone()).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("sqlite-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            define_input,
        )
        .await?;

    let start_request = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: RunState = client.call_tool_typed("scenario_start", start_input).await?;

    server.shutdown().await;

    let bind2 = allocate_bind_addr()?.to_string();
    config.server.bind = Some(bind2.clone());
    let server2 = spawn_mcp_server(config).await?;
    let client2 = server2.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client2, std::time::Duration::from_secs(5)).await?;

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    client2
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            define_input,
        )
        .await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: StatusRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client2.call_tool_typed("scenario_status", status_input).await?;
    if status.run_id != fixture.run_id {
        return Err(format!(
            "expected run_id {}, got {}",
            fixture.run_id.as_str(),
            status.run_id.as_str()
        )
        .into());
    }

    let mut transcript = client.transcript();
    transcript.extend(client2.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["sqlite run state persists across restarts".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server2.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Restart redefine workflow is clearer as one scenario.")]
async fn sqlite_requires_redefine_after_restart() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sqlite_requires_redefine_after_restart")?;
    let temp = TempDir::new()?;
    let db_path = temp.path().join("run_state.sqlite");

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: Some(db_path.clone()),
        busy_timeout_ms: 5_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };

    let server = spawn_mcp_server(config.clone()).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("sqlite-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            define_input,
        )
        .await?;

    let start_request = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: RunState = client.call_tool_typed("scenario_start", start_input).await?;

    server.shutdown().await;

    let bind2 = allocate_bind_addr()?.to_string();
    config.server.bind = Some(bind2.clone());
    let server2 = spawn_mcp_server(config).await?;
    let client2 = server2.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client2, std::time::Duration::from_secs(5)).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: StatusRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let Err(_) = client2.call_tool("scenario_status", status_input).await else {
        return Err("expected scenario_status to fail before redefine".into());
    };

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    client2
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            define_input,
        )
        .await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: StatusRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client2.call_tool_typed("scenario_status", status_input).await?;
    if status.run_id != fixture.run_id {
        return Err(format!(
            "expected run_id {}, got {}",
            fixture.run_id.as_str(),
            status.run_id.as_str()
        )
        .into());
    }

    let mut transcript = client.transcript();
    transcript.extend(client2.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["restart requires re-define before status".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server2.shutdown().await;
    Ok(())
}
