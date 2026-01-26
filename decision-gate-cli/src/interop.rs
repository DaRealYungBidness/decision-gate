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
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// ============================================================================
// SECTION: Public Types
// ============================================================================

/// Inputs required to run an interop evaluation.
#[derive(Debug, Clone)]
pub(crate) struct InteropConfig {
    /// Base URL for the MCP HTTP JSON-RPC endpoint.
    pub(crate) mcp_url: String,
    /// Scenario specification payload.
    pub(crate) spec: ScenarioSpec,
    /// Run configuration payload.
    pub(crate) run_config: RunConfig,
    /// Trigger event payload.
    pub(crate) trigger: TriggerEvent,
    /// Timestamp used for the scenario start request.
    pub(crate) started_at: Timestamp,
    /// Timestamp used for the status request.
    pub(crate) status_requested_at: Timestamp,
    /// Whether to issue entry packets on scenario start.
    pub(crate) issue_entry_packets: bool,
    /// Optional bearer token for MCP authentication.
    pub(crate) bearer_token: Option<String>,
    /// Optional client subject header for mTLS proxy auth.
    pub(crate) client_subject: Option<String>,
    /// MCP request timeout.
    pub(crate) timeout: Duration,
}

/// Transcript entry for each MCP JSON-RPC request/response pair.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TranscriptEntry {
    /// Monotonic sequence number for this entry.
    pub(crate) sequence: u64,
    /// JSON-RPC method invoked.
    pub(crate) method: String,
    /// Serialized request payload.
    pub(crate) request: Value,
    /// Serialized response payload.
    pub(crate) response: Value,
    /// Optional error string captured by the client.
    pub(crate) error: Option<String>,
}

/// Report emitted by the interop runner.
#[derive(Debug, Serialize)]
pub(crate) struct InteropReport {
    /// Scenario specification payload.
    pub(crate) spec: ScenarioSpec,
    /// Spec hash returned by the MCP server.
    pub(crate) spec_hash: HashDigest,
    /// Run configuration used to start the scenario.
    pub(crate) run_config: RunConfig,
    /// Timestamp used for scenario start.
    pub(crate) started_at: Timestamp,
    /// Trigger event used for evaluation.
    pub(crate) trigger: TriggerEvent,
    /// Timestamp used for status lookup.
    pub(crate) status_requested_at: Timestamp,
    /// Trigger evaluation result.
    pub(crate) trigger_result: TriggerResult,
    /// Final scenario status snapshot.
    pub(crate) status: ScenarioStatus,
    /// Captured MCP transcript.
    pub(crate) transcript: Vec<TranscriptEntry>,
}

// ============================================================================
// SECTION: Public Helpers
// ============================================================================

/// Validates that the interop inputs are internally consistent.
pub(crate) fn validate_inputs(
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
            trigger.tenant_id.as_str(),
            run_config.tenant_id.as_str()
        ));
    }
    if trigger.namespace_id != run_config.namespace_id {
        return Err(format!(
            "namespace_id mismatch: trigger={} run_config={}",
            trigger.namespace_id.as_str(),
            run_config.namespace_id.as_str()
        ));
    }
    Ok(())
}

/// Executes the interop workflow against the MCP server.
pub(crate) async fn run_interop(config: InteropConfig) -> Result<InteropReport, String> {
    let mut client = McpHttpClient::new(
        config.mcp_url,
        config.timeout,
        config.bearer_token,
        config.client_subject,
    )?;

    let define_input = ScenarioDefineRequest { spec: config.spec.clone() };
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
            tenant_id: config.run_config.tenant_id.clone(),
            namespace_id: config.run_config.namespace_id.clone(),
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

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Option<Value>,
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
struct ToolCallResult {
    content: Vec<ToolContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    Json { json: Value },
}

/// Minimal MCP HTTP client with transcript capture.
struct McpHttpClient {
    base_url: String,
    client: Client,
    transcript: Vec<TranscriptEntry>,
    next_id: u64,
    bearer_token: Option<String>,
    client_subject: Option<String>,
}

impl McpHttpClient {
    fn new(
        base_url: String,
        timeout: Duration,
        bearer_token: Option<String>,
        client_subject: Option<String>,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| format!("failed to build http client: {err}"))?;
        Ok(Self {
            base_url,
            client,
            transcript: Vec::new(),
            next_id: 1,
            bearer_token,
            client_subject,
        })
    }

    fn transcript(&self) -> Vec<TranscriptEntry> {
        self.transcript.clone()
    }

    async fn call_tool_typed<T: for<'de> Deserialize<'de>>(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<T, String> {
        let json = self.call_tool(name, arguments).await?;
        serde_json::from_value(json).map_err(|err| format!("decode {name} response: {err}"))
    }

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
        let json = match iter.next() {
            Some(ToolContent::Json { json }) => json,
            _ => return Err(format!("tool {name} returned no json content")),
        };
        Ok(json)
    }

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
        let body = response.text().await.map_err(|err| {
            let message = format!("failed to read response body: {err}");
            self.push_transcript(
                &request.method,
                request_value.clone(),
                Value::Null,
                Some(message.clone()),
            );
            message
        })?;
        let response_value: Value =
            serde_json::from_str(&body).unwrap_or_else(|_| Value::String(body.clone()));

        if !status.is_success() {
            let message = format!("http status {status} with body {body}");
            self.push_transcript(
                &request.method,
                request_value,
                response_value,
                Some(message.clone()),
            );
            return Err(message);
        }

        let parsed: JsonRpcResponse = serde_json::from_str(&body).map_err(|err| {
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

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

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
