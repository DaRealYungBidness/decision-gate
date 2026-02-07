// system-tests/tests/helpers/mcp_client.rs
// ============================================================================
// Module: MCP HTTP Client
// Description: JSON-RPC client for Decision Gate MCP server.
// Purpose: Issue tools/list and tools/call over HTTP with transcripts.
// Dependencies: reqwest, serde
// ============================================================================

//! ## Overview
//! JSON-RPC client for Decision Gate MCP server.
//! Purpose: Issue tools/list and tools/call over HTTP with transcripts.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use decision_gate_contract::tooling::ToolDefinition;
use reqwest::Certificate;
use reqwest::Client;
use reqwest::Identity;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::time::sleep;

use super::docs::ResourceContent;
use super::docs::ResourceMetadata;
use super::timeouts;

/// Maximum attempts for transient HTTP send failures in system tests.
const MAX_HTTP_SEND_ATTEMPTS: u32 = 3;
/// Base backoff delay for transient HTTP send retries.
const BASE_HTTP_SEND_RETRY_DELAY_MS: u64 = 50;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptEntry {
    pub sequence: u64,
    pub method: String,
    pub request: Value,
    pub response: Value,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
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
struct ToolCallResult {
    content: Vec<ToolContent>,
}

#[derive(Debug, Deserialize)]
struct ResourceReadResult {
    contents: Vec<ResourceContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    Json { json: Value },
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Option<Value>,
}

/// MCP HTTP client with transcript capture.
#[derive(Clone)]
pub struct McpHttpClient {
    base_url: String,
    client: Client,
    transcript: Arc<Mutex<Vec<TranscriptEntry>>>,
    bearer_token: Option<String>,
    client_subject: Option<String>,
}

impl McpHttpClient {
    /// Creates a new MCP HTTP client with a timeout.
    pub fn new(base_url: String, timeout: Duration) -> Result<Self, String> {
        let timeout = timeouts::resolve_timeout(timeout);
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| format!("failed to build http client: {err}"))?;
        Ok(Self::new_with_client(base_url, client))
    }

    /// Creates a new MCP HTTP client with a custom TLS configuration.
    pub fn new_with_tls(
        base_url: String,
        timeout: Duration,
        ca_pem: &[u8],
        identity_pem: Option<&[u8]>,
    ) -> Result<Self, String> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let timeout = timeouts::resolve_timeout(timeout);
        let mut builder = Client::builder().timeout(timeout);
        let cert =
            Certificate::from_pem(ca_pem).map_err(|err| format!("invalid ca cert: {err}"))?;
        builder = builder.add_root_certificate(cert);
        if let Some(identity_pem) = identity_pem {
            let identity = Identity::from_pem(identity_pem)
                .map_err(|err| format!("invalid client identity: {err}"))?;
            builder = builder.identity(identity);
        }
        let client =
            builder.build().map_err(|err| format!("failed to build tls client: {err:?}"))?;
        Ok(Self::new_with_client(base_url, client))
    }

    /// Creates a new MCP HTTP client from an existing reqwest client.
    pub fn new_with_client(base_url: String, client: Client) -> Self {
        Self {
            base_url,
            client,
            transcript: Arc::new(Mutex::new(Vec::new())),
            bearer_token: None,
            client_subject: None,
        }
    }

    /// Attaches a bearer token for Authorization headers.
    #[must_use]
    pub fn with_bearer_token(mut self, token: String) -> Self {
        self.bearer_token = Some(token);
        self
    }

    /// Attaches a client subject header for mTLS proxy auth.
    #[must_use]
    pub fn with_client_subject(mut self, subject: String) -> Self {
        self.client_subject = Some(subject);
        self
    }

    /// Returns the base URL for the MCP server.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns a snapshot of the transcript entries.
    pub fn transcript(&self) -> Vec<TranscriptEntry> {
        self.transcript.lock().map_or_else(|_| Vec::new(), |entries| entries.clone())
    }

    /// Issues a tools/list request.
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>, String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/list".to_string(),
            params: None,
        };
        let response = self.send_request(&request).await?;
        let result =
            response.result.ok_or_else(|| "missing result in tools/list response".to_string())?;
        let parsed: ToolListResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid tools/list payload: {err}"))?;
        Ok(parsed.tools)
    }

    /// Issues a tools/call request and returns the tool JSON payload.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/call".to_string(),
            params: Some(params),
        };
        let response = self.send_request(&request).await?;
        let result = response.result.ok_or_else(|| format!("missing result for tool {name}"))?;
        let parsed: ToolCallResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid tools/call payload for {name}: {err}"))?;
        let json = parsed
            .content
            .into_iter()
            .map(|item| match item {
                ToolContent::Json {
                    json,
                } => json,
            })
            .next()
            .ok_or_else(|| format!("tool {name} returned no json content"))?;
        Ok(json)
    }

    /// Issues a tools/call request and decodes the response into a type.
    pub async fn call_tool_typed<T: for<'de> Deserialize<'de>>(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<T, String> {
        let json = self.call_tool(name, arguments).await?;
        serde_json::from_value(json).map_err(|err| format!("decode {name} response: {err}"))
    }

    /// Issues a resources/list request.
    pub async fn list_resources(&self) -> Result<Vec<ResourceMetadata>, String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "resources/list".to_string(),
            params: None,
        };
        let response = self.send_request(&request).await?;
        let result = response
            .result
            .ok_or_else(|| "missing result in resources/list response".to_string())?;
        let parsed: ResourceListResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid resources/list payload: {err}"))?;
        Ok(parsed.resources)
    }

    /// Issues a resources/read request.
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceContent, String> {
        let params = serde_json::json!({
            "uri": uri,
        });
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "resources/read".to_string(),
            params: Some(params),
        };
        let response = self.send_request(&request).await?;
        let result = response.result.ok_or_else(|| format!("missing result for resource {uri}"))?;
        let parsed: ResourceReadResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid resources/read payload: {err}"))?;
        parsed
            .contents
            .into_iter()
            .next()
            .ok_or_else(|| format!("resource {uri} returned no content"))
    }

    async fn send_request(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        let request_value = serde_json::to_value(request)
            .map_err(|err| format!("jsonrpc serialization failed: {err}"))?;
        for attempt in 1..=MAX_HTTP_SEND_ATTEMPTS {
            let mut http_request = self.client.post(&self.base_url).json(&request_value);
            if let Some(token) = &self.bearer_token {
                http_request = http_request.bearer_auth(token);
            }
            if let Some(subject) = &self.client_subject {
                http_request = http_request.header("x-decision-gate-client-subject", subject);
            }

            let response = match http_request.send().await {
                Ok(response) => response,
                Err(err) => {
                    if should_retry_http_send(&err, attempt) {
                        sleep(retry_delay_for_attempt(attempt)).await;
                        continue;
                    }
                    return Err(format!("http request failed after {attempt} attempt(s): {err}"));
                }
            };
            let status = response.status();
            let payload = response
                .json::<JsonRpcResponse>()
                .await
                .map_err(|err| format!("invalid json-rpc response: {err}"))?;

            let error_message = payload.error.as_ref().map(|err| err.message.clone());
            self.record_transcript(
                request_value.clone(),
                serde_json::to_value(&payload).unwrap_or(Value::Null),
                error_message,
            );

            if let Some(error) = payload.error.as_ref() {
                return Err(error.message.clone());
            }
            if !status.is_success() {
                return Err(format!("http status {status} for json-rpc request"));
            }
            return Ok(payload);
        }

        Err("http request failed: exhausted retry attempts".to_string())
    }

    fn record_transcript(&self, request: Value, response: Value, error: Option<String>) {
        let Ok(mut guard) = self.transcript.lock() else {
            return;
        };
        let sequence = u64::try_from(guard.len()).unwrap_or(u64::MAX).saturating_add(1);
        guard.push(TranscriptEntry {
            sequence,
            method: request.get("method").and_then(Value::as_str).unwrap_or("unknown").to_string(),
            request,
            response,
            error,
        });
    }
}

/// Returns true when an HTTP send failure should be retried.
fn should_retry_http_send(err: &reqwest::Error, attempt: u32) -> bool {
    if attempt >= MAX_HTTP_SEND_ATTEMPTS {
        return false;
    }
    if err.is_connect() || err.is_timeout() {
        return true;
    }
    if !err.is_request() {
        return false;
    }
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("connection reset")
        || msg.contains("connection refused")
        || msg.contains("connection closed")
        || msg.contains("broken pipe")
        || msg.contains("connection aborted")
        || msg.contains("timed out")
        || msg.contains("eof")
}

/// Returns bounded linear backoff for HTTP send retries.
fn retry_delay_for_attempt(attempt: u32) -> Duration {
    Duration::from_millis(u64::from(attempt) * BASE_HTTP_SEND_RETRY_DELAY_MS)
}
