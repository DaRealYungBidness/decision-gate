// system-tests/tests/helpers/auth_proxy.rs
// ============================================================================
// Module: Auth Mapping Proxy
// Description: Stub integration-layer auth mapper for ASC role testing.
// Purpose: Enforce role-to-tool mapping before forwarding to DG MCP server.
// Dependencies: axum, decision-gate-contract, reqwest, serde
// ============================================================================

use std::collections::BTreeSet;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::thread;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use decision_gate_contract::ToolName;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::runtime::Builder;
use tokio::sync::oneshot;


pub const ROLE_HEADER: &str = "x-asc-roles";
pub const POLICY_CLASS_HEADER: &str = "x-asc-policy-class";

#[derive(Clone)]
struct ProxyState {
    target_url: String,
    client: Client,
}

/// Handle for the auth mapping proxy server.
pub struct AuthProxyHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<thread::JoinHandle<()>>,
}

impl AuthProxyHandle {
    /// Returns the base URL for the proxy server.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for AuthProxyHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Spawn an auth mapping proxy that forwards to a Decision Gate MCP server.
pub async fn spawn_auth_proxy(target_url: String) -> Result<AuthProxyHandle, String> {
    let listener = StdTcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("auth proxy bind failed: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("auth proxy listener nonblocking failed: {err}"))?;
    let addr =
        listener.local_addr().map_err(|err| format!("auth proxy local addr failed: {err}"))?;
    let base_url = format!("http://{}", addr);

    let state = ProxyState {
        target_url,
        client: Client::new(),
    };
    let app = Router::new().route("/rpc", post(handle_rpc)).with_state(Arc::new(state));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join = thread::spawn(move || {
        let runtime =
            Builder::new_current_thread().enable_all().build().expect("auth proxy runtime");
        runtime.block_on(async move {
            let listener =
                tokio::net::TcpListener::from_std(listener).expect("auth proxy listener from_std");
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });
    });
    Ok(AuthProxyHandle {
        base_url,
        shutdown: Some(shutdown_tx),
        join: Some(join),
    })
}

#[derive(Debug, Deserialize, Serialize)]
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
    name: ToolName,
    arguments: Value,
}

#[derive(Debug, Deserialize, Serialize)]
struct ToolListResult {
    tools: Vec<ToolListEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ToolListEntry {
    name: ToolName,
    #[serde(flatten)]
    rest: Value,
}

async fn handle_rpc(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let request: Result<JsonRpcRequest, _> = serde_json::from_slice(bytes.as_ref());
    let response = match request {
        Ok(request) => {
            let allowed = match resolve_permissions(&headers) {
                Ok(allowed) => allowed,
                Err(message) => {
                    return axum::Json(JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32001,
                            message,
                        }),
                    });
                }
            };
            route_request(&state, request, &allowed, headers).await
        }
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

async fn route_request(
    state: &ProxyState,
    request: JsonRpcRequest,
    allowed: &BTreeSet<ToolName>,
    headers: HeaderMap,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "tools/list" => match forward_request(state, &request, headers).await {
            Ok(result) => filter_tools_list(request.id, result, allowed),
            Err(message) => JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32003,
                    message,
                }),
            },
        },
        "tools/call" => {
            let params = request.params.clone().unwrap_or(Value::Null);
            let call: Result<ToolCallParams, _> = serde_json::from_value(params);
            match call {
                Ok(call) => {
                    if !allowed.contains(&call.name) {
                        return JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32003,
                                message: "unauthorized".to_string(),
                            }),
                        };
                    }
                    match forward_request(state, &request, headers).await {
                        Ok(result) => JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: request.id,
                            result: Some(result),
                            error: None,
                        },
                        Err(message) => JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32003,
                                message,
                            }),
                        },
                    }
                }
                Err(_) => JsonRpcResponse {
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

async fn forward_request(
    state: &ProxyState,
    request: &JsonRpcRequest,
    headers: HeaderMap,
) -> Result<Value, String> {
    let mut builder = state.client.post(&state.target_url).json(&request);
    if let Some(correlation) = headers.get("x-correlation-id") {
        builder = builder.header("x-correlation-id", correlation);
    }
    let response = builder.send().await.map_err(|err| err.to_string())?;
    let payload: Value = response.json().await.map_err(|err| err.to_string())?;
    if let Some(result) = payload.get("result") {
        return Ok(result.clone());
    }
    if let Some(error) = payload.get("error") {
        if let Some(message) = error.get("message").and_then(Value::as_str) {
            return Err(message.to_string());
        }
    }
    Err("missing result in upstream response".to_string())
}

fn filter_tools_list(id: Value, result: Value, allowed: &BTreeSet<ToolName>) -> JsonRpcResponse {
    let parsed: Result<ToolListResult, _> = serde_json::from_value(result.clone());
    match parsed {
        Ok(mut parsed) => {
            parsed.tools.retain(|tool| allowed.contains(&tool.name));
            let filtered = serde_json::to_value(parsed).unwrap_or(Value::Null);
            JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(filtered),
                error: None,
            }
        }
        Err(_) => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        },
    }
}

fn resolve_permissions(headers: &HeaderMap) -> Result<BTreeSet<ToolName>, String> {
    let roles_header = headers.get(ROLE_HEADER).and_then(|value| value.to_str().ok()).unwrap_or("");
    let policy_header =
        headers.get(POLICY_CLASS_HEADER).and_then(|value| value.to_str().ok()).unwrap_or("");
    let roles = parse_roles(roles_header)?;
    let policy_class = parse_policy_class(policy_header)?;
    let allowed = allowed_tools_for_roles(&roles, policy_class);
    if allowed.is_empty() {
        return Err("unauthorized".to_string());
    }
    Ok(allowed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AscRole {
    TenantAdmin,
    NamespaceOwner,
    NamespaceAdmin,
    NamespaceWriter,
    NamespaceReader,
    SchemaManager,
    AgentSandbox,
    NamespaceDeleteAdmin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyClass {
    Scratch,
    Project,
    Prod,
}

fn parse_roles(value: &str) -> Result<BTreeSet<AscRole>, String> {
    if value.trim().is_empty() {
        return Err("missing roles".to_string());
    }
    let mut roles = BTreeSet::new();
    for token in value.split(',') {
        let role = match token.trim() {
            "tenant_admin" => AscRole::TenantAdmin,
            "namespace_owner" => AscRole::NamespaceOwner,
            "namespace_admin" => AscRole::NamespaceAdmin,
            "namespace_writer" => AscRole::NamespaceWriter,
            "namespace_reader" => AscRole::NamespaceReader,
            "schema_manager" => AscRole::SchemaManager,
            "agent_sandbox" => AscRole::AgentSandbox,
            "namespace_delete_admin" => AscRole::NamespaceDeleteAdmin,
            other if !other.is_empty() => {
                return Err(format!("unknown role: {other}"));
            }
            _ => continue,
        };
        roles.insert(role);
    }
    if roles.is_empty() {
        return Err("missing roles".to_string());
    }
    Ok(roles)
}

fn parse_policy_class(value: &str) -> Result<PolicyClass, String> {
    match value.trim() {
        "scratch" => Ok(PolicyClass::Scratch),
        "project" => Ok(PolicyClass::Project),
        "prod" => Ok(PolicyClass::Prod),
        "" => Err("missing policy_class".to_string()),
        other => Err(format!("unknown policy_class: {other}")),
    }
}

pub fn allowed_tools_for_roles(
    roles: &BTreeSet<AscRole>,
    policy_class: PolicyClass,
) -> BTreeSet<ToolName> {
    let mut allowed = BTreeSet::new();
    for role in roles {
        match role {
            AscRole::TenantAdmin | AscRole::NamespaceOwner | AscRole::NamespaceAdmin => {
                allowed.extend(all_tools());
            }
            AscRole::NamespaceWriter => {
                allowed.extend(run_tools());
                allowed.extend(read_tools());
                allowed.insert(ToolName::RunpackVerify);
            }
            AscRole::NamespaceReader => {
                allowed.extend(read_tools());
                allowed.insert(ToolName::RunpackVerify);
            }
            AscRole::SchemaManager => {
                if matches!(policy_class, PolicyClass::Scratch | PolicyClass::Project) {
                    allowed.insert(ToolName::SchemasRegister);
                    allowed.extend(read_tools());
                }
            }
            AscRole::AgentSandbox => {
                if matches!(policy_class, PolicyClass::Scratch) {
                    allowed.extend(run_tools());
                    allowed.extend(read_tools());
                }
            }
            AscRole::NamespaceDeleteAdmin => {
                allowed.extend(read_tools());
            }
        }
    }
    allowed
}

fn run_tools() -> BTreeSet<ToolName> {
    BTreeSet::from([
        ToolName::ScenarioStart,
        ToolName::ScenarioTrigger,
        ToolName::ScenarioNext,
        ToolName::ScenarioSubmit,
        ToolName::Precheck,
    ])
}

fn read_tools() -> BTreeSet<ToolName> {
    BTreeSet::from([
        ToolName::ScenarioStatus,
        ToolName::ScenariosList,
        ToolName::SchemasList,
        ToolName::SchemasGet,
        ToolName::ProvidersList,
        ToolName::EvidenceQuery,
    ])
}

fn all_tools() -> BTreeSet<ToolName> {
    BTreeSet::from([
        ToolName::ScenarioDefine,
        ToolName::ScenarioStart,
        ToolName::ScenarioStatus,
        ToolName::ScenarioNext,
        ToolName::ScenarioSubmit,
        ToolName::ScenarioTrigger,
        ToolName::EvidenceQuery,
        ToolName::RunpackExport,
        ToolName::RunpackVerify,
        ToolName::ProvidersList,
        ToolName::SchemasList,
        ToolName::SchemasRegister,
        ToolName::SchemasGet,
        ToolName::ScenariosList,
        ToolName::Precheck,
    ])
}

pub fn roles_to_header(roles: &[AscRole]) -> String {
    roles
        .iter()
        .map(|role| match role {
            AscRole::TenantAdmin => "tenant_admin",
            AscRole::NamespaceOwner => "namespace_owner",
            AscRole::NamespaceAdmin => "namespace_admin",
            AscRole::NamespaceWriter => "namespace_writer",
            AscRole::NamespaceReader => "namespace_reader",
            AscRole::SchemaManager => "schema_manager",
            AscRole::AgentSandbox => "agent_sandbox",
            AscRole::NamespaceDeleteAdmin => "namespace_delete_admin",
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub const fn policy_class_to_header(policy_class: PolicyClass) -> &'static str {
    match policy_class {
        PolicyClass::Scratch => "scratch",
        PolicyClass::Project => "project",
        PolicyClass::Prod => "prod",
    }
}
