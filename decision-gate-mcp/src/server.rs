// decision-gate-mcp/src/server.rs
// ============================================================================
// Module: MCP Server
// Description: MCP server implementations for stdio, HTTP, and SSE transports.
// Purpose: Expose Decision Gate tools via JSON-RPC 2.0.
// Dependencies: decision-gate-core, axum, tokio
// ============================================================================

//! ## Overview
//! The MCP server exposes Decision Gate tools using JSON-RPC 2.0. It supports
//! stdio, HTTP, and SSE transports and always routes calls through
//! [`crate::tools::ToolRouter`]. Security posture: inputs are untrusted and must
//! be validated; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::convert::Infallible;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::response::IntoResponse;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::post;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::SharedRunStateStore;
use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth::DefaultToolAuthz;
use crate::auth::RequestContext;
use crate::auth::StderrAuditSink;
use crate::capabilities::CapabilityRegistry;
use crate::config::DecisionGateConfig;
use crate::config::RunStateStoreType;
use crate::config::ServerAuthMode;
use crate::config::ServerTransport;
use crate::evidence::FederatedEvidenceProvider;
use crate::tools::ToolDefinition;
use crate::tools::ToolError;
use crate::tools::ToolRouter;

// ============================================================================
// SECTION: MCP Server
// ============================================================================

/// MCP server instance.
pub struct McpServer {
    /// Server configuration.
    config: DecisionGateConfig,
    /// Tool router for request dispatch.
    router: ToolRouter,
}

impl McpServer {
    /// Builds a new MCP server from configuration.
    ///
    /// # Errors
    ///
    /// Returns [`McpServerError`] when initialization fails.
    pub fn from_config(mut config: DecisionGateConfig) -> Result<Self, McpServerError> {
        config.validate().map_err(|err| McpServerError::Config(err.to_string()))?;
        let evidence = FederatedEvidenceProvider::from_config(&config)
            .map_err(|err| McpServerError::Init(err.to_string()))?;
        let capabilities = CapabilityRegistry::from_config(&config)
            .map_err(|err| McpServerError::Init(err.to_string()))?;
        let store = build_run_state_store(&config)?;
        let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
        let audit = Arc::new(StderrAuditSink);
        let router = ToolRouter::new(
            evidence,
            config.evidence.clone(),
            store,
            Arc::new(capabilities),
            authz,
            audit,
        );
        emit_local_only_warning(&config.server);
        Ok(Self {
            config,
            router,
        })
    }

    /// Serves requests using the configured transport.
    ///
    /// # Errors
    ///
    /// Returns [`McpServerError`] when the server fails.
    pub async fn serve(self) -> Result<(), McpServerError> {
        let transport = self.config.server.transport;
        let max_body_bytes = self.config.server.max_body_bytes;
        match transport {
            ServerTransport::Stdio => serve_stdio(&self.router, max_body_bytes),
            ServerTransport::Http => serve_http(self.config, self.router).await,
            ServerTransport::Sse => serve_sse(self.config, self.router).await,
        }
    }
}

/// Builds the run state store from MCP configuration.
fn build_run_state_store(
    config: &DecisionGateConfig,
) -> Result<SharedRunStateStore, McpServerError> {
    let store = match config.run_state_store.store_type {
        RunStateStoreType::Memory => SharedRunStateStore::from_store(InMemoryRunStateStore::new()),
        RunStateStoreType::Sqlite => {
            let path = config.run_state_store.path.clone().ok_or_else(|| {
                McpServerError::Config("sqlite run_state_store requires path".to_string())
            })?;
            let sqlite_config = SqliteStoreConfig {
                path,
                busy_timeout_ms: config.run_state_store.busy_timeout_ms,
                journal_mode: config.run_state_store.journal_mode,
                sync_mode: config.run_state_store.sync_mode,
                max_versions: config.run_state_store.max_versions,
            };
            let store = SqliteRunStateStore::new(sqlite_config)
                .map_err(|err| McpServerError::Init(err.to_string()))?;
            SharedRunStateStore::from_store(store)
        }
    };
    Ok(store)
}

// ============================================================================
// SECTION: Stdio Transport
// ============================================================================

/// Serves JSON-RPC requests over stdin/stdout.
fn serve_stdio(router: &ToolRouter, max_body_bytes: usize) -> Result<(), McpServerError> {
    let mut reader = BufReader::new(std::io::stdin());
    let mut writer = std::io::stdout();
    loop {
        let bytes = read_framed(&mut reader, max_body_bytes)?;
        let request: JsonRpcRequest = serde_json::from_slice(&bytes)
            .map_err(|_| McpServerError::Transport("invalid json-rpc request".to_string()))?;
        let context = RequestContext::stdio();
        let response = handle_request(router, &context, request);
        let payload = serde_json::to_vec(&response.1)
            .map_err(|_| McpServerError::Transport("json-rpc serialization failed".to_string()))?;
        write_framed(&mut writer, &payload)?;
    }
}

// ============================================================================
// SECTION: HTTP Transport
// ============================================================================

/// Serves JSON-RPC requests over HTTP.
async fn serve_http(config: DecisionGateConfig, router: ToolRouter) -> Result<(), McpServerError> {
    let bind = config
        .server
        .bind
        .as_ref()
        .ok_or_else(|| McpServerError::Config("bind address required".to_string()))?;
    let addr: SocketAddr =
        bind.parse().map_err(|_| McpServerError::Config("invalid bind address".to_string()))?;
    let state = Arc::new(ServerState {
        router,
        max_body_bytes: config.server.max_body_bytes,
    });
    let app = Router::new().route("/rpc", post(handle_http)).with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| McpServerError::Transport("http bind failed".to_string()))?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .map_err(|_| McpServerError::Transport("http server failed".to_string()))
}

/// Serves JSON-RPC requests over SSE.
async fn serve_sse(config: DecisionGateConfig, router: ToolRouter) -> Result<(), McpServerError> {
    let bind = config
        .server
        .bind
        .as_ref()
        .ok_or_else(|| McpServerError::Config("bind address required".to_string()))?;
    let addr: SocketAddr =
        bind.parse().map_err(|_| McpServerError::Config("invalid bind address".to_string()))?;
    let state = Arc::new(ServerState {
        router,
        max_body_bytes: config.server.max_body_bytes,
    });
    let app = Router::new().route("/rpc", post(handle_sse)).with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| McpServerError::Transport("sse bind failed".to_string()))?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .map_err(|_| McpServerError::Transport("sse server failed".to_string()))
}

/// Shared server state for HTTP/SSE handlers.
#[derive(Clone)]
struct ServerState {
    /// Tool router for request dispatch.
    router: ToolRouter,
    /// Maximum allowed request body size.
    max_body_bytes: usize,
}

/// Handles HTTP JSON-RPC requests.
async fn handle_http(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let context = http_request_context(ServerTransport::Http, peer, &headers);
    let response = parse_request(&state, &context, &bytes);
    (response.0, axum::Json(response.1))
}

/// Handles SSE JSON-RPC requests.
async fn handle_sse(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let context = http_request_context(ServerTransport::Sse, peer, &headers);
    let response = parse_request(&state, &context, &bytes);
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(1);
    let payload = serde_json::to_string(&response.1).unwrap_or_else(|_| {
        "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32060,\"message\":\"serialization \
         failed\"}}"
            .to_string()
    });
    let _ = tx.send(Ok(Event::default().data(payload))).await;
    Sse::new(ReceiverStream::new(rx))
}

// ============================================================================
// SECTION: JSON-RPC Handling
// ============================================================================

/// Incoming JSON-RPC request payload.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    /// JSON-RPC protocol version.
    jsonrpc: String,
    /// Request identifier.
    id: Value,
    /// Method name.
    method: String,
    /// Optional parameters payload.
    params: Option<Value>,
}

/// JSON-RPC response envelope.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    /// JSON-RPC protocol version.
    jsonrpc: &'static str,
    /// Request identifier.
    id: Value,
    /// Successful result payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    /// Error payload when the request fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error payload.
#[derive(Debug, Serialize)]
struct JsonRpcError {
    /// Error code.
    code: i64,
    /// Human-readable error message.
    message: String,
}

/// Tool call parameters for JSON-RPC requests.
#[derive(Debug, Deserialize)]
struct ToolCallParams {
    /// Tool name.
    name: String,
    /// Raw JSON arguments.
    arguments: Value,
}

/// Tool list response payload.
#[derive(Debug, Serialize)]
struct ToolListResult {
    /// Registered tool definitions.
    tools: Vec<ToolDefinition>,
}

/// Tool call response payload.
#[derive(Debug, Serialize)]
struct ToolCallResult {
    /// Tool output content.
    content: Vec<ToolContent>,
}

/// Tool output payloads for JSON-RPC responses.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    /// JSON tool output.
    Json {
        /// JSON payload.
        json: Value,
    },
}

/// Dispatches a JSON-RPC request to the tool router.
fn handle_request(
    router: &ToolRouter,
    base_context: &RequestContext,
    request: JsonRpcRequest,
) -> (StatusCode, JsonRpcResponse) {
    let context = base_context.clone().with_request_id(request.id.to_string());
    if request.jsonrpc != "2.0" {
        return (
            StatusCode::BAD_REQUEST,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32600,
                    message: "invalid json-rpc version".to_string(),
                }),
            },
        );
    }
    match request.method.as_str() {
        "tools/list" => match router.list_tools(&context) {
            Ok(tools) => match serde_json::to_value(ToolListResult {
                tools,
            }) {
                Ok(value) => (
                    StatusCode::OK,
                    JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: request.id,
                        result: Some(value),
                        error: None,
                    },
                ),
                Err(_) => jsonrpc_error(request.id, ToolError::Serialization),
            },
            Err(err) => jsonrpc_error(request.id, err),
        },
        "tools/call" => {
            let id = request.id;
            let params = request.params.unwrap_or(Value::Null);
            let call = serde_json::from_value::<ToolCallParams>(params);
            match call {
                Ok(call) => {
                    match call_tool_with_blocking(router, context, &call.name, call.arguments) {
                        Ok(result) => match serde_json::to_value(ToolCallResult {
                            content: vec![ToolContent::Json {
                                json: result,
                            }],
                        }) {
                            Ok(value) => (
                                StatusCode::OK,
                                JsonRpcResponse {
                                    jsonrpc: "2.0",
                                    id,
                                    result: Some(value),
                                    error: None,
                                },
                            ),
                            Err(_) => jsonrpc_error(id, ToolError::Serialization),
                        },
                        Err(err) => jsonrpc_error(id, err),
                    }
                }
                Err(_) => (
                    StatusCode::BAD_REQUEST,
                    JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "invalid tool params".to_string(),
                        }),
                    },
                ),
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "method not found".to_string(),
                }),
            },
        ),
    }
}

/// Executes a tool call, shifting to a blocking context when available.
fn call_tool_with_blocking(
    router: &ToolRouter,
    context: RequestContext,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| router.handle_tool_call(&context, name, arguments))
        }
        _ => router.handle_tool_call(&context, name, arguments),
    }
}

/// Parses and validates a JSON-RPC request payload.
fn parse_request(
    state: &ServerState,
    context: &RequestContext,
    bytes: &Bytes,
) -> (StatusCode, JsonRpcResponse) {
    if bytes.len() > state.max_body_bytes {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: -32070,
                    message: "request body too large".to_string(),
                }),
            },
        );
    }
    let request: Result<JsonRpcRequest, _> = serde_json::from_slice(bytes.as_ref());
    request.map_or_else(
        |_| {
            (
                StatusCode::BAD_REQUEST,
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32600,
                        message: "invalid json-rpc request".to_string(),
                    }),
                },
            )
        },
        |request| handle_request(&state.router, context, request),
    )
}

fn http_request_context(
    transport: ServerTransport,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> RequestContext {
    let auth_header =
        headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()).map(str::to_string);
    let client_subject = headers
        .get("x-decision-gate-client-subject")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    RequestContext::http(transport, Some(peer.ip()), auth_header, client_subject)
}

fn emit_local_only_warning(server: &crate::config::ServerConfig) {
    let auth_mode = server.auth.as_ref().map(|auth| auth.mode).unwrap_or(ServerAuthMode::LocalOnly);
    if auth_mode == ServerAuthMode::LocalOnly {
        eprintln!(
            "decision-gate-mcp: WARNING: server running in local-only mode without explicit auth; \
             configure server.auth to enable bearer_token or mtls"
        );
    }
}

/// Builds a JSON-RPC error response for a tool failure.
fn jsonrpc_error(id: Value, error: ToolError) -> (StatusCode, JsonRpcResponse) {
    let (status, code, message) = match error {
        ToolError::UnknownTool => (StatusCode::BAD_REQUEST, -32601, "unknown tool".to_string()),
        ToolError::Unauthenticated(_) => {
            (StatusCode::UNAUTHORIZED, -32001, "unauthenticated".to_string())
        }
        ToolError::Unauthorized(_) => (StatusCode::FORBIDDEN, -32003, "unauthorized".to_string()),
        ToolError::InvalidParams(message) => (StatusCode::BAD_REQUEST, -32602, message),
        ToolError::CapabilityViolation {
            code,
            message,
        } => (StatusCode::BAD_REQUEST, -32602, format!("{code}: {message}")),
        ToolError::NotFound(message) => (StatusCode::OK, -32004, message),
        ToolError::Conflict(message) => (StatusCode::OK, -32009, message),
        ToolError::Evidence(message) => (StatusCode::OK, -32020, message),
        ToolError::ControlPlane(err) => (StatusCode::OK, -32030, err.to_string()),
        ToolError::Runpack(message) => (StatusCode::OK, -32040, message),
        ToolError::Internal(message) => (StatusCode::OK, -32050, message),
        ToolError::Serialization => (StatusCode::OK, -32060, "serialization failed".to_string()),
    };
    (
        status,
        JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
            }),
        },
    )
}

// ============================================================================
// SECTION: Framing Helpers
// ============================================================================

/// Reads a framed stdio payload using MCP Content-Length headers.
fn read_framed(
    reader: &mut BufReader<impl Read>,
    max_body_bytes: usize,
) -> Result<Vec<u8>, McpServerError> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|_| McpServerError::Transport("stdio read failed".to_string()))?;
        if bytes == 0 {
            return Err(McpServerError::Transport("stdio closed".to_string()));
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .map_err(|_| McpServerError::Transport("invalid content length".to_string()))?;
            content_length = Some(parsed);
        }
    }
    let len = content_length
        .ok_or_else(|| McpServerError::Transport("missing content length".to_string()))?;
    if len > max_body_bytes {
        return Err(McpServerError::Transport("payload too large".to_string()));
    }
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|_| McpServerError::Transport("stdio read failed".to_string()))?;
    Ok(buf)
}

/// Writes a framed stdio payload using MCP Content-Length headers.
fn write_framed(writer: &mut impl Write, payload: &[u8]) -> Result<(), McpServerError> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer
        .write_all(header.as_bytes())
        .map_err(|_| McpServerError::Transport("stdio write failed".to_string()))?;
    writer
        .write_all(payload)
        .map_err(|_| McpServerError::Transport("stdio write failed".to_string()))?;
    writer.flush().map_err(|_| McpServerError::Transport("stdio write failed".to_string()))
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// MCP server errors.
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    /// Configuration errors.
    #[error("config error: {0}")]
    Config(String),
    /// Initialization errors.
    #[error("init error: {0}")]
    Init(String),
    /// Transport errors.
    #[error("transport error: {0}")]
    Transport(String),
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::print_stdout,
        clippy::print_stderr,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::panic_in_result_fn,
        clippy::unwrap_in_result,
        reason = "Test-only framing assertions."
    )]

    use std::io::BufReader;
    use std::io::Cursor;

    use super::read_framed;

    #[test]
    fn read_framed_rejects_payload_over_limit() {
        let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let framed = format!(
            "Content-Length: {}\r\n\r\n{}",
            payload.len(),
            String::from_utf8_lossy(payload)
        );
        let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
        let result = read_framed(&mut reader, payload.len() - 1);
        assert!(result.is_err());
    }

    #[test]
    fn read_framed_accepts_payload_at_limit() {
        let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let framed = format!(
            "Content-Length: {}\r\n\r\n{}",
            payload.len(),
            String::from_utf8_lossy(payload)
        );
        let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
        let result = read_framed(&mut reader, payload.len());
        assert!(result.is_ok());
        let bytes = result.expect("payload read");
        assert_eq!(bytes, payload);
    }
}
