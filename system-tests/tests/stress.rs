// system-tests/tests/stress.rs
// ============================================================================
// Module: Stress Tests
// Description: Concurrency and burst-load checks for MCP tooling.
// Purpose: Validate resilience under concurrent registry/list/precheck load.
// Dependencies: system-tests helpers
// ============================================================================

//! Stress tests for Decision Gate system-tests.
// TODO: Add fuzz/property tests for schema validation and cursor parsing.
// TODO: Add long-running soak/perf regression tests once infra is ready.

mod helpers;

use std::collections::HashSet;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::Timestamp;
use decision_gate_core::TrustLane;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasListResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use ret_logic::TriState;
use serde_json::json;
use tokio::task::JoinSet;

const CONCURRENT_WRITES: usize = 40;
const LIST_ENTRIES: usize = 50;
const LIST_WORKERS: usize = 8;
const PRECHECK_STORM: usize = 64;

fn schema_record(
    tenant_id: &decision_gate_core::TenantId,
    namespace_id: &decision_gate_core::NamespaceId,
    schema_id: &str,
    version: &str,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id: tenant_id.clone(),
        namespace_id: namespace_id.clone(),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("stress schema".to_string()),
        created_at: Timestamp::Logical(1),
    }
}

async fn list_all_schemas(
    client: &McpHttpClient,
    tenant_id: &decision_gate_core::TenantId,
    namespace_id: &decision_gate_core::NamespaceId,
    limit: usize,
) -> Result<Vec<(String, String)>, String> {
    let mut cursor: Option<String> = None;
    let mut keys = Vec::new();

    loop {
        let request = SchemasListRequest {
            tenant_id: tenant_id.clone(),
            namespace_id: namespace_id.clone(),
            cursor: cursor.clone(),
            limit: Some(limit),
        };
        let input = serde_json::to_value(&request)
            .map_err(|err| format!("serialize schemas_list: {err}"))?;
        let response: SchemasListResponse = client.call_tool_typed("schemas_list", input).await?;
        if response.items.is_empty() {
            if response.next_token.is_some() {
                return Err("schemas_list returned empty page with next_token".to_string());
            }
            break;
        }
        for record in response.items {
            keys.push((record.schema_id.to_string(), record.version.to_string()));
        }
        cursor = response.next_token;
        if cursor.is_none() {
            break;
        }
    }

    Ok(keys)
}

fn ensure_sorted_unique(keys: &[(String, String)]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for window in keys.windows(2) {
        if window[0] > window[1] {
            return Err("schema list ordering unstable".to_string());
        }
    }
    for key in keys {
        if !seen.insert(key.clone()) {
            return Err("duplicate schema entry detected".to_string());
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stress_registry_concurrent_writes() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("stress_registry_concurrent_writes")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(10))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(10)).await?;

    let fixture = ScenarioFixture::time_after("stress-registry", "run-0", 0);
    let tenant_id = fixture.tenant_id.clone();
    let namespace_id = fixture.namespace_id.clone();

    let mut joins = JoinSet::new();
    for idx in 0 .. CONCURRENT_WRITES {
        let client = client.clone();
        let tenant_id = tenant_id.clone();
        let namespace_id = namespace_id.clone();
        joins.spawn(async move {
            let record =
                schema_record(&tenant_id, &namespace_id, &format!("stress-{idx:03}"), "v1");
            let request = SchemasRegisterRequest {
                record,
            };
            let input = serde_json::to_value(&request)
                .map_err(|err| format!("serialize schemas_register: {err}"))?;
            let _: serde_json::Value = client.call_tool_typed("schemas_register", input).await?;
            Ok::<(), String>(())
        });
    }
    while let Some(result) = joins.join_next().await {
        result
            .map_err(|err| format!("join error: {err}"))?
            .map_err(|err| format!("schemas_register failed: {err}"))?;
    }

    let keys = list_all_schemas(&client, &tenant_id, &namespace_id, 25).await?;
    if keys.len() != CONCURRENT_WRITES {
        return Err(format!("expected {CONCURRENT_WRITES} entries, got {}", keys.len()).into());
    }
    ensure_sorted_unique(&keys)?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["concurrent schema registry writes succeeded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stress_schema_list_paging_concurrent_reads() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("stress_schema_list_paging_concurrent_reads")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(10))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(10)).await?;

    let fixture = ScenarioFixture::time_after("stress-list", "run-0", 0);
    let tenant_id = fixture.tenant_id.clone();
    let namespace_id = fixture.namespace_id.clone();

    for idx in 0 .. LIST_ENTRIES {
        let record = schema_record(&tenant_id, &namespace_id, &format!("list-{idx:03}"), "v1");
        let request = SchemasRegisterRequest {
            record,
        };
        let input = serde_json::to_value(&request)?;
        let _: serde_json::Value = client.call_tool_typed("schemas_register", input).await?;
    }

    let baseline = list_all_schemas(&client, &tenant_id, &namespace_id, 7).await?;
    if baseline.len() != LIST_ENTRIES {
        return Err(format!("expected {LIST_ENTRIES} entries, got {}", baseline.len()).into());
    }
    ensure_sorted_unique(&baseline)?;

    let mut joins = JoinSet::new();
    for _ in 0 .. LIST_WORKERS {
        let client = client.clone();
        let tenant_id = tenant_id.clone();
        let namespace_id = namespace_id.clone();
        let baseline = baseline.clone();
        joins.spawn(async move {
            let keys = list_all_schemas(&client, &tenant_id, &namespace_id, 7).await?;
            if keys != baseline {
                return Err("schemas_list returned unstable ordering".to_string());
            }
            Ok::<(), String>(())
        });
    }
    while let Some(result) = joins.join_next().await {
        result
            .map_err(|err| format!("join error: {err}"))?
            .map_err(|err| format!("schemas_list failed: {err}"))?;
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["schema list paging stable under concurrent reads".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stress_precheck_request_storm() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("stress_precheck_request_storm")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(10))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(10)).await?;

    let fixture = ScenarioFixture::time_after("stress-precheck", "run-0", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let record = schema_record(&fixture.tenant_id, &fixture.namespace_id, "asserted", "v1");
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _: serde_json::Value = client.call_tool_typed("schemas_register", register_input).await?;

    let mut joins = JoinSet::new();
    for _ in 0 .. PRECHECK_STORM {
        let client = client.clone();
        let tenant_id = fixture.tenant_id.clone();
        let namespace_id = fixture.namespace_id.clone();
        let scenario_id = define_output.scenario_id.clone();
        let record = record.clone();
        joins.spawn(async move {
            let request = PrecheckToolRequest {
                tenant_id,
                namespace_id,
                scenario_id: Some(scenario_id),
                spec: None,
                stage_id: None,
                data_shape: DataShapeRef {
                    schema_id: record.schema_id.clone(),
                    version: record.version.clone(),
                },
                payload: json!({"after": true}),
            };
            let input = serde_json::to_value(&request)
                .map_err(|err| format!("serialize precheck: {err}"))?;
            let response: PrecheckToolResponse = client.call_tool_typed("precheck", input).await?;
            match response.decision {
                DecisionOutcome::Complete {
                    ..
                } => {}
                other => return Err(format!("unexpected decision: {other:?}")),
            }
            if response.gate_evaluations[0].status != TriState::True {
                return Err("unexpected gate status".to_string());
            }
            Ok::<(), String>(())
        });
    }
    while let Some(result) = joins.join_next().await {
        result
            .map_err(|err| format!("join error: {err}"))?
            .map_err(|err| format!("precheck failed: {err}"))?;
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["precheck request storm handled".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
