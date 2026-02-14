// crates/decision-gate-mcp/src/evidence.rs
// ============================================================================
// Module: MCP Evidence Federation
// Description: Federated evidence provider spanning built-in and MCP sources.
// Purpose: Route evidence queries through policy enforcement and signature checks.
// Dependencies: decision-gate-core, decision-gate-providers, reqwest, serde_json
// ============================================================================

//! ## Overview
//! The federated evidence provider routes evidence queries to built-in providers
//! or external MCP providers. It enforces trust policies and signature
//! verification. Security posture: inputs are untrusted and must be validated;
//! see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as Base64;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceSignature;
use decision_gate_core::EvidenceValue;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::HashDigest;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_providers::ProviderRegistry;
use ed25519_dalek::Signature;
use ed25519_dalek::VerifyingKey;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::config::DecisionGateConfig;
use crate::config::ProviderConfig;
use crate::config::ProviderType;
use crate::config::TrustPolicy;
use crate::correlation::sanitize_client_correlation_id;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum size of MCP provider responses (bytes).
const MAX_MCP_PROVIDER_RESPONSE_BYTES: usize = 1024 * 1024;
/// JSON-RPC request id counter for MCP provider calls.
static JSON_RPC_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

// ============================================================================
// SECTION: Public Types
// ============================================================================

/// External MCP provider client configuration.
///
/// # Invariants
/// - `provider_id` uniquely identifies the provider in the registry.
#[derive(Debug, Clone)]
pub struct ProviderClientConfig {
    /// Provider identifier.
    pub provider_id: String,
    /// Command for stdio MCP providers.
    pub command: Vec<String>,
    /// HTTP URL for MCP providers.
    pub url: Option<String>,
    /// Allow insecure HTTP connections.
    pub allow_insecure_http: bool,
    /// Optional bearer token for HTTP providers.
    pub bearer_token: Option<String>,
}

/// Evidence provider that federates built-ins and MCP providers.
///
/// # Invariants
/// - Provider registry state is shared and synchronized via the inner Arc.
#[derive(Clone)]
pub struct FederatedEvidenceProvider {
    /// Shared registry and policy state.
    inner: Arc<FederatedInner>,
}

// ============================================================================
// SECTION: Internal Types
// ============================================================================

/// Shared registry and policy state for federated providers.
struct FederatedInner {
    /// Provider registry with built-ins and MCP clients.
    registry: ProviderRegistry,
    /// Per-provider policy overrides.
    policies: BTreeMap<String, ProviderPolicy>,
    /// Default policy when no override exists.
    default_policy: ProviderPolicy,
}

#[derive(Debug, Clone)]
/// Provider policy for trust and raw evidence disclosure.
struct ProviderPolicy {
    /// Trust requirements applied to responses.
    trust: ProviderTrust,
    /// Whether raw evidence may be returned.
    allow_raw: bool,
}

#[derive(Debug, Clone)]
/// Trust enforcement policy for providers.
enum ProviderTrust {
    /// Audit mode without signature enforcement.
    Audit,
    /// Require signatures with a configured key set.
    RequireSignature {
        /// Verifying keys indexed by identifier.
        keys: BTreeMap<String, VerifyingKey>,
    },
}

// ============================================================================
// SECTION: Construction
// ============================================================================

impl FederatedEvidenceProvider {
    /// Builds a federated provider from the MCP configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when provider configuration is invalid.
    pub fn from_config(config: &DecisionGateConfig) -> Result<Self, EvidenceError> {
        let mut registry =
            ProviderRegistry::new(decision_gate_providers::ProviderAccessPolicy::default());
        let mut policies = BTreeMap::new();

        let default_policy = ProviderPolicy {
            trust: parse_trust_policy(&config.trust.default_policy)?,
            allow_raw: false,
        };

        for provider in &config.providers {
            match provider.provider_type {
                ProviderType::Builtin => {
                    register_builtin_provider(&mut registry, provider)?;
                    policies.insert(
                        provider.name.clone(),
                        ProviderPolicy {
                            trust: parse_trust_override(provider, &default_policy)?,
                            allow_raw: provider.allow_raw,
                        },
                    );
                }
                ProviderType::Mcp => {
                    let client = McpProviderClient::from_config(provider)?;
                    registry.register_provider(provider.name.clone(), client)?;
                    policies.insert(
                        provider.name.clone(),
                        ProviderPolicy {
                            trust: parse_trust_override(provider, &default_policy)?,
                            allow_raw: provider.allow_raw,
                        },
                    );
                }
            }
        }

        Ok(Self {
            inner: Arc::new(FederatedInner {
                registry,
                policies,
                default_policy,
            }),
        })
    }

    /// Returns true if the provider allows raw values to be disclosed.
    #[must_use]
    pub fn provider_allows_raw(&self, provider_id: &str) -> bool {
        self.inner
            .policies
            .get(provider_id)
            .map_or(self.inner.default_policy.allow_raw, |policy| policy.allow_raw)
    }
}

impl EvidenceProvider for FederatedEvidenceProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let _ = sanitize_context_correlation_id(ctx)?;
        let provider_id = query.provider_id.as_str();
        let policy = self.inner.policies.get(provider_id).unwrap_or(&self.inner.default_policy);
        let mut result = self.inner.registry.query(query, ctx)?;
        apply_signature_policy(&policy.trust, &mut result)?;
        Ok(result)
    }

    fn validate_providers(&self, spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        self.inner.registry.validate_providers(spec)
    }
}

// ============================================================================
// SECTION: MCP Provider Client
// ============================================================================

#[derive(Debug)]
/// MCP provider client for stdio or HTTP transports.
struct McpProviderClient {
    /// Transport selection and state.
    transport: McpTransport,
    /// Optional bearer token for authentication.
    bearer_token: Option<String>,
}

#[derive(Debug)]
/// Transport variants for MCP provider connections.
enum McpTransport {
    /// Stdio transport with spawned process.
    Stdio {
        /// Spawned MCP process handle.
        process: Mutex<McpProcess>,
    },
    /// HTTP transport with base URL.
    Http {
        /// Base URL for MCP HTTP requests.
        url: String,
        /// HTTP client for MCP requests.
        client: Client,
    },
}

#[derive(Debug)]
/// Spawned MCP process handle and IO.
struct McpProcess {
    /// Child process handle.
    child: Child,
    /// Child stdin for request writes.
    stdin: ChildStdin,
    /// Buffered child stdout for response reads.
    stdout: BufReader<ChildStdout>,
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        if let Ok(Some(_)) = self.child.try_wait() {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.try_wait();
    }
}

impl McpProviderClient {
    /// Builds an MCP provider client from the configured transport settings.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] if the provider transport configuration is invalid.
    fn from_config(config: &ProviderConfig) -> Result<Self, EvidenceError> {
        let bearer_token = config.auth.as_ref().and_then(|auth| auth.bearer_token.clone());

        if !config.command.is_empty() {
            let process = spawn_mcp_process(&config.command)?;
            return Ok(Self {
                transport: McpTransport::Stdio {
                    process: Mutex::new(process),
                },
                bearer_token,
            });
        }

        let url = config
            .url
            .clone()
            .ok_or_else(|| EvidenceError::Provider("mcp url missing".to_string()))?;
        if !config.allow_insecure_http && url.starts_with("http://") {
            return Err(EvidenceError::Provider("insecure http disabled for provider".to_string()));
        }
        let timeouts = &config.timeouts;
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(timeouts.connect_timeout_ms))
            .timeout(Duration::from_millis(timeouts.request_timeout_ms))
            .build()
            .map_err(|_| EvidenceError::Provider("http client build failed".to_string()))?;

        Ok(Self {
            transport: McpTransport::Http {
                url,
                client,
            },
            bearer_token,
        })
    }
}

impl EvidenceProvider for McpProviderClient {
    fn query(
        &self,
        query: &EvidenceQuery,
        ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let params = serde_json::json!({
            "name": "evidence_query",
            "arguments": {
                "query": query,
                "context": ctx,
            }
        });
        let correlation_id = sanitize_context_correlation_id(ctx)?;
        let request_id = request_id_for_context(correlation_id.as_deref());
        let request = JsonRpcRequest::new("tools/call", params, request_id);
        let response = match &self.transport {
            McpTransport::Http {
                url,
                client,
            } => call_http(
                client,
                url,
                self.bearer_token.as_deref(),
                &request,
                correlation_id.as_deref(),
            )?,
            McpTransport::Stdio {
                process,
            } => call_stdio(process, &request)?,
        };
        let response = decode_tool_response(response)?;
        Ok(response)
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

// ============================================================================
// SECTION: JSON-RPC Helpers
// ============================================================================

/// JSON-RPC request envelope for MCP tool calls.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    /// JSON-RPC protocol version.
    jsonrpc: &'static str,
    /// Request identifier.
    id: Value,
    /// Remote method name.
    method: String,
    /// Request parameters payload.
    params: Value,
}

impl JsonRpcRequest {
    /// Builds a JSON-RPC request with the provided identifier.
    fn new(method: &str, params: Value, id: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// JSON-RPC response envelope for MCP tool calls.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    /// Successful result payload.
    result: Option<Value>,
    /// Error payload when the request fails.
    error: Option<JsonRpcError>,
}

/// JSON-RPC error payload.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    /// Human-readable error message.
    message: String,
}

/// Tool call response wrapper.
#[derive(Debug, Deserialize)]
struct ToolCallResult {
    /// Tool content variants emitted by the MCP provider.
    content: Vec<ToolContent>,
}

/// Tool content variants for MCP responses.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    /// JSON evidence response payload.
    Json {
        /// Evidence result payload.
        json: Box<EvidenceResult>,
    },
    /// Text responses are rejected for evidence queries.
    Text {
        /// Text content emitted by the provider.
        text: String,
    },
}

/// Builds a deterministic JSON-RPC request ID for an evidence request.
fn request_id_for_context(correlation_id: Option<&str>) -> Value {
    if let Some(correlation_id) = correlation_id {
        return Value::String(correlation_id.to_string());
    }
    Value::Number(serde_json::Number::from(JSON_RPC_ID_COUNTER.fetch_add(1, Ordering::Relaxed)))
}

/// Extracts and sanitizes the correlation ID from an evidence context.
fn sanitize_context_correlation_id(ctx: &EvidenceContext) -> Result<Option<String>, EvidenceError> {
    sanitize_client_correlation_id(
        ctx.correlation_id.as_ref().map(decision_gate_core::CorrelationId::as_str),
    )
    .map_err(|_| EvidenceError::Provider("invalid correlation id".to_string()))
}

/// Executes a JSON-RPC tool call over HTTP.
fn call_http(
    client: &Client,
    url: &str,
    bearer: Option<&str>,
    request: &JsonRpcRequest,
    correlation_id: Option<&str>,
) -> Result<JsonRpcResponse, EvidenceError> {
    let mut builder = client.post(url).json(request);
    if let Some(token) = bearer {
        builder = builder.bearer_auth(token);
    }
    if let Some(value) = correlation_id {
        builder = builder.header("x-correlation-id", value);
    }
    let mut response = builder.send().map_err(|err| map_http_send_error(&err))?;
    if !response.status().is_success() {
        return Err(EvidenceError::Provider(format!(
            "http request failed with status {}",
            response.status()
        )));
    }
    let bytes = read_http_body(&mut response, MAX_MCP_PROVIDER_RESPONSE_BYTES)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| EvidenceError::Provider("invalid json-rpc response".to_string()))
}

/// Executes a JSON-RPC tool call over stdio with a locked process handle.
fn call_stdio(
    process: &Mutex<McpProcess>,
    request: &JsonRpcRequest,
) -> Result<JsonRpcResponse, EvidenceError> {
    let mut guard = process
        .lock()
        .map_err(|_| EvidenceError::Provider("mcp process lock poisoned".to_string()))?;
    if let Some(status) = guard
        .child
        .try_wait()
        .map_err(|_| EvidenceError::Provider("mcp process state unavailable".to_string()))?
    {
        return Err(EvidenceError::Provider(format!("mcp process exited: {status}")));
    }
    let payload = serde_json::to_vec(request)
        .map_err(|_| EvidenceError::Provider("mcp request serialization failed".to_string()))?;
    write_framed(&mut guard.stdin, &payload)?;
    let response_bytes = read_framed(&mut guard.stdout, MAX_MCP_PROVIDER_RESPONSE_BYTES)?;
    drop(guard);
    serde_json::from_slice(&response_bytes)
        .map_err(|_| EvidenceError::Provider("invalid json-rpc response".to_string()))
}

/// Writes a JSON-RPC payload with the MCP framing header.
fn write_framed(writer: &mut ChildStdin, payload: &[u8]) -> Result<(), EvidenceError> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer
        .write_all(header.as_bytes())
        .map_err(|_| EvidenceError::Provider("mcp write failed".to_string()))?;
    writer
        .write_all(payload)
        .map_err(|_| EvidenceError::Provider("mcp write failed".to_string()))?;
    writer.flush().map_err(|_| EvidenceError::Provider("mcp write failed".to_string()))
}

/// Reads a framed JSON-RPC payload from the MCP process.
fn read_framed(
    reader: &mut BufReader<impl Read>,
    max_body_bytes: usize,
) -> Result<Vec<u8>, EvidenceError> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|_| EvidenceError::Provider("mcp read failed".to_string()))?;
        if bytes == 0 {
            return Err(EvidenceError::Provider("mcp connection closed".to_string()));
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            let trimmed = value.trim();
            let parsed = trimmed
                .parse::<usize>()
                .map_err(|_| EvidenceError::Provider("invalid content length".to_string()))?;
            content_length = Some(parsed);
        }
    }
    let len = content_length
        .ok_or_else(|| EvidenceError::Provider("missing content length header".to_string()))?;
    if len > max_body_bytes {
        return Err(EvidenceError::Provider("mcp response too large".to_string()));
    }
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|_| EvidenceError::Provider("mcp read failed".to_string()))?;
    Ok(buf)
}

/// Reads an HTTP response body with a maximum size limit.
fn read_http_body(
    response: &mut reqwest::blocking::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, EvidenceError> {
    let max_bytes_u64 = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if let Some(length) = response.content_length()
        && length > max_bytes_u64
    {
        return Err(EvidenceError::Provider("http response too large".to_string()));
    }
    let mut limited = response.take(max_bytes_u64.saturating_add(1));
    let mut buf = Vec::new();
    limited.read_to_end(&mut buf).map_err(|err| {
        if err.kind() == std::io::ErrorKind::TimedOut {
            EvidenceError::Provider("http response timed out".to_string())
        } else {
            EvidenceError::Provider("http response read failed".to_string())
        }
    })?;
    if buf.len() > max_bytes {
        return Err(EvidenceError::Provider("http response too large".to_string()));
    }
    Ok(buf)
}

// (header sanitization handled via crate::correlation)

/// Maps reqwest send errors to stable provider error messages.
fn map_http_send_error(error: &reqwest::Error) -> EvidenceError {
    if error.is_timeout() {
        EvidenceError::Provider("http request timed out".to_string())
    } else {
        EvidenceError::Provider("http request failed".to_string())
    }
}

/// Decodes the MCP tool response into an evidence result.
fn decode_tool_response(response: JsonRpcResponse) -> Result<EvidenceResult, EvidenceError> {
    if let Some(error) = response.error {
        return Err(EvidenceError::Provider(error.message));
    }
    let Some(result) = response.result else {
        return Err(EvidenceError::Provider("missing tool result".to_string()));
    };
    let tool_result: ToolCallResult = serde_json::from_value(result)
        .map_err(|_| EvidenceError::Provider("invalid tool result".to_string()))?;
    let Some(content) = tool_result.content.into_iter().next() else {
        return Err(EvidenceError::Provider("empty tool result".to_string()));
    };
    match content {
        ToolContent::Json {
            json,
        } => Ok(*json),
        ToolContent::Text {
            text,
        } => Err(EvidenceError::Provider(format!("unexpected text response: {text}"))),
    }
}

/// Spawns an MCP provider process for stdio transport.
fn spawn_mcp_process(command: &[String]) -> Result<McpProcess, EvidenceError> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| EvidenceError::Provider("mcp command is empty".to_string()))?;
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|_| EvidenceError::Provider("failed to spawn mcp provider".to_string()))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| EvidenceError::Provider("mcp stdin unavailable".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| EvidenceError::Provider("mcp stdout unavailable".to_string()))?;
    Ok(McpProcess {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

// ============================================================================
// SECTION: Trust Policy Enforcement
// ============================================================================

/// Returns the provider-specific trust policy or the default.
fn parse_trust_override(
    provider: &ProviderConfig,
    default_policy: &ProviderPolicy,
) -> Result<ProviderTrust, EvidenceError> {
    provider.trust.as_ref().map_or_else(|| Ok(default_policy.trust.clone()), parse_trust_policy)
}

/// Parses the trust policy for an MCP provider.
fn parse_trust_policy(policy: &TrustPolicy) -> Result<ProviderTrust, EvidenceError> {
    match policy {
        TrustPolicy::Audit => Ok(ProviderTrust::Audit),
        TrustPolicy::RequireSignature {
            keys,
        } => {
            let mut key_map = BTreeMap::new();
            for key_path in keys {
                let key = load_public_key(key_path)?;
                key_map.insert(key_path.clone(), key);
            }
            Ok(ProviderTrust::RequireSignature {
                keys: key_map,
            })
        }
    }
}

/// Applies trust requirements to a provider response.
fn apply_signature_policy(
    policy: &ProviderTrust,
    result: &mut EvidenceResult,
) -> Result<(), EvidenceError> {
    match policy {
        ProviderTrust::Audit => Ok(()),
        ProviderTrust::RequireSignature {
            keys,
        } => {
            let signature = result
                .signature
                .clone()
                .ok_or_else(|| EvidenceError::Provider("missing evidence signature".to_string()))?;
            if signature.scheme != "ed25519" {
                return Err(EvidenceError::Provider("unsupported signature scheme".to_string()));
            }
            let key = keys.get(&signature.key_id).ok_or_else(|| {
                EvidenceError::Provider("signature key not authorized".to_string())
            })?;
            let hash = ensure_evidence_hash(result)?;
            verify_signature(key, &hash, &signature)?;
            Ok(())
        }
    }
}

/// Ensures an evidence hash is present for signature verification.
///
/// Exposed as `pub` for unit testing in integration tests.
///
/// # Errors
///
/// Returns [`EvidenceError`] when the evidence payload is missing, hashing fails,
/// or a provided hash does not match the computed value.
pub fn ensure_evidence_hash(result: &mut EvidenceResult) -> Result<HashDigest, EvidenceError> {
    let Some(value) = &result.value else {
        return Err(EvidenceError::Provider(
            "missing evidence hash for signature verification".to_string(),
        ));
    };
    let computed = match value {
        EvidenceValue::Json(json) => {
            let bytes = canonical_json_bytes(json)
                .map_err(|_| EvidenceError::Provider("hashing failed".to_string()))?;
            hash_bytes(HashAlgorithm::Sha256, &bytes)
        }
        EvidenceValue::Bytes(bytes) => hash_bytes(HashAlgorithm::Sha256, bytes),
    };
    if let Some(hash) = &result.evidence_hash {
        if hash != &computed {
            return Err(EvidenceError::Provider("evidence hash mismatch".to_string()));
        }
        return Ok(hash.clone());
    }
    result.evidence_hash = Some(computed.clone());
    Ok(computed)
}

/// Verifies a signature against the evidence hash.
fn verify_signature(
    key: &VerifyingKey,
    hash: &HashDigest,
    signature: &EvidenceSignature,
) -> Result<(), EvidenceError> {
    let message = canonical_json_bytes(hash)
        .map_err(|_| EvidenceError::Provider("signature hash serialization failed".to_string()))?;
    let signature = Signature::try_from(signature.signature.as_slice())
        .map_err(|_| EvidenceError::Provider("invalid signature bytes".to_string()))?;
    key.verify_strict(&message, &signature)
        .map_err(|_| EvidenceError::Provider("signature verification failed".to_string()))
}

/// Loads an ed25519 public key from disk.
fn load_public_key(path: &str) -> Result<VerifyingKey, EvidenceError> {
    let bytes = fs::read(path)
        .map_err(|_| EvidenceError::Provider("unable to read public key".to_string()))?;
    let key_bytes = if bytes.len() == 32 {
        bytes
    } else {
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| EvidenceError::Provider("public key must be utf-8".to_string()))?;
        Base64
            .decode(text.trim())
            .map_err(|_| EvidenceError::Provider("invalid base64 public key".to_string()))?
    };
    let key_bytes: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| EvidenceError::Provider("invalid ed25519 public key".to_string()))?;
    VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| EvidenceError::Provider("invalid ed25519 public key".to_string()))
}

// ============================================================================
// SECTION: Built-in Provider Registration
// ============================================================================

/// Registers a built-in provider with validated configuration.
fn register_builtin_provider(
    registry: &mut ProviderRegistry,
    provider: &ProviderConfig,
) -> Result<(), EvidenceError> {
    match provider.name.as_str() {
        "time" => {
            let config = provider
                .parse_config::<decision_gate_providers::TimeProviderConfig>()
                .map_err(|err| EvidenceError::Provider(err.to_string()))?;
            registry
                .register_provider("time", decision_gate_providers::TimeProvider::new(config))?;
        }
        "env" => {
            let config = provider
                .parse_config::<decision_gate_providers::EnvProviderConfig>()
                .map_err(|err| EvidenceError::Provider(err.to_string()))?;
            registry.register_provider("env", decision_gate_providers::EnvProvider::new(config))?;
        }
        "json" => {
            let config = provider
                .parse_config::<decision_gate_providers::JsonProviderConfig>()
                .map_err(|err| EvidenceError::Provider(err.to_string()))?;
            let provider = decision_gate_providers::JsonProvider::new(config)?;
            registry.register_provider("json", provider)?;
        }
        "http" => {
            let config = provider
                .parse_config::<decision_gate_providers::HttpProviderConfig>()
                .map_err(|err| EvidenceError::Provider(err.to_string()))?;
            let provider = decision_gate_providers::HttpProvider::new(config)?;
            registry.register_provider("http", provider)?;
        }
        _ => {
            return Err(EvidenceError::Provider(format!(
                "unknown builtin provider: {}",
                provider.name
            )));
        }
    }
    Ok(())
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests;
