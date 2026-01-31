// system-tests/tests/helpers/provider_stub.rs
// ============================================================================
// Module: Provider Stub
// Description: Minimal MCP provider stub for system-tests.
// Purpose: Exercise federated provider flows over HTTP.
// Dependencies: axum, decision-gate-core
// ============================================================================

use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::TrustLane;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use tokio::runtime::Builder;
use tokio::sync::oneshot;
use tokio::time::sleep;

#[derive(Clone)]
struct ProviderState {
    response: ProviderResponse,
    response_delay: Duration,
    requests: Arc<Mutex<Vec<ProviderRequest>>>,
}

#[derive(Clone)]
enum ProviderResponse {
    Fixed(Value),
    Fixtures(Vec<ProviderFixture>),
}

/// Fixture describing a check response for a specific parameter set.
#[derive(Clone, Debug)]
pub struct ProviderFixture {
    pub check_id: String,
    pub params: Value,
    pub result: Value,
    pub anchor: Option<EvidenceAnchor>,
}

/// Recorded request metadata for provider stub calls.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderRequest {
    pub request_id: Value,
    pub correlation_id: Option<String>,
}

/// Handle for the stub MCP provider server.
pub struct ProviderStubHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<thread::JoinHandle<()>>,
    requests: Arc<Mutex<Vec<ProviderRequest>>>,
}

impl ProviderStubHandle {
    /// Returns the provider URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns captured provider requests.
    pub fn requests(&self) -> Vec<ProviderRequest> {
        self.requests.lock().map_or_else(|_| Vec::new(), |entries| entries.clone())
    }
}

impl Drop for ProviderStubHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
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
    spawn_provider_stub_with_response(ProviderResponse::Fixed(response_value), response_delay).await
}

/// Spawn a stub MCP provider that returns responses based on fixtures.
pub async fn spawn_provider_fixture_stub(
    fixtures: Vec<ProviderFixture>,
) -> Result<ProviderStubHandle, String> {
    spawn_provider_stub_with_response(
        ProviderResponse::Fixtures(fixtures),
        Duration::from_millis(0),
    )
    .await
}

#[allow(clippy::unused_async, reason = "Async signature keeps helper API consistent in tests.")]
async fn spawn_provider_stub_with_response(
    response: ProviderResponse,
    response_delay: Duration,
) -> Result<ProviderStubHandle, String> {
    let listener = StdTcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("provider stub bind failed: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("provider stub listener nonblocking failed: {err}"))?;
    let addr =
        listener.local_addr().map_err(|err| format!("provider stub local addr failed: {err}"))?;
    let base_url = format!("http://{addr}/rpc");

    let requests = Arc::new(Mutex::new(Vec::new()));
    let state = ProviderState {
        response,
        response_delay,
        requests: Arc::clone(&requests),
    };
    let app = Router::new().route("/rpc", post(handle_rpc)).with_state(state);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join = thread::spawn(move || {
        let runtime = match Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = error;
                return;
            }
        };
        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = error;
                    return;
                }
            };
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });
    });
    Ok(ProviderStubHandle {
        base_url,
        shutdown: Some(shutdown_tx),
        join: Some(join),
        requests,
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

async fn handle_rpc(
    State(state): State<ProviderState>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let request: Result<JsonRpcRequest, _> = serde_json::from_slice(bytes.as_ref());
    let request_id = request.as_ref().map(|req| req.id.clone()).unwrap_or(Value::Null);
    let correlation_id = headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    record_request(&state, request_id, correlation_id);
    if state.response_delay > Duration::from_millis(0) {
        sleep(state.response_delay).await;
    }
    let response = request.map_or_else(
        |_| JsonRpcResponse {
            jsonrpc: "2.0",
            id: Value::Null,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "invalid request".to_string(),
            }),
        },
        |request| handle_request(&state, request),
    );
    axum::Json(response)
}

#[allow(clippy::too_many_lines, reason = "Single request handler keeps stub logic easy to audit.")]
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
                    let parsed: EvidenceQueryRequest = match serde_json::from_value(call.arguments)
                    {
                        Ok(parsed) => parsed,
                        Err(_) => {
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
                    };
                    let (response_value, anchor) = match &state.response {
                        ProviderResponse::Fixed(value) => (value.clone(), None),
                        ProviderResponse::Fixtures(fixtures) => {
                            let check_id = &parsed.query.check_id;
                            let params = normalize_params(parsed.query.params.clone());
                            let fixture = fixtures.iter().find(|fixture| {
                                fixture.check_id == *check_id && fixture.params == params
                            });
                            match fixture {
                                Some(fixture) => (fixture.result.clone(), fixture.anchor.clone()),
                                None => {
                                    return JsonRpcResponse {
                                        jsonrpc: "2.0",
                                        id: request.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32602,
                                            message: "no matching fixture".to_string(),
                                        }),
                                    };
                                }
                            }
                        }
                    };
                    let result = EvidenceResult {
                        value: Some(EvidenceValue::Json(response_value)),
                        lane: TrustLane::Verified,
                        error: None,
                        evidence_hash: None,
                        evidence_ref: None,
                        evidence_anchor: anchor,
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

fn normalize_params(params: Option<Value>) -> Value {
    params.unwrap_or_else(|| Value::Object(Map::new()))
}

fn record_request(state: &ProviderState, request_id: Value, correlation_id: Option<String>) {
    let Ok(mut guard) = state.requests.lock() else {
        return;
    };
    guard.push(ProviderRequest {
        request_id,
        correlation_id,
    });
}
