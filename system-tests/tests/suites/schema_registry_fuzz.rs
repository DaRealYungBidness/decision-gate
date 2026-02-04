// system-tests/tests/suites/schema_registry_fuzz.rs
// ============================================================================
// Module: Schema Registry Fuzz/Validation Tests
// Description: Deterministic malformed input coverage for registry tooling.
// Purpose: Ensure cursor parsing and schema validation fail closed.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Deterministic malformed input coverage for registry tooling.
//! Purpose: Ensure cursor parsing and schema validation fail closed.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::Timestamp;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

use crate::helpers;

fn schema_record(
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    schema_id: &str,
    version: &str,
    schema: serde_json::Value,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema,
        description: Some("registry fuzz schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn schema_registry_cursor_rejects_invalid_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("schema_registry_cursor_rejects_invalid_inputs")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("cursor-fuzz", "run-1", 0);
    let record = schema_record(
        fixture.tenant_id,
        fixture.namespace_id,
        "cursor-schema",
        "v1",
        json!({
            "type": "object",
            "properties": { "value": { "type": "string" } },
            "required": ["value"]
        }),
    );
    let register = SchemasRegisterRequest {
        record,
    };
    client
        .call_tool_typed::<serde_json::Value>("schemas_register", serde_json::to_value(&register)?)
        .await?;

    let invalid_cursors = vec![
        "not-json".to_string(),
        "{\"schema_id\":1,\"version\":\"v1\"}".to_string(),
        "{\"schema_id\":\"a\"}".to_string(),
        format!("\"{}\"", "a".repeat(4096)),
    ];
    for cursor in invalid_cursors {
        let request = SchemasListRequest {
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            cursor: Some(cursor),
            limit: Some(10),
        };
        let input = serde_json::to_value(&request)?;
        let Err(err) = client.call_tool("schemas_list", input).await else {
            return Err("expected invalid cursor rejection".into());
        };
        if !err.contains("invalid cursor") {
            return Err(format!("unexpected error for invalid cursor: {err}").into());
        }
    }

    for limit in [0usize, 10_000usize] {
        let request = SchemasListRequest {
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            cursor: None,
            limit: Some(limit),
        };
        let input = serde_json::to_value(&request)?;
        let Err(err) = client.call_tool("schemas_list", input).await else {
            return Err("expected invalid limit rejection".into());
        };
        if !err.contains("limit must be between") {
            return Err(format!("unexpected limit error: {err}").into());
        }
    }

    let request = SchemasListRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        cursor: None,
        limit: Some(10),
    };
    let input = serde_json::to_value(&request)?;
    let response: decision_gate_mcp::tools::SchemasListResponse =
        client.call_tool_typed("schemas_list", input).await?;
    if response.items.len() != 1 {
        return Err(format!(
            "expected 1 schema after invalid cursor tests, got {}",
            response.items.len()
        )
        .into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["invalid schema registry cursors and limits rejected".to_string()],
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
#[allow(clippy::too_many_lines, reason = "Single flow covers schema + precheck rejection paths.")]
async fn schema_registry_invalid_schema_and_precheck_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("schema_registry_invalid_schema_and_precheck_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("schema-precheck", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let invalid_record = schema_record(
        fixture.tenant_id,
        fixture.namespace_id,
        "invalid-schema",
        "v1",
        json!({
            "type": "object",
            "properties": {
                "value": { "type": "nope" }
            }
        }),
    );
    let invalid_register = SchemasRegisterRequest {
        record: invalid_record.clone(),
    };
    let invalid_input = serde_json::to_value(&invalid_register)?;
    let Err(err) = client.call_tool("schemas_register", invalid_input).await else {
        return Err("expected invalid schema rejection".into());
    };
    if !err.contains("invalid schema") {
        return Err(format!("unexpected invalid schema error: {err}").into());
    }

    let get_request = SchemasGetRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        schema_id: invalid_record.schema_id.clone(),
        version: invalid_record.version.clone(),
    };
    let Err(err) = client.call_tool("schemas_get", serde_json::to_value(&get_request)?).await
    else {
        return Err("expected invalid schema to be absent".into());
    };
    if !err.contains("not found") {
        return Err(format!("unexpected schemas_get error: {err}").into());
    }

    let valid_record = schema_record(
        fixture.tenant_id,
        fixture.namespace_id,
        "asserted",
        "v1",
        json!({
            "type": "object",
            "properties": {
                "value": { "type": "string" }
            },
            "required": ["value"],
            "additionalProperties": false
        }),
    );
    let register_request = SchemasRegisterRequest {
        record: valid_record.clone(),
    };
    client
        .call_tool_typed::<serde_json::Value>(
            "schemas_register",
            serde_json::to_value(&register_request)?,
        )
        .await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        scenario_id: Some(define_output.scenario_id),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: valid_record.schema_id.clone(),
            version: valid_record.version.clone(),
        },
        payload: json!({ "missing": "value" }),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let Err(err) = client.call_tool("precheck", precheck_input).await else {
        return Err("expected precheck payload validation failure".into());
    };
    if !err.contains("payload does not match schema") {
        return Err(format!("unexpected precheck validation error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["invalid schema and payloads rejected".to_string()],
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
