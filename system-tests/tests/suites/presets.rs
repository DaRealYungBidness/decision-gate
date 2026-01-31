// system-tests/tests/suites/presets.rs
// ============================================================================
// Module: Preset Configuration Tests
// Description: Validate bundled preset configs behave as documented.
// Purpose: Ensure onboarding presets are runnable and enforce expected posture.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::runtime::NextRequest;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::RunStateStoreType;
use decision_gate_mcp::config::SchemaRegistryType;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn preset_quickstart_dev_http() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("preset_quickstart_dev_http")?;
    let bind = allocate_bind_addr()?.to_string();
    let temp_dir = TempDir::new()?;
    let mut config = load_preset("quickstart-dev.toml")?;
    configure_paths(&mut config, &bind, &temp_dir);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("preset-quickstart", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    run_basic_scenario(&client, &fixture).await?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["quickstart-dev preset scenario lifecycle passed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn preset_default_recommended_http() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("preset_default_recommended_http")?;
    let bind = allocate_bind_addr()?.to_string();
    let temp_dir = TempDir::new()?;
    let mut config = load_preset("default-recommended.toml")?;
    configure_paths(&mut config, &bind, &temp_dir);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("preset-default", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    run_basic_scenario(&client, &fixture).await?;

    let record = build_schema_record(fixture.tenant_id, fixture.namespace_id, "preset-default");
    let register_request = SchemasRegisterRequest {
        record,
    };
    let register_input = serde_json::to_value(&register_request)?;
    client.call_tool("schemas_register", register_input).await?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["default-recommended preset scenario + registry passed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn preset_hardened_http() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("preset_hardened_http")?;
    let bind = allocate_bind_addr()?.to_string();
    let temp_dir = TempDir::new()?;
    let mut config = load_preset("hardened.toml")?;
    configure_paths(&mut config, &bind, &temp_dir);
    let token = config
        .server
        .auth
        .as_ref()
        .and_then(|auth| auth.bearer_tokens.first())
        .cloned()
        .ok_or("hardened preset missing bearer token")?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.clone());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("preset-hardened", "run-1", 0);
    let tenant_id = TenantId::new(NonZeroU64::new(1).ok_or("tenant id invalid")?);
    let namespace_id = NamespaceId::new(NonZeroU64::new(2).ok_or("namespace id invalid")?);
    fixture.tenant_id = tenant_id;
    fixture.namespace_id = namespace_id;
    fixture.spec.namespace_id = namespace_id;

    run_basic_scenario(&client, &fixture).await?;

    let record = build_schema_record(tenant_id, namespace_id, "preset-hardened");
    let register_request = SchemasRegisterRequest {
        record,
    };
    let register_input = serde_json::to_value(&register_request)?;
    let Err(err) = client.call_tool("schemas_register", register_input).await else {
        return Err("expected schema signing rejection".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["hardened preset scenario passed; signing required enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

fn load_preset(name: &str) -> Result<DecisionGateConfig, Box<dyn std::error::Error>> {
    let mut path = repo_root()?;
    path.push("configs");
    path.push("presets");
    path.push(name);
    Ok(DecisionGateConfig::load(Some(&path))?)
}

fn repo_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(root.parent().ok_or("failed to resolve repo root")?.to_path_buf())
}

fn configure_paths(config: &mut DecisionGateConfig, bind: &str, temp_dir: &TempDir) {
    config.server.bind = Some(bind.to_string());
    if matches!(config.run_state_store.store_type, RunStateStoreType::Sqlite) {
        config.run_state_store.path = Some(temp_dir.path().join("decision-gate.db"));
    }
    if matches!(config.schema_registry.registry_type, SchemaRegistryType::Sqlite) {
        config.schema_registry.path = Some(temp_dir.path().join("schema-registry.db"));
    }
}

fn build_schema_record(
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    schema_id: &str,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new("v1"),
        schema: json!({"type": "object", "additionalProperties": false}),
        description: Some("preset schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}

async fn run_basic_scenario(
    client: &helpers::mcp_client::McpHttpClient,
    fixture: &ScenarioFixture,
) -> Result<(), Box<dyn std::error::Error>> {
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    client.call_tool("scenario_start", start_input).await?;

    let next_request = ScenarioNextRequest {
        scenario_id: define_output.scenario_id,
        request: NextRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "agent-1".to_string(),
            time: Timestamp::Logical(2),
            correlation_id: None,
        },
        feedback: None,
    };
    let next_input = serde_json::to_value(&next_request)?;
    client.call_tool("scenario_next", next_input).await?;
    Ok(())
}
