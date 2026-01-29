// decision-gate-cli/src/interop.rs
// ============================================================================
// Module: Decision Gate CLI Interop Runner
// Description: MCP HTTP driver for deterministic interop evaluation.
// Purpose: Execute scenario define/start/trigger/status with transcript capture.
// Dependencies: decision-gate-core, decision-gate-mcp, reqwest, serde
// ============================================================================

//! ## Overview
//! Runs a deterministic interop workflow against a Decision Gate MCP server.
//! The runner builds MCP tool payloads from explicit inputs and emits a
//! canonical JSON report suitable for automation and audits.
//!
//! ## Invariants
//! - Inputs are explicit; no wall-clock timestamps are generated here.
//! - Scenario/run identifiers must match across spec, run config, and trigger.
//! - MCP transcripts capture every tool call in order.
//!
//! Security posture: HTTP responses are untrusted; enforce strict size limits
//! and fail closed on malformed responses (see `Docs/security/threat_model.md`).

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::time::Duration;

use decision_gate_core::HashDigest;
use decision_gate_core::RunConfig;
use decision_gate_core::RunState;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use decision_gate_core::runtime::ScenarioStatus;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use reqwest::Client;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// ============================================================================
// SECTION: Limits
// ============================================================================

/// Maximum response body size accepted from MCP HTTP servers.
pub const MAX_INTEROP_RESPONSE_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum response body preview included in error strings.
const MAX_INTEROP_ERROR_BODY_BYTES: usize = 2048;

// ============================================================================
// SECTION: Public Types
// ============================================================================

/// Inputs required to run an interop evaluation.
#[derive(Debug, Clone)]
pub struct InteropConfig {
    /// Base URL for the MCP HTTP JSON-RPC endpoint.
    pub mcp_url: String,
    /// Scenario specification payload.
    pub spec: ScenarioSpec,
    /// Run configuration payload.
    pub run_config: RunConfig,
    /// Trigger event payload.
    pub trigger: TriggerEvent,
    /// Timestamp used for the scenario start request.
    pub started_at: Timestamp,
    /// Timestamp used for the status request.
    pub status_requested_at: Timestamp,
    /// Whether to issue entry packets on scenario start.
    pub issue_entry_packets: bool,
    /// Optional bearer token for MCP authentication.
    pub bearer_token: Option<String>,
    /// Optional client subject header for mTLS proxy auth.
    pub client_subject: Option<String>,
    /// MCP request timeout.
    pub timeout: Duration,
}

/// Transcript entry for each MCP JSON-RPC request/response pair.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptEntry {
    /// Monotonic sequence number for this entry.
    pub sequence: u64,
    /// JSON-RPC method invoked.
    pub method: String,
    /// Serialized request payload.
    pub request: Value,
    /// Serialized response payload.
    pub response: Value,
    /// Optional error string captured by the client.
    pub error: Option<String>,
}

/// Report emitted by the interop runner.
#[derive(Debug, Serialize)]
pub struct InteropReport {
    /// Scenario specification payload.
    pub spec: ScenarioSpec,
    /// Spec hash returned by the MCP server.
    pub spec_hash: HashDigest,
    /// Run configuration used to start the scenario.
    pub run_config: RunConfig,
    /// Timestamp used for scenario start.
    pub started_at: Timestamp,
    /// Trigger event used for evaluation.
    pub trigger: TriggerEvent,
    /// Timestamp used for status lookup.
    pub status_requested_at: Timestamp,
    /// Trigger evaluation result.
    pub trigger_result: TriggerResult,
    /// Final scenario status snapshot.
    pub status: ScenarioStatus,
    /// Captured MCP transcript.
    pub transcript: Vec<TranscriptEntry>,
}

// ============================================================================
// SECTION: Public Helpers
// ============================================================================

/// Validates that the interop inputs are internally consistent.
///
/// # Errors
///
/// Returns an error when scenario, run, tenant, or namespace identifiers do not match.
pub fn validate_inputs(
    spec: &ScenarioSpec,
    run_config: &RunConfig,
    trigger: &TriggerEvent,
) -> Result<(), String> {
    if spec.scenario_id != run_config.scenario_id {
        return Err(format!(
            "scenario_id mismatch: spec={} run_config={}",
            spec.scenario_id.as_str(),
            run_config.scenario_id.as_str()
        ));
    }
    if trigger.run_id != run_config.run_id {
        return Err(format!(
            "run_id mismatch: trigger={} run_config={}",
            trigger.run_id.as_str(),
            run_config.run_id.as_str()
        ));
    }
    if trigger.tenant_id != run_config.tenant_id {
        return Err(format!(
            "tenant_id mismatch: trigger={} run_config={}",
            trigger.tenant_id, run_config.tenant_id
        ));
    }
    if trigger.namespace_id != run_config.namespace_id {
        return Err(format!(
            "namespace_id mismatch: trigger={} run_config={}",
            trigger.namespace_id, run_config.namespace_id
        ));
    }
    Ok(())
}

/// Executes the interop workflow against the MCP server.
///
/// # Errors
///
/// Returns an error when request serialization, transport, or server responses fail.
pub async fn run_interop(config: InteropConfig) -> Result<InteropReport, String> {
    let mut client = McpHttpClient::new(
        config.mcp_url,
        config.timeout,
        config.bearer_token,
        config.client_subject,
    )?;

    let define_input = ScenarioDefineRequest {
        spec: config.spec.clone(),
    };
    let define_value =
        serde_json::to_value(&define_input).map_err(|err| format!("define payload: {err}"))?;
    let define_response: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_value).await?;

    if define_response.scenario_id != config.spec.scenario_id {
        return Err(format!(
            "scenario_define returned unexpected scenario_id: expected={} actual={}",
            config.spec.scenario_id.as_str(),
            define_response.scenario_id.as_str()
        ));
    }

    let start_request = ScenarioStartRequest {
        scenario_id: define_response.scenario_id.clone(),
        run_config: config.run_config.clone(),
        started_at: config.started_at,
        issue_entry_packets: config.issue_entry_packets,
    };
    let start_value =
        serde_json::to_value(&start_request).map_err(|err| format!("start payload: {err}"))?;
    let _state: RunState = client
        .call_tool_typed("scenario_start", start_value)
        .await
        .map_err(|err| format!("scenario_start failed: {err}"))?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_response.scenario_id.clone(),
        trigger: config.trigger.clone(),
    };
    let trigger_value =
        serde_json::to_value(&trigger_request).map_err(|err| format!("trigger payload: {err}"))?;
    let trigger_result: TriggerResult = client
        .call_tool_typed("scenario_trigger", trigger_value)
        .await
        .map_err(|err| format!("scenario_trigger failed: {err}"))?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_response.scenario_id,
        request: StatusRequest {
            tenant_id: config.run_config.tenant_id,
            namespace_id: config.run_config.namespace_id,
            run_id: config.run_config.run_id.clone(),
            requested_at: config.status_requested_at,
            correlation_id: config.trigger.correlation_id.clone(),
        },
    };
    let status_value =
        serde_json::to_value(&status_request).map_err(|err| format!("status payload: {err}"))?;
    let status: ScenarioStatus = client
        .call_tool_typed("scenario_status", status_value)
        .await
        .map_err(|err| format!("scenario_status failed: {err}"))?;

    Ok(InteropReport {
        spec: config.spec,
        spec_hash: define_response.spec_hash,
        run_config: config.run_config,
        started_at: config.started_at,
        trigger: config.trigger,
        status_requested_at: config.status_requested_at,
        trigger_result,
        status,
        transcript: client.transcript(),
    })
}

// ============================================================================
// SECTION: MCP HTTP Client
// ============================================================================

/// JSON-RPC request envelope for tool calls.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    /// JSON-RPC protocol version.
    jsonrpc: &'static str,
    /// Monotonic request identifier.
    id: u64,
    /// Remote method name.
    method: String,
    /// Optional request parameters.
    params: Option<Value>,
}

/// JSON-RPC response envelope for tool calls.
#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcResponse {
    /// Successful response payload.
    result: Option<Value>,
    /// Error payload if the request failed.
    error: Option<JsonRpcError>,
}

/// JSON-RPC error payload.
#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    /// JSON-RPC error code.
    code: i64,
    /// Error message describing the failure.
    message: String,
    #[serde(default)]
    /// Optional error data payload.
    data: Option<Value>,
}

/// Tool call response wrapper for MCP.
#[derive(Debug, Deserialize)]
struct ToolCallResult {
    /// Content blocks returned by the tool.
    content: Vec<ToolContent>,
}

/// Tool content payload variants.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    /// JSON payload content.
    Json {
        /// JSON value returned by the tool.
        json: Value,
    },
}

/// Minimal MCP HTTP client with transcript capture.
struct McpHttpClient {
    /// MCP base URL for JSON-RPC.
    base_url: String,
    /// HTTP client instance.
    client: Client,
    /// Recorded transcript entries.
    transcript: Vec<TranscriptEntry>,
    /// Next JSON-RPC request id.
    next_id: u64,
    /// Optional bearer token.
    bearer_token: Option<String>,
    /// Optional client subject for downstream auth.
    client_subject: Option<String>,
    /// Maximum response body size.
    max_response_bytes: usize,
}

impl McpHttpClient {
    /// Builds a new HTTP client for MCP tool calls.
    fn new(
        base_url: String,
        timeout: Duration,
        bearer_token: Option<String>,
        client_subject: Option<String>,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(timeout)
            .redirect(Policy::none())
            .build()
            .map_err(|err| format!("failed to build http client: {err}"))?;
        Ok(Self {
            base_url,
            client,
            transcript: Vec::new(),
            next_id: 1,
            bearer_token,
            client_subject,
            max_response_bytes: MAX_INTEROP_RESPONSE_BYTES,
        })
    }

    /// Returns the recorded request/response transcript.
    fn transcript(&self) -> Vec<TranscriptEntry> {
        self.transcript.clone()
    }

    /// Calls a tool and decodes the typed JSON response.
    async fn call_tool_typed<T: for<'de> Deserialize<'de>>(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<T, String> {
        let json = self.call_tool(name, arguments).await?;
        serde_json::from_value(json).map_err(|err| format!("decode {name} response: {err}"))
    }

    /// Calls a tool and returns the raw JSON response content.
    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: self.next_id(),
            method: "tools/call".to_string(),
            params: Some(params),
        };
        let response = self.send_request(&request).await?;
        let result = response.result.ok_or_else(|| format!("missing result for tool {name}"))?;
        let parsed: ToolCallResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid tools/call payload for {name}: {err}"))?;
        let mut iter = parsed.content.into_iter();
        let Some(ToolContent::Json {
            json,
        }) = iter.next()
        else {
            return Err(format!("tool {name} returned no json content"));
        };
        Ok(json)
    }

    /// Sends a JSON-RPC request and returns the parsed response.
    async fn send_request(&mut self, request: &JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        let request_value = serde_json::to_value(request)
            .map_err(|err| format!("serialize json-rpc request: {err}"))?;
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.bearer_token {
            let value = format!("Bearer {token}");
            let header = HeaderValue::from_str(&value)
                .map_err(|err| format!("invalid bearer token header: {err}"))?;
            headers.insert(AUTHORIZATION, header);
        }
        if let Some(subject) = &self.client_subject {
            let header = HeaderValue::from_str(subject)
                .map_err(|err| format!("invalid client subject header: {err}"))?;
            headers.insert("x-decision-gate-client-subject", header);
        }

        let response = self.client.post(&self.base_url).headers(headers).json(request).send().await;

        let response = match response {
            Ok(response) => response,
            Err(err) => {
                self.push_transcript(
                    &request.method,
                    request_value,
                    Value::Null,
                    Some(err.to_string()),
                );
                return Err(format!("http request failed: {err}"));
            }
        };

        let status = response.status();
        let mut response = response;
        let body_bytes = match self.read_body_with_limit(&mut response).await {
            Ok(bytes) => bytes,
            Err(message) => {
                self.push_transcript(
                    &request.method,
                    request_value.clone(),
                    Value::Null,
                    Some(message.clone()),
                );
                return Err(message);
            }
        };
        let response_value: Value = serde_json::from_slice(&body_bytes)
            .unwrap_or_else(|_| Value::String(Self::body_preview(&body_bytes)));

        if !status.is_success() {
            let body_preview = Self::body_preview(&body_bytes);
            let message = format!("http status {status} with body {body_preview}");
            self.push_transcript(
                &request.method,
                request_value,
                response_value,
                Some(message.clone()),
            );
            return Err(message);
        }

        let parsed: JsonRpcResponse =
            serde_json::from_value(response_value.clone()).map_err(|err| {
                let message = format!("invalid json-rpc response: {err}");
                self.push_transcript(
                    &request.method,
                    request_value.clone(),
                    response_value.clone(),
                    Some(message.clone()),
                );
                message
            })?;

        if let Some(err) = &parsed.error {
            let message = format!("json-rpc error {}: {}", err.code, err.message);
            self.push_transcript(
                &request.method,
                request_value,
                response_value,
                Some(message.clone()),
            );
            return Err(message);
        }

        self.push_transcript(&request.method, request_value, response_value, None);
        Ok(parsed)
    }

    /// Reads the response body while enforcing a strict size limit.
    async fn read_body_with_limit(
        &self,
        response: &mut reqwest::Response,
    ) -> Result<Vec<u8>, String> {
        let limit = u64::try_from(self.max_response_bytes)
            .map_err(|_| "response size limit out of range".to_string())?;
        if let Some(length) = response.content_length()
            && length > limit
        {
            return Err(format!("response body exceeds size limit ({length} > {limit})"));
        }

        let mut body = Vec::new();
        while let Some(chunk) =
            response.chunk().await.map_err(|err| format!("failed to read response body: {err}"))?
        {
            let next_len = body
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| "response body exceeds size limit".to_string())?;
            if next_len > self.max_response_bytes {
                return Err(format!(
                    "response body exceeds size limit ({next_len} > {})",
                    self.max_response_bytes
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    /// Produces a bounded UTF-8 preview of response bodies for error reporting.
    fn body_preview(bytes: &[u8]) -> String {
        if bytes.is_empty() {
            return String::new();
        }
        let preview_len = bytes.len().min(MAX_INTEROP_ERROR_BODY_BYTES);
        let preview = String::from_utf8_lossy(&bytes[.. preview_len]);
        if bytes.len() > preview_len {
            let remaining = bytes.len() - preview_len;
            format!("{preview}...[truncated {remaining} bytes]")
        } else {
            preview.to_string()
        }
    }

    /// Returns the next JSON-RPC request identifier.
    const fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Appends a request/response pair to the transcript.
    fn push_transcript(
        &mut self,
        method: &str,
        request: Value,
        response: Value,
        error: Option<String>,
    ) {
        let sequence = u64::try_from(self.transcript.len()).unwrap_or(u64::MAX) + 1;
        self.transcript.push(TranscriptEntry {
            sequence,
            method: method.to_string(),
            request,
            response,
            error,
        });
    }
}
