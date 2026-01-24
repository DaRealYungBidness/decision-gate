// system-tests/tests/smoke.rs
// ============================================================================
// Module: Smoke Tests
// Description: Fast end-to-end sanity checks for MCP tooling.
// Purpose: Validate basic scenario lifecycle over HTTP.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Smoke tests for the Decision Gate MCP HTTP surface.

mod helpers;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::RunStatus;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use ret_logic::TriState;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn smoke_define_start_next_status() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("smoke_define_start_next_status")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = decision_gate_core::TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("smoke-scenario", "run-1", 0);

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
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let next_request = ScenarioNextRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: NextRequest {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "agent-1".to_string(),
            time: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let next_input = serde_json::to_value(&next_request)?;
    let _next_result: decision_gate_core::runtime::NextResult =
        client.call_tool_typed("scenario_next", next_input).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: StatusRequest {
            run_id: fixture.run_id,
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    if status.status != RunStatus::Completed {
        return Err(format!("expected completed status, got {:?}", status.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["scenario lifecycle completed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke_schema_register_precheck() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("smoke_schema_register_precheck")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = decision_gate_core::TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("precheck-scenario", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let record = DataShapeRecord {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("precheck schema".to_string()),
        created_at: Timestamp::Logical(1),
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        scenario_id: Some(define_output.scenario_id.clone()),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"after": true}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let precheck_output: PrecheckToolResponse =
        client.call_tool_typed("precheck", precheck_input).await?;

    match precheck_output.decision {
        DecisionOutcome::Complete {
            stage_id,
        } => {
            if stage_id != fixture.stage_id {
                return Err(format!(
                    "expected stage {}, got {}",
                    fixture.stage_id.as_str(),
                    stage_id.as_str()
                )
                .into());
            }
        }
        other => return Err(format!("unexpected decision: {other:?}").into()),
    }
    let eval = precheck_output
        .gate_evaluations
        .first()
        .ok_or_else(|| "missing gate evaluation".to_string())?;
    if eval.status != TriState::True {
        return Err(format!("expected TriState::True, got {:?}", eval.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["schema register and precheck succeeded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke_schema_registry_max_entries_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("smoke_schema_registry_max_entries_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.schema_registry.max_entries = Some(1);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("registry-limit", "run-1", 0);
    let record_a = DataShapeRecord {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: DataShapeId::new("asserted-a"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("precheck schema".to_string()),
        created_at: Timestamp::Logical(1),
    };
    let record_b = DataShapeRecord {
        schema_id: DataShapeId::new("asserted-b"),
        ..record_a.clone()
    };

    let register_request = SchemasRegisterRequest {
        record: record_a,
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

    let register_request = SchemasRegisterRequest {
        record: record_b,
    };
    let register_input = serde_json::to_value(&register_request)?;
    let Err(err) = client.call_tool("schemas_register", register_input).await else {
        return Err("expected max entries rejection".into());
    };
    if !err.contains("max entries") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["schema registry max entries enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke_precheck_rejects_invalid_payload() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("smoke_precheck_rejects_invalid_payload")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = decision_gate_core::TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("precheck-invalid", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let record = DataShapeRecord {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("precheck schema".to_string()),
        created_at: Timestamp::Logical(1),
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        scenario_id: Some(define_output.scenario_id.clone()),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"after": "not-a-bool"}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let Err(err) = client.call_tool("precheck", precheck_input).await else {
        return Err("expected payload validation failure".into());
    };
    if !err.contains("payload does not match schema") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["precheck rejects invalid payload".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke_precheck_respects_trust_lane_default() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("smoke_precheck_respects_trust_lane_default")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("precheck-trust-default", "run-1", 0);
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let record = DataShapeRecord {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("precheck schema".to_string()),
        created_at: Timestamp::Logical(1),
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id.clone(),
        namespace_id: fixture.namespace_id.clone(),
        scenario_id: Some(define_output.scenario_id.clone()),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"after": true}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let response: PrecheckToolResponse = client.call_tool_typed("precheck", precheck_input).await?;
    match response.decision {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => return Err(format!("unexpected decision: {other:?}").into()),
    }
    let eval =
        response.gate_evaluations.first().ok_or_else(|| "missing gate evaluation".to_string())?;
    if eval.status != TriState::Unknown {
        return Err(format!("expected TriState::Unknown, got {:?}", eval.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["precheck respects default trust lane".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
