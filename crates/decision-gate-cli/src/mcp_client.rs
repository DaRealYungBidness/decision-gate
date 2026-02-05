// crates/decision-gate-cli/src/mcp_client.rs
// ============================================================================
// Module: MCP Client
// Description: Multi-transport JSON-RPC client for Decision Gate MCP tools.
// Purpose: Provide CLI access to MCP tools over HTTP, SSE, or stdio transports.
// Dependencies: reqwest, serde, decision-gate-contract
// ============================================================================

//! ## Overview
//! Provides a minimal MCP client for the CLI to call `tools/list`, `tools/call`,
//! `resources/list`, and `resources/read` across HTTP, SSE, or stdio transports.
//!
//! Security posture: inputs and server responses are untrusted; apply size
//! limits, fail closed on parsing errors, and never log secrets.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use decision_gate_contract::tooling::ToolDefinition;
use decision_gate_core::ToolName;
use reqwest::Client;
use reqwest::header::ACCEPT;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum MCP response body size accepted by the CLI.
pub const MAX_MCP_RESPONSE_BYTES: usize = decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;

// ============================================================================
// SECTION: Types
// ============================================================================

/// Supported MCP transports for the CLI client.
///
/// # Invariants
/// - Variants are stable for CLI parsing and transport selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTransport {
    /// HTTP JSON-RPC transport.
    Http,
    /// SSE JSON-RPC transport.
    Sse,
    /// Stdio JSON-RPC transport.
    Stdio,
}

/// CLI MCP client configuration.
///
/// # Invariants
/// - For [`McpTransport::Http`] and [`McpTransport::Sse`], `endpoint` must be `Some`.
/// - For [`McpTransport::Stdio`], `stdio_command` must be `Some`.
/// - `stdio_env` entries are treated as raw key/value pairs and must be valid environment variables
///   for the target platform.
#[derive(Clone)]
pub struct McpClientConfig {
    /// Selected transport.
    pub transport: McpTransport,
    /// Endpoint URL for HTTP/SSE transports.
    pub endpoint: Option<String>,
    /// Stdio command to spawn.
    pub stdio_command: Option<String>,
    /// Stdio command arguments.
    pub stdio_args: Vec<String>,
    /// Stdio environment variables.
    pub stdio_env: Vec<(String, String)>,
    /// Request timeout.
    pub timeout: Duration,
    /// Optional bearer token.
    pub bearer_token: Option<String>,
    /// Optional client subject header.
    pub client_subject: Option<String>,
}

impl std::fmt::Debug for McpClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClientConfig")
            .field("transport", &self.transport)
            .field("endpoint", &self.endpoint)
            .field("stdio_command", &self.stdio_command)
            .field("stdio_args", &self.stdio_args)
            .field("stdio_env", &self.stdio_env)
            .field("timeout", &self.timeout)
            .field("bearer_token", &self.bearer_token.as_ref().map(|_| "<redacted>"))
            .field("client_subject", &self.client_subject)
            .finish()
    }
}

/// MCP client errors.
///
/// # Invariants
/// - Variants are stable for CLI error mapping and tests.
/// - String payloads are user-facing and may include untrusted server text.
#[derive(Debug, Error)]
pub enum McpClientError {
    /// Configuration error.
    #[error("mcp client config error: {0}")]
    Config(String),
    /// Transport error.
    #[error("mcp transport error: {0}")]
    Transport(String),
    /// JSON serialization error.
    #[error("mcp json error: {0}")]
    Json(String),
    /// Protocol parsing error.
    #[error("mcp protocol error: {0}")]
    Protocol(String),
    /// Response size exceeds limits.
    #[error("mcp response exceeds size limit ({actual} > {limit})")]
    ResponseTooLarge {
        /// Actual size in bytes.
        actual: usize,
        /// Maximum size in bytes.
        limit: usize,
    },
}

/// MCP resource metadata returned by `resources/list`.
///
/// # Invariants
/// - Values are untrusted and unvalidated; callers must treat them as hostile input.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceMetadata {
    /// Resource URI.
    pub uri: String,
    /// Resource display name.
    pub name: String,
    /// Resource description.
    pub description: String,
    /// Resource MIME type.
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// MCP resource content returned by `resources/read`.
///
/// # Invariants
/// - Values are untrusted and unvalidated; callers must treat them as hostile input.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceContent {
    /// Resource URI.
    pub uri: String,
    /// Resource MIME type.
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    /// Resource text content.
    pub text: String,
}

/// MCP client implementation.
///
/// # Invariants
/// - `next_id` is strictly increasing for each request sent by this client.
/// - `transport` is fully initialized and ready to send requests.
pub struct McpClient {
    /// Selected transport client.
    transport: McpTransportClient,
    /// Next JSON-RPC request identifier.
    next_id: u64,
}

// ============================================================================
// SECTION: JSON-RPC Structures
// ============================================================================

/// JSON-RPC request envelope.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    /// JSON-RPC version tag.
    jsonrpc: &'static str,
    /// Request identifier.
    id: u64,
    /// Method name to invoke.
    method: &'a str,
    /// Optional parameters payload.
    params: Option<Value>,
}

/// JSON-RPC response envelope.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    /// Optional result payload.
    result: Option<Value>,
    /// Optional error payload.
    error: Option<JsonRpcError>,
}

/// JSON-RPC error payload.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    /// Error message provided by the server.
    message: String,
}

/// `tools/list` result payload.
#[derive(Debug, Deserialize)]
struct ToolListResult {
    /// Tool definitions returned by the server.
    tools: Vec<ToolDefinition>,
}

/// `tools/call` result payload.
#[derive(Debug, Deserialize)]
struct ToolCallResult {
    /// Tool response content entries.
    content: Vec<ToolContent>,
}

/// Tool response content variants.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    /// JSON payload.
    Json {
        /// JSON response body.
        json: Value,
    },
}

/// `resources/list` result payload.
#[derive(Debug, Deserialize)]
struct ResourceListResult {
    /// Resource metadata entries.
    resources: Vec<ResourceMetadata>,
}

/// `resources/read` result payload.
#[derive(Debug, Deserialize)]
struct ResourceReadResult {
    /// Resource content entries.
    contents: Vec<ResourceContent>,
}

// ============================================================================
// SECTION: Client Implementations
// ============================================================================

/// Transport-specific client implementations.
enum McpTransportClient {
    /// HTTP JSON-RPC transport.
    Http(HttpMcpClient),
    /// SSE JSON-RPC transport.
    Sse(HttpMcpClient),
    /// Stdio JSON-RPC transport.
    Stdio(StdioMcpClient),
}

impl McpClient {
    /// Creates a new MCP client for the requested transport.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when configuration is invalid or transport setup fails.
    pub fn new(config: McpClientConfig) -> Result<Self, McpClientError> {
        let transport = match config.transport {
            McpTransport::Http => McpTransportClient::Http(HttpMcpClient::new(config)?),
            McpTransport::Sse => McpTransportClient::Sse(HttpMcpClient::new(config)?),
            McpTransport::Stdio => McpTransportClient::Stdio(StdioMcpClient::spawn(config)?),
        };
        Ok(Self {
            transport,
            next_id: 1,
        })
    }

    #[cfg(test)]
    #[allow(dead_code, reason = "Test-only helper for request id overflow coverage.")]
    pub(crate) const fn set_next_id_for_test(&mut self, next_id: u64) {
        self.next_id = next_id;
    }

    /// Calls `tools/list` and returns the tool definitions.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the transport or parsing fails.
    pub async fn list_tools(&mut self) -> Result<Vec<ToolDefinition>, McpClientError> {
        let response = self.send_request("tools/list", None).await?;
        let result = response.result.ok_or_else(|| {
            McpClientError::Protocol("missing result in tools/list response".into())
        })?;
        let parsed: ToolListResult = serde_json::from_value(result)
            .map_err(|err| McpClientError::Json(format!("invalid tools/list payload: {err}")))?;
        Ok(parsed.tools)
    }

    /// Calls `tools/call` and returns the JSON content payload.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the transport or parsing fails.
    pub async fn call_tool(
        &mut self,
        tool_name: ToolName,
        arguments: Value,
    ) -> Result<Value, McpClientError> {
        self.call_tool_raw(tool_name.as_str(), arguments).await
    }

    /// Calls `tools/call` with a raw tool name string.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the transport or parsing fails.
    pub async fn call_tool_raw(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, McpClientError> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });
        let response = self.send_request("tools/call", Some(params)).await?;
        let result = response.result.ok_or_else(|| {
            McpClientError::Protocol(format!("missing result for tool {tool_name}"))
        })?;
        let parsed: ToolCallResult = serde_json::from_value(result).map_err(|err| {
            McpClientError::Json(format!("invalid tools/call payload for {tool_name}: {err}"))
        })?;
        let json = parsed
            .content
            .into_iter()
            .map(|item| match item {
                ToolContent::Json {
                    json,
                } => json,
            })
            .next()
            .ok_or_else(|| {
                McpClientError::Protocol(format!("tool {tool_name} returned no json content"))
            })?;
        Ok(json)
    }

    /// Calls `resources/list` and returns resource metadata.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the transport or parsing fails.
    pub async fn list_resources(&mut self) -> Result<Vec<ResourceMetadata>, McpClientError> {
        let response = self.send_request("resources/list", None).await?;
        let result = response.result.ok_or_else(|| {
            McpClientError::Protocol("missing result in resources/list response".into())
        })?;
        let parsed: ResourceListResult = serde_json::from_value(result).map_err(|err| {
            McpClientError::Json(format!("invalid resources/list payload: {err}"))
        })?;
        Ok(parsed.resources)
    }

    /// Calls `resources/read` and returns resource contents.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the transport or parsing fails.
    pub async fn read_resource(
        &mut self,
        uri: &str,
    ) -> Result<Vec<ResourceContent>, McpClientError> {
        let params = serde_json::json!({ "uri": uri });
        let response = self.send_request("resources/read", Some(params)).await?;
        let result = response.result.ok_or_else(|| {
            McpClientError::Protocol("missing result in resources/read response".into())
        })?;
        let parsed: ResourceReadResult = serde_json::from_value(result).map_err(|err| {
            McpClientError::Json(format!("invalid resources/read payload: {err}"))
        })?;
        Ok(parsed.contents)
    }

    /// Sends a JSON-RPC request for the given method and parameters.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when transport or parsing fails.
    async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse, McpClientError> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| McpClientError::Protocol("json-rpc request id overflow".to_string()))?;
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };
        match &mut self.transport {
            McpTransportClient::Http(client) => client.send_request(&request, false).await,
            McpTransportClient::Sse(client) => client.send_request(&request, true).await,
            McpTransportClient::Stdio(client) => client.send_request(&request).await,
        }
    }
}

// ============================================================================
// SECTION: HTTP Transport
// ============================================================================

/// HTTP/SSE JSON-RPC transport client.
struct HttpMcpClient {
    /// Reqwest client instance.
    client: Client,
    /// Base endpoint URL.
    endpoint: String,
    /// Optional bearer token.
    bearer_token: Option<String>,
    /// Optional client subject header.
    client_subject: Option<String>,
}

impl HttpMcpClient {
    /// Builds a new HTTP/SSE transport client.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the configuration is invalid or the HTTP
    /// client cannot be constructed.
    fn new(config: McpClientConfig) -> Result<Self, McpClientError> {
        let endpoint = config.endpoint.ok_or_else(|| {
            McpClientError::Config("endpoint is required for HTTP/SSE transport".to_string())
        })?;
        let client = Client::builder()
            .timeout(config.timeout)
            .redirect(Policy::none())
            .build()
            .map_err(|err| McpClientError::Transport(err.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            bearer_token: config.bearer_token,
            client_subject: config.client_subject,
        })
    }

    /// Sends a JSON-RPC request over HTTP or SSE.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the request fails or the response is invalid.
    async fn send_request(
        &self,
        request: &JsonRpcRequest<'_>,
        sse: bool,
    ) -> Result<JsonRpcResponse, McpClientError> {
        let payload = serde_json::to_vec(request)
            .map_err(|err| McpClientError::Json(format!("jsonrpc serialization failed: {err}")))?;
        let headers = self.headers(sse)?;
        let response = self
            .client
            .post(&self.endpoint)
            .headers(headers)
            .body(payload)
            .send()
            .await
            .map_err(|err| McpClientError::Transport(err.to_string()))?;
        let status = response.status();
        let body = read_response_body_with_limit(response, MAX_MCP_RESPONSE_BYTES).await?;
        if !status.is_success() {
            let preview = String::from_utf8_lossy(&body);
            return Err(McpClientError::Transport(format!(
                "http status {}: {}",
                status.as_u16(),
                preview.trim()
            )));
        }
        let json_bytes = if sse { parse_sse_body(&body)? } else { body };
        let response: JsonRpcResponse = serde_json::from_slice(&json_bytes)
            .map_err(|err| McpClientError::Protocol(format!("invalid json-rpc response: {err}")))?;
        if let Some(error) = response.error.as_ref() {
            return Err(McpClientError::Protocol(error.message.clone()));
        }
        Ok(response)
    }

    /// Builds request headers for the MCP transport.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when header values are invalid.
    fn headers(&self, sse: bool) -> Result<HeaderMap, McpClientError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if sse {
            headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        }
        if let Some(token) = &self.bearer_token {
            let value = format!("Bearer {token}");
            let header = HeaderValue::from_str(&value)
                .map_err(|_| McpClientError::Config("invalid bearer token header".to_string()))?;
            headers.insert(AUTHORIZATION, header);
        }
        if let Some(subject) = &self.client_subject {
            let header = HeaderValue::from_str(subject)
                .map_err(|_| McpClientError::Config("invalid client subject header".to_string()))?;
            headers.insert("x-decision-gate-client-subject", header);
        }
        Ok(headers)
    }
}

// ============================================================================
// SECTION: HTTP/SSE Helpers
// ============================================================================

/// Reads an HTTP/SSE response body while enforcing a hard byte limit.
async fn read_response_body_with_limit(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, McpClientError> {
    let mut body = Vec::new();
    let mut total: usize = 0;
    while let Some(chunk) =
        response.chunk().await.map_err(|err| McpClientError::Transport(err.to_string()))?
    {
        let next_total =
            total.checked_add(chunk.len()).ok_or(McpClientError::ResponseTooLarge {
                actual: usize::MAX,
                limit,
            })?;
        if next_total > limit {
            return Err(McpClientError::ResponseTooLarge {
                actual: next_total,
                limit,
            });
        }
        body.extend_from_slice(&chunk);
        total = next_total;
    }
    Ok(body)
}

/// Parses an SSE response body and extracts the first `data:` payload.
///
/// # Errors
///
/// Returns [`McpClientError`] when the body is not UTF-8 or lacks `data:` lines.
pub fn parse_sse_body(body: &[u8]) -> Result<Vec<u8>, McpClientError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| McpClientError::Protocol("sse response was not valid utf-8".to_string()))?;
    let mut data_lines = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            if !data_lines.is_empty() {
                break;
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim_start());
        }
    }
    if data_lines.is_empty() {
        return Err(McpClientError::Protocol("sse response missing data".to_string()));
    }
    let joined = data_lines.join("\n");
    Ok(joined.into_bytes())
}

// ============================================================================
// SECTION: Stdio Transport
// ============================================================================

/// Stdio JSON-RPC transport client.
struct StdioMcpClient {
    /// Spawned child process handle.
    child: Child,
    /// Child stdin handle (shared across requests).
    stdin: Arc<Mutex<ChildStdin>>,
    /// Child stdout handle (shared across requests).
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
}

impl StdioMcpClient {
    /// Spawns the stdio transport process.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when the process cannot be spawned or streams
    /// are unavailable.
    fn spawn(config: McpClientConfig) -> Result<Self, McpClientError> {
        let command = config
            .stdio_command
            .ok_or_else(|| McpClientError::Config("stdio command is required".to_string()))?;
        let mut cmd = Command::new(&command);
        cmd.args(&config.stdio_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (key, value) in &config.stdio_env {
            cmd.env(key, value);
        }
        let mut child = cmd
            .spawn()
            .map_err(|err| McpClientError::Transport(format!("spawn stdio failed: {err}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpClientError::Transport("missing child stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpClientError::Transport("missing child stdout".to_string()))?;
        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
        })
    }

    /// Sends a JSON-RPC request to the stdio child process.
    ///
    /// # Errors
    ///
    /// Returns [`McpClientError`] when I/O or parsing fails.
    async fn send_request(
        &self,
        request: &JsonRpcRequest<'_>,
    ) -> Result<JsonRpcResponse, McpClientError> {
        let payload = serde_json::to_vec(request)
            .map_err(|err| McpClientError::Json(format!("jsonrpc serialization failed: {err}")))?;
        let stdin = Arc::clone(&self.stdin);
        let stdout = Arc::clone(&self.stdout);
        tokio::task::spawn_blocking(move || {
            {
                let mut input = stdin
                    .lock()
                    .map_err(|_| McpClientError::Transport("stdin lock poisoned".to_string()))?;
                write_framed(&mut *input, &payload)?;
            }
            let response_bytes = {
                let mut output = stdout
                    .lock()
                    .map_err(|_| McpClientError::Transport("stdout lock poisoned".to_string()))?;
                read_framed(&mut *output)?
            };
            let response: JsonRpcResponse =
                serde_json::from_slice(&response_bytes).map_err(|err| {
                    McpClientError::Protocol(format!("invalid json-rpc response: {err}"))
                })?;
            if let Some(error) = response.error.as_ref() {
                return Err(McpClientError::Protocol(error.message.clone()));
            }
            Ok(response)
        })
        .await
        .map_err(|err| McpClientError::Transport(format!("stdio request join failed: {err}")))?
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Reads a stdio-framed JSON-RPC message.
///
/// # Errors
///
/// Returns [`McpClientError`] when framing headers are invalid, the content
/// length exceeds limits, or I/O fails.
pub fn read_framed(reader: &mut BufReader<impl Read>) -> Result<Vec<u8>, McpClientError> {
    let mut content_length: Option<u64> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| McpClientError::Transport(format!("stdio read failed: {err}")))?;
        if bytes == 0 {
            return Err(McpClientError::Transport("stdio closed".to_string()));
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            let parsed = value
                .trim()
                .parse::<u64>()
                .map_err(|_| McpClientError::Protocol("invalid content length".to_string()))?;
            content_length = Some(parsed);
        }
    }
    let len = content_length.ok_or_else(|| {
        McpClientError::Protocol("missing content length in stdio response".to_string())
    })?;
    let limit = u64::try_from(MAX_MCP_RESPONSE_BYTES).unwrap_or(u64::MAX);
    if len > limit {
        let actual = usize::try_from(len).unwrap_or(usize::MAX);
        return Err(McpClientError::ResponseTooLarge {
            actual,
            limit: MAX_MCP_RESPONSE_BYTES,
        });
    }
    let len = usize::try_from(len).map_err(|_| {
        McpClientError::Protocol("content length exceeds addressable size".to_string())
    })?;
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|err| McpClientError::Transport(format!("stdio read failed: {err}")))?;
    Ok(buf)
}

/// Writes a stdio-framed JSON-RPC message.
///
/// # Errors
///
/// Returns [`McpClientError`] when writes fail.
pub fn write_framed(writer: &mut impl Write, payload: &[u8]) -> Result<(), McpClientError> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer
        .write_all(header.as_bytes())
        .map_err(|err| McpClientError::Transport(format!("stdio write failed: {err}")))?;
    writer
        .write_all(payload)
        .map_err(|err| McpClientError::Transport(format!("stdio write failed: {err}")))?;
    writer
        .flush()
        .map_err(|err| McpClientError::Transport(format!("stdio write failed: {err}")))?;
    Ok(())
}

// ============================================================================
// SECTION: Utilities
// ============================================================================

/// Builds a stdio config environment variable pair.
#[must_use]
pub fn stdio_config_env(config_path: &Path) -> (String, String) {
    ("DECISION_GATE_CONFIG".to_string(), config_path.display().to_string())
}
