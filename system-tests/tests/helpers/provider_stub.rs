// system-tests/tests/helpers/provider_stub.rs
// ============================================================================
// Module: Provider Stub
// Description: Minimal MCP provider stub for system-tests.
// Purpose: Exercise federated provider flows over HTTP.
// Dependencies: axum, decision-gate-core
// ============================================================================

use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use super::harness::allocate_bind_addr;

#[derive(Clone)]
struct ProviderState {
    response_value: Value,
    response_delay: Duration,
}

/// Handle for the stub MCP provider server.
pub struct ProviderStubHandle {
    base_url: String,
    join: JoinHandle<()>,
}

impl ProviderStubHandle {
    /// Returns the provider URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for ProviderStubHandle {
    fn drop(&mut self) {
        self.join.abort();
    }
}

/// Spawn a stub MCP provider that returns a fixed JSON value.
pub async fn spawn_provider_stub(response_value: Value) -> Result<ProviderStubHandle, String> {
    spawn_provider_stub_with_delay(response_value, Duration::from_millis(0)).await
}

/// Spawn a stub MCP provider with a response delay.
pub async fn spawn_provider_stub_with_delay(
    response_value: Value,
    response_delay: Duration,
) -> Result<ProviderStubHandle, String> {
    let addr = allocate_bind_addr()?;
    let state = ProviderState {
        response_value,
        response_delay,
    };
    let app = Router::new().route("/rpc", post(handle_rpc)).with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| format!("provider stub bind failed: {err}"))?;
    let base_url = format!("http://{}/rpc", listener.local_addr().map_err(|err| err.to_string())?);
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(ProviderStubHandle {
        base_url,
        join,
    })
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    arguments: Value,
}

#[derive(Debug, Serialize)]
struct ToolCallResult {
    content: Vec<ToolContent>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    Json { json: EvidenceResult },
}

async fn handle_rpc(State(state): State<ProviderState>, bytes: Bytes) -> impl IntoResponse {
    let request: Result<JsonRpcRequest, _> = serde_json::from_slice(bytes.as_ref());
    if state.response_delay > Duration::from_millis(0) {
        sleep(state.response_delay).await;
    }
    let response = match request {
        Ok(request) => handle_request(&state, request),
        Err(_) => JsonRpcResponse {
            jsonrpc: "2.0",
            id: Value::Null,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "invalid request".to_string(),
            }),
        },
    };
    axum::Json(response)
}

fn handle_request(state: &ProviderState, request: JsonRpcRequest) -> JsonRpcResponse {
    if request.jsonrpc != "2.0" {
        return JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "invalid json-rpc version".to_string(),
            }),
        };
    }

    match request.method.as_str() {
        "tools/call" => {
            let params = request.params.unwrap_or(Value::Null);
            let call: Result<ToolCallParams, _> = serde_json::from_value(params);
            match call {
                Ok(call) if call.name == "evidence_query" => {
                    let parsed: Result<EvidenceQueryRequest, _> =
                        serde_json::from_value(call.arguments);
                    if parsed.is_err() {
                        return JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32602,
                                message: "invalid evidence_query payload".to_string(),
                            }),
                        };
                    }
                    let result = EvidenceResult {
                        value: Some(EvidenceValue::Json(state.response_value.clone())),
                        evidence_hash: None,
                        evidence_ref: None,
                        evidence_anchor: None,
                        signature: None,
                        content_type: Some("application/json".to_string()),
                    };
                    JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: request.id,
                        result: Some(
                            serde_json::to_value(ToolCallResult {
                                content: vec![ToolContent::Json {
                                    json: result,
                                }],
                            })
                            .unwrap_or(Value::Null),
                        ),
                        error: None,
                    }
                }
                _ => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "invalid tool params".to_string(),
                    }),
                },
            }
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "method not found".to_string(),
            }),
        },
    }
}
