// system-tests/tests/suites/sse_transport.rs
// ============================================================================
// Module: SSE Transport Tests
// Description: End-to-end SSE transport validation for MCP tools.
// Purpose: Ensure tools/list and tools/call succeed over SSE with correlation headers.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! ## Overview
//! End-to-end SSE transport validation for MCP tools.
//! Purpose: Ensure tools/list and tools/call succeed over SSE with correlation headers.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::time::Duration;

use decision_gate_contract::ToolName;
use decision_gate_contract::tooling::ToolDefinition;
use decision_gate_core::Timestamp;
use decision_gate_mcp::docs::RESOURCE_URI_PREFIX;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::docs::ResourceContent;
use helpers::docs::ResourceMetadata;
use helpers::docs::SearchResult;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_sse_config;
use helpers::harness::base_sse_config_with_bearer;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::timeouts;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::helpers;

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ToolListResult {
    tools: Vec<ToolDefinition>,
}

#[derive(Debug, Deserialize)]
struct ResourceListResult {
    resources: Vec<ResourceMetadata>,
}

#[derive(Debug, Deserialize)]
struct ResourceReadResult {
    contents: Vec<ResourceContent>,
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "End-to-end SSE flow is best reviewed in one block.")]
async fn sse_transport_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sse_transport_end_to_end")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_sse_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let base_url = server.base_url().to_string();

    let tools_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    wait_for_ready(
        || async {
            send_sse_request(&base_url, &tools_request, None, Some("corr-1".to_string()))
                .await
                .map(|_| ())
        },
        Duration::from_secs(5),
        "sse server",
    )
    .await?;

    let (tools_response, tools_headers) =
        send_sse_request(&base_url, &tools_request, None, Some("corr-1".to_string())).await?;
    let tools = tools_response
        .result
        .as_ref()
        .ok_or_else(|| "missing result for tools/list".to_string())?;
    if tools.get("tools").is_none() {
        return Err("tools/list missing tools list".into());
    }
    assert_correlation_headers(&tools_headers, Some("corr-1"))?;

    let mut fixture = ScenarioFixture::time_after("sse-interop", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_output = call_tool_over_sse(
        &base_url,
        "scenario_define",
        serde_json::to_value(&define_request)?,
        Some("corr-2".to_string()),
    )
    .await?;

    let start_request = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    call_tool_over_sse(
        &base_url,
        "scenario_start",
        serde_json::to_value(&start_request)?,
        Some("corr-3".to_string()),
    )
    .await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        trigger: fixture.trigger_event("trigger-1", Timestamp::Logical(2)),
    };
    call_tool_over_sse(
        &base_url,
        "scenario_trigger",
        serde_json::to_value(&trigger_request)?,
        Some("corr-4".to_string()),
    )
    .await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: decision_gate_core::runtime::StatusRequest {
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            run_id: fixture.run_id.clone(),
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_output = call_tool_over_sse(
        &base_url,
        "scenario_status",
        serde_json::to_value(&status_request)?,
        Some("corr-5".to_string()),
    )
    .await?;

    if define_output.get("scenario_id").is_none() {
        return Err("scenario_define response missing scenario_id".into());
    }
    if status_output.get("status").is_none() {
        return Err("scenario_status response missing status".into());
    }

    let transcript = vec![helpers::mcp_client::TranscriptEntry {
        sequence: 1,
        method: "tools/list".to_string(),
        request: tools_request.clone(),
        response: serde_json::to_value(&tools_response)?,
        error: tools_response.error.as_ref().map(|err| err.message.clone()),
    }];
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["sse transport executed tools/list and tools/call".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn docs_search_sse_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("docs_search_sse_end_to_end")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_sse_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let base_url = server.base_url().to_string();

    let tools_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    wait_for_ready(
        || async { send_sse_request(&base_url, &tools_request, None, None).await.map(|_| ()) },
        Duration::from_secs(5),
        "sse server",
    )
    .await?;

    let (tools_response, _headers) =
        send_sse_request(&base_url, &tools_request, None, None).await?;
    let tools_result =
        tools_response.result.clone().ok_or_else(|| "missing result for tools/list".to_string())?;
    let tools: ToolListResult =
        serde_json::from_value(tools_result).map_err(|err| format!("invalid tools/list: {err}"))?;
    if !tools.tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("tools/list missing decision_gate_docs_search".into());
    }

    let docs_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "decision_gate_docs_search",
            "arguments": {
                "query": "trust lanes",
                "max_sections": 3
            }
        }
    });
    let (docs_response, _headers) = send_sse_request(&base_url, &docs_request, None, None).await?;
    if let Some(error) = docs_response.error {
        return Err(format!("docs search error: {}", error.message).into());
    }
    let docs_result =
        docs_response.result.clone().ok_or_else(|| "missing result for docs search".to_string())?;
    let content = docs_result
        .get("content")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|value| value.get("json"))
        .cloned()
        .ok_or_else(|| "missing docs search content".to_string())?;
    let search: SearchResult =
        serde_json::from_value(content).map_err(|err| format!("invalid docs search: {err}"))?;
    if !search.docs_covered.iter().any(|doc| doc.doc_id == "evidence_flow_and_execution_model") {
        return Err("docs search missing evidence flow doc".into());
    }

    let transcript = vec![
        helpers::mcp_client::TranscriptEntry {
            sequence: 1,
            method: "tools/list".to_string(),
            request: tools_request,
            response: serde_json::to_value(&tools_response)?,
            error: tools_response.error.as_ref().map(|err| err.message.clone()),
        },
        helpers::mcp_client::TranscriptEntry {
            sequence: 2,
            method: "tools/call".to_string(),
            request: docs_request,
            response: serde_json::to_value(&docs_response)?,
            error: docs_response.error.as_ref().map(|err| err.message.clone()),
        },
    ];
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["sse docs search returned expected sections".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}

#[allow(clippy::too_many_lines, reason = "SSE resources flow is validated end-to-end in one test.")]
#[tokio::test(flavor = "multi_thread")]
async fn docs_resources_sse_list_read() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("docs_resources_sse_list_read")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_sse_config_with_bearer(&bind, "docs-token");
    let server = spawn_mcp_server(config).await?;
    let base_url = server.base_url().to_string();

    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list"
    });

    wait_for_ready(
        || async {
            send_sse_request(&base_url, &list_request, Some("docs-token".to_string()), None)
                .await
                .map(|_| ())
        },
        Duration::from_secs(5),
        "sse server",
    )
    .await?;

    let (unauthorized, _) = send_sse_request(&base_url, &list_request, None, None).await?;
    let error =
        unauthorized.error.as_ref().ok_or_else(|| "missing unauthenticated error".to_string())?;
    if !error.message.contains("unauthenticated") {
        return Err(format!("unexpected unauthenticated error: {}", error.message).into());
    }

    let (list_response, _headers) =
        send_sse_request(&base_url, &list_request, Some("docs-token".to_string()), None).await?;
    if let Some(error) = list_response.error {
        return Err(format!("resources/list error: {}", error.message).into());
    }
    let list_result = list_response
        .result
        .clone()
        .ok_or_else(|| "missing result for resources/list".to_string())?;
    let list: ResourceListResult = serde_json::from_value(list_result)
        .map_err(|err| format!("invalid resources/list: {err}"))?;
    if list.resources.is_empty() {
        return Err("resources/list returned empty list".into());
    }
    let evidence = list
        .resources
        .iter()
        .find(|entry| entry.uri == "decision-gate://docs/evidence-flow")
        .ok_or_else(|| "missing evidence-flow resource".to_string())?;
    if !evidence.uri.starts_with(RESOURCE_URI_PREFIX) {
        return Err("resource uri missing decision-gate://docs/ prefix".into());
    }

    let read_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/read",
        "params": {
            "uri": evidence.uri
        }
    });
    let (read_response, _headers) =
        send_sse_request(&base_url, &read_request, Some("docs-token".to_string()), None).await?;
    if let Some(error) = read_response.error {
        return Err(format!("resources/read error: {}", error.message).into());
    }
    let read_result = read_response
        .result
        .clone()
        .ok_or_else(|| "missing result for resources/read".to_string())?;
    let read: ResourceReadResult = serde_json::from_value(read_result)
        .map_err(|err| format!("invalid resources/read: {err}"))?;
    let content = read
        .contents
        .first()
        .ok_or_else(|| "resources/read returned empty contents".to_string())?;
    if !content.text.contains("# Evidence Flow + Execution Model") {
        return Err("resource body missing expected heading".into());
    }

    let transcript = vec![
        helpers::mcp_client::TranscriptEntry {
            sequence: 1,
            method: "resources/list".to_string(),
            request: list_request.clone(),
            response: serde_json::to_value(&unauthorized)?,
            error: unauthorized.error.as_ref().map(|err| err.message.clone()),
        },
        helpers::mcp_client::TranscriptEntry {
            sequence: 2,
            method: "resources/list".to_string(),
            request: list_request,
            response: serde_json::to_value(&list_response)?,
            error: list_response.error.as_ref().map(|err| err.message.clone()),
        },
        helpers::mcp_client::TranscriptEntry {
            sequence: 3,
            method: "resources/read".to_string(),
            request: read_request,
            response: serde_json::to_value(&read_response)?,
            error: read_response.error.as_ref().map(|err| err.message.clone()),
        },
    ];
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["sse resources list/read succeeded with auth".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn sse_transport_bearer_rejects_missing_token() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("sse_transport_bearer_rejects_missing_token")?;
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
        || async { send_sse_request(&base_url, &request, None, None).await.map(|_| ()) },
        Duration::from_secs(5),
        "sse server",
    )
    .await?;

    let (unauthorized, _) = send_sse_request(&base_url, &request, None, None).await?;
    let error = unauthorized
        .error
        .as_ref()
        .ok_or_else(|| "missing error for unauthenticated request".to_string())?;
    if !error.message.contains("unauthenticated") {
        return Err(format!("unexpected unauthenticated error: {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
    reporter.finish(
        "pass",
        vec!["sse bearer auth rejects missing token".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}

async fn call_tool_over_sse(
    base_url: &str,
    name: &str,
    arguments: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    });
    let (response, _headers) = send_sse_request(base_url, &request, None, correlation_id).await?;
    if let Some(error) = response.error {
        return Err(error.message);
    }
    response.result.ok_or_else(|| format!("missing result for tool {name}")).and_then(|result| {
        let content = result
            .get("content")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|value| value.get("json"))
            .cloned();
        content.ok_or_else(|| format!("missing json content for tool {name}"))
    })
}

async fn send_sse_request(
    base_url: &str,
    request: &Value,
    bearer: Option<String>,
    correlation_id: Option<String>,
) -> Result<(JsonRpcResponse, reqwest::header::HeaderMap), String> {
    let client = reqwest::Client::builder()
        .timeout(timeouts::resolve_timeout(Duration::from_secs(5)))
        .build()
        .map_err(|err| format!("failed to build http client: {err}"))?;
    let mut builder = client.post(base_url).json(request);
    if let Some(token) = bearer {
        builder = builder.bearer_auth(token);
    }
    if let Some(correlation_id) = correlation_id {
        builder = builder.header("x-correlation-id", correlation_id);
    }
    let response = builder.send().await.map_err(|err| format!("http request failed: {err}"))?;
    let headers = response.headers().clone();
    let body =
        response.text().await.map_err(|err| format!("failed to read sse response: {err}"))?;
    let data_line = body
        .lines()
        .find(|line| line.starts_with("data: "))
        .ok_or_else(|| "missing sse data line".to_string())?;
    let json = data_line.trim_start_matches("data: ").trim();
    let payload: JsonRpcResponse =
        serde_json::from_str(json).map_err(|err| format!("invalid json-rpc: {err}"))?;
    Ok((payload, headers))
}

fn assert_correlation_headers(
    headers: &reqwest::header::HeaderMap,
    expected_client: Option<&str>,
) -> Result<(), String> {
    let server = headers
        .get("x-server-correlation-id")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "missing x-server-correlation-id header".to_string())?;
    if server.is_empty() {
        return Err("empty x-server-correlation-id header".to_string());
    }
    if let Some(expected) = expected_client {
        let client = headers
            .get("x-correlation-id")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "missing x-correlation-id header".to_string())?;
        if client != expected {
            return Err(format!("unexpected x-correlation-id: {client}"));
        }
    }
    Ok(())
}
