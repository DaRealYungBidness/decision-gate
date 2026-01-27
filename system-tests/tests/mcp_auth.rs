// system-tests/tests/mcp_auth.rs
// ============================================================================
// Module: MCP Auth Tests
// Description: System tests for MCP tool call authentication and authorization.
// Purpose: Validate bearer auth and tool allowlist enforcement end-to-end.
// Dependencies: system-tests helpers
// ============================================================================

//! MCP auth system tests.

mod helpers;

use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::Timestamp;
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config_with_bearer;
use helpers::harness::base_http_config_with_mtls;
use helpers::harness::base_sse_config_with_bearer;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::TranscriptEntry;
use helpers::readiness::wait_for_ready;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn http_bearer_token_required() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_bearer_token_required")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config_with_bearer(&bind, "test-token");
    let server = spawn_mcp_server(config).await?;

    let authorized =
        server.client(Duration::from_secs(5))?.with_bearer_token("test-token".to_string());
    wait_for_server_ready(&authorized, Duration::from_secs(5)).await?;

    let unauthorized = server.client(Duration::from_secs(5))?;
    let Err(err) = unauthorized.list_tools().await else {
        return Err("expected auth failure".into());
    };
    if !err.contains("unauthenticated") {
        return Err(format!("expected unauthenticated error, got: {err}").into());
    }

    let tools = authorized.list_tools().await?;
    if tools.is_empty() {
        return Err("expected non-empty tools list".into());
    }

    let mut transcript = unauthorized.transcript();
    transcript.extend(authorized.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["bearer auth required for MCP tools".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_tool_allowlist_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_tool_allowlist_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config_with_bearer(&bind, "allowlist-token");
    if let Some(auth) = config.server.auth.as_mut() {
        auth.allowed_tools = vec!["scenario_define".to_string(), "scenario_start".to_string()];
    }
    let server = spawn_mcp_server(config).await?;
    let client =
        server.client(Duration::from_secs(5))?.with_bearer_token("allowlist-token".to_string());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("allowlist-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id.clone());
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

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: StatusRequest {
            run_id: fixture.run_id,
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let Err(err) = client.call_tool("scenario_status", status_input).await else {
        return Err("expected allowlist denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized error, got: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["tool allowlist enforced for MCP calls".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_tool_allowlist_blocks_precheck() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_tool_allowlist_blocks_precheck")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config_with_bearer(&bind, "allowlist-token");
    config.trust.min_lane = TrustLane::Asserted;
    if let Some(auth) = config.server.auth.as_mut() {
        auth.allowed_tools = vec!["scenario_define".to_string(), "schemas_register".to_string()];
    }
    let server = spawn_mcp_server(config).await?;
    let client =
        server.client(Duration::from_secs(5))?.with_bearer_token("allowlist-token".to_string());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("allowlist-precheck", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id.clone());
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
        signing: None,
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
    let Err(err) = client.call_tool("precheck", precheck_input).await else {
        return Err("expected allowlist denial".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("expected unauthorized error, got: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["tool allowlist blocks precheck".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_mtls_subject_required() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_mtls_subject_required")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config_with_mtls(&bind, "CN=decision-gate-client,O=Example");
    let server = spawn_mcp_server(config).await?;

    let authorized = server
        .client(Duration::from_secs(5))?
        .with_client_subject("CN=decision-gate-client,O=Example".to_string());
    wait_for_server_ready(&authorized, Duration::from_secs(5)).await?;

    let unauthorized = server.client(Duration::from_secs(5))?;
    let Err(err) = unauthorized.list_tools().await else {
        return Err("expected auth failure".into());
    };
    if !err.contains("unauthenticated") {
        return Err(format!("expected unauthenticated error, got: {err}").into());
    }

    let tools = authorized.list_tools().await?;
    if tools.is_empty() {
        return Err("expected non-empty tools list".into());
    }

    let mut transcript = unauthorized.transcript();
    transcript.extend(authorized.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["mTLS subject auth required for MCP tools".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct JsonRpcError {
    message: String,
}

#[tokio::test(flavor = "multi_thread")]
async fn sse_bearer_token_required() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sse_bearer_token_required")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_sse_config_with_bearer(&bind, "sse-token");
    let server = spawn_mcp_server(config).await?;
    let base_url = server.base_url().to_string();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    wait_for_ready(
        || async { send_sse_request(&base_url, &request, None).await.map(|_| ()) },
        Duration::from_secs(5),
        "sse server",
    )
    .await?;

    let unauthorized = send_sse_request(&base_url, &request, None).await?;
    let unauthorized_error = unauthorized
        .error
        .as_ref()
        .ok_or_else(|| "missing error for unauthorized request".to_string())?;
    if !unauthorized_error.message.contains("unauthenticated") {
        return Err(
            format!("expected unauthenticated error, got: {}", unauthorized_error.message).into()
        );
    }

    let authorized = send_sse_request(&base_url, &request, Some("sse-token".to_string())).await?;
    let tools =
        authorized.result.as_ref().ok_or_else(|| "missing result for tools/list".to_string())?;
    if tools.get("tools").is_none() {
        return Err("expected tools/list response to include tools".into());
    }

    let transcript = vec![
        TranscriptEntry {
            sequence: 1,
            method: "tools/list".to_string(),
            request: request.clone(),
            response: serde_json::to_value(&unauthorized).unwrap_or(Value::Null),
            error: Some(unauthorized_error.message.clone()),
        },
        TranscriptEntry {
            sequence: 2,
            method: "tools/list".to_string(),
            request: request.clone(),
            response: serde_json::to_value(&authorized).unwrap_or(Value::Null),
            error: None,
        },
    ];
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["sse bearer auth enforced for tools/list".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

async fn send_sse_request(
    base_url: &str,
    request: &serde_json::Value,
    token: Option<String>,
) -> Result<JsonRpcResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| format!("failed to build http client: {err}"))?;
    let mut builder = client.post(base_url).json(request);
    if let Some(token) = token {
        builder = builder.bearer_auth(token);
    }
    let response = builder.send().await.map_err(|err| format!("http request failed: {err}"))?;
    let body =
        response.text().await.map_err(|err| format!("failed to read sse response: {err}"))?;
    let data_line = body
        .lines()
        .find(|line| line.starts_with("data: "))
        .ok_or_else(|| "missing sse data line".to_string())?;
    let json = data_line.trim_start_matches("data: ").trim();
    let payload: JsonRpcResponse =
        serde_json::from_str(json).map_err(|err| format!("invalid json-rpc: {err}"))?;
    Ok(payload)
}
