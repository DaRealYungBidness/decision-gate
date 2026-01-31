// system-tests/tests/suites/providers.rs
// ============================================================================
// Module: Provider Tests
// Description: Built-in and federated provider coverage.
// Purpose: Validate provider conditions and MCP federation.
// Dependencies: system-tests helpers
// ============================================================================

//! Provider integration tests for Decision Gate system-tests.

use std::fs;
use std::io;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::post;
use decision_gate_contract::types::CheckContract;
use decision_gate_contract::types::CheckExample;
use decision_gate_contract::types::DeterminismClass;
use decision_gate_contract::types::ProviderContract;
use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::config::AnchorProviderConfig;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::TrustPolicy;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::EvidenceQueryResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::config_with_provider;
use helpers::harness::config_with_provider_timeouts;
use helpers::harness::spawn_mcp_server;
use helpers::provider_stub::ProviderFixture;
use helpers::provider_stub::spawn_provider_fixture_stub;
use helpers::provider_stub::spawn_provider_stub;
use helpers::provider_stub::spawn_provider_stub_with_delay;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use tempfile::tempdir;
use tokio::sync::oneshot;
use toml::Value as TomlValue;
use toml::value::Table;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

fn i64_from_usize(value: usize, label: &str) -> Result<i64, std::io::Error> {
    i64::try_from(value).map_err(|_| std::io::Error::other(format!("{label} out of range")))
}

fn i64_from_u64(value: u64, label: &str) -> Result<i64, std::io::Error> {
    i64::try_from(value).map_err(|_| std::io::Error::other(format!("{label} out of range")))
}

fn json_provider_config(root: &Path, max_bytes: usize) -> Result<TomlValue, std::io::Error> {
    let mut table = Table::new();
    table.insert("root".to_string(), TomlValue::String(root.to_string_lossy().to_string()));
    table.insert(
        "max_bytes".to_string(),
        TomlValue::Integer(i64_from_usize(max_bytes, "max_bytes")?),
    );
    table.insert("allow_yaml".to_string(), TomlValue::Boolean(false));
    Ok(TomlValue::Table(table))
}

fn http_provider_config(
    allow_http: bool,
    timeout_ms: u64,
    max_response_bytes: usize,
    allowed_hosts: Option<Vec<String>>,
) -> Result<TomlValue, std::io::Error> {
    let mut table = Table::new();
    table.insert("allow_http".to_string(), TomlValue::Boolean(allow_http));
    table.insert(
        "timeout_ms".to_string(),
        TomlValue::Integer(i64_from_u64(timeout_ms, "timeout_ms")?),
    );
    table.insert(
        "max_response_bytes".to_string(),
        TomlValue::Integer(i64_from_usize(max_response_bytes, "max_response_bytes")?),
    );
    if let Some(hosts) = allowed_hosts {
        let list = hosts.into_iter().map(TomlValue::String).collect();
        table.insert("allowed_hosts".to_string(), TomlValue::Array(list));
    }
    table.insert("user_agent".to_string(), TomlValue::String("dg-test".to_string()));
    table.insert("hash_algorithm".to_string(), TomlValue::String("sha256".to_string()));
    Ok(TomlValue::Table(table))
}

fn env_provider_config(
    allowlist: Option<Vec<String>>,
    denylist: Vec<String>,
    overrides: Vec<(String, String)>,
    max_value_bytes: usize,
    max_key_bytes: usize,
) -> Result<TomlValue, std::io::Error> {
    let mut table = Table::new();
    if let Some(allowlist) = allowlist {
        let list = allowlist.into_iter().map(TomlValue::String).collect();
        table.insert("allowlist".to_string(), TomlValue::Array(list));
    }
    let denylist = denylist.into_iter().map(TomlValue::String).collect();
    table.insert("denylist".to_string(), TomlValue::Array(denylist));
    table.insert(
        "max_value_bytes".to_string(),
        TomlValue::Integer(i64_from_usize(max_value_bytes, "max_value_bytes")?),
    );
    table.insert(
        "max_key_bytes".to_string(),
        TomlValue::Integer(i64_from_usize(max_key_bytes, "max_key_bytes")?),
    );
    let mut overrides_table = Table::new();
    for (key, value) in overrides {
        overrides_table.insert(key, TomlValue::String(value));
    }
    table.insert("overrides".to_string(), TomlValue::Table(overrides_table));
    Ok(TomlValue::Table(table))
}

fn time_provider_config(allow_logical: bool) -> TomlValue {
    let mut table = Table::new();
    table.insert("allow_logical".to_string(), TomlValue::Boolean(allow_logical));
    TomlValue::Table(table)
}

#[allow(clippy::missing_const_for_fn, reason = "Mutation helper is runtime-only.")]
fn enable_raw_evidence(config: &mut DecisionGateConfig) {
    config.evidence.allow_raw_values = true;
    config.evidence.require_provider_opt_in = false;
}

fn set_provider_config(
    config: &mut DecisionGateConfig,
    provider_name: &str,
    value: TomlValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.name == provider_name)
        .ok_or_else(|| format!("missing provider {provider_name}"))?;
    provider.config = Some(value);
    Ok(())
}

#[derive(Debug)]
struct HttpTestServerHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl HttpTestServerHandle {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl Drop for HttpTestServerHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn spawn_http_test_server() -> Result<HttpTestServerHandle, Box<dyn std::error::Error>> {
    async fn ok_handler() -> &'static str {
        "hello"
    }

    async fn large_handler() -> String {
        "x".repeat(2048)
    }

    async fn slow_handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(200)).await;
        "slow"
    }

    async fn redirect_handler() -> impl IntoResponse {
        (StatusCode::FOUND, [(LOCATION, HeaderValue::from_static("/redirect"))])
    }

    let app = Router::new()
        .route("/ok", get(ok_handler))
        .route("/large", get(large_handler))
        .route("/slow", get(slow_handler))
        .route("/redirect", get(redirect_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{addr}");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
    });
    Ok(HttpTestServerHandle {
        base_url,
        shutdown: Some(shutdown_tx),
    })
}

#[cfg(unix)]
fn create_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(src, dst)
}

#[derive(Debug, Deserialize)]
#[allow(
    dead_code,
    reason = "Struct mirrors JSON-RPC requests for validation; fields unused directly."
)]
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
#[allow(dead_code, reason = "Struct mirrors tool-call payloads for schema coverage in tests.")]
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
    Json { json: Box<decision_gate_core::EvidenceResult> },
    Text { text: String },
}

#[derive(Debug)]
struct McpProviderHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl McpProviderHandle {
    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for McpProviderHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn spawn_mcp_provider(app: Router) -> Result<McpProviderHandle, Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{addr}/rpc");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
    });
    Ok(McpProviderHandle {
        base_url,
        shutdown: Some(shutdown_tx),
    })
}

const fn jsonrpc_success(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn jsonrpc_error(id: Value, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code: -32000,
            message: message.to_string(),
        }),
    }
}

fn tool_result_for_value(value: Value) -> Value {
    let result = decision_gate_core::EvidenceResult {
        value: Some(decision_gate_core::EvidenceValue::Json(value)),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: Some("application/json".to_string()),
    };
    serde_json::to_value(ToolCallResult {
        content: vec![ToolContent::Json {
            json: Box::new(result),
        }],
    })
    .unwrap_or(Value::Null)
}

fn jsonrpc_id_from_bytes(bytes: &Bytes) -> Value {
    serde_json::from_slice::<JsonRpcRequest>(bytes).map(|request| request.id).unwrap_or(Value::Null)
}

fn parse_evidence_request(bytes: &Bytes) -> Result<(Value, EvidenceQueryRequest), String> {
    let request: JsonRpcRequest =
        serde_json::from_slice(bytes).map_err(|_| "invalid jsonrpc request".to_string())?;
    let params = request.params.ok_or_else(|| "missing jsonrpc params".to_string())?;
    let call: ToolCallParams =
        serde_json::from_value(params).map_err(|_| "invalid tool params".to_string())?;
    let parsed: EvidenceQueryRequest =
        serde_json::from_value(call.arguments).map_err(|_| "invalid evidence_query".to_string())?;
    Ok((request.id, parsed))
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_time_after() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("provider_time_after")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("provider-time", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: fixture.run_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "provider-test".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let outcome = &trigger_result.decision.outcome;
    if !matches!(outcome, DecisionOutcome::Complete { .. }) {
        return Err(format!("unexpected decision outcome: {outcome:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["time provider check evaluated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn json_provider_missing_jsonpath_returns_error_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_provider_missing_jsonpath_returns_error_metadata")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let dir = tempdir()?;
    let report_path = dir.path().join("report.json");
    fs::write(&report_path, r#"{"summary":{"passed":1}}"#)?;

    let fixture = ScenarioFixture::time_after("json-evidence", "run-1", 0);
    let request = decision_gate_mcp::tools::EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({
                "file": report_path.to_string_lossy(),
                "jsonpath": "$.summary.failed"
            })),
        },
        context: fixture.evidence_context("json-trigger", Timestamp::Logical(1)),
    };

    let input = serde_json::to_value(&request)?;
    let response: decision_gate_mcp::tools::EvidenceQueryResponse =
        client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or_else(|| io::Error::other("missing error metadata"))?;
    if error.code != "jsonpath_not_found" {
        return Err(format!("expected jsonpath_not_found, got {}", error.code).into());
    }
    if response.result.value.is_some() {
        return Err("expected missing jsonpath to return null evidence value".into());
    }
    if response.result.content_type.is_some() {
        return Err("expected missing jsonpath to return no content_type".into());
    }
    if response.result.evidence_hash.is_some() {
        return Err("expected missing jsonpath to return no evidence_hash".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json provider returns error metadata for missing jsonpath".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn json_provider_rejects_path_outside_root() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_provider_rejects_path_outside_root")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let root_dir = tempdir()?;
    let outside_dir = tempdir()?;
    let outside_path = outside_dir.path().join("outside.json");
    fs::write(&outside_path, r#"{"ok":true}"#)?;

    let json_config = json_provider_config(root_dir.path(), 1024)?;
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.name == "json")
        .ok_or("missing json provider")?;
    provider.config = Some(json_config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("json-path-outside", "run-1", 0);
    let request = decision_gate_mcp::tools::EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({
                "file": outside_path.to_string_lossy(),
            })),
        },
        context: fixture.evidence_context("json-trigger", Timestamp::Logical(1)),
    };

    let input = serde_json::to_value(&request)?;
    let response: decision_gate_mcp::tools::EvidenceQueryResponse =
        client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if error.code != "path_outside_root" {
        return Err(format!("expected path_outside_root, got {}", error.code).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json provider rejects path outside configured root".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn json_provider_enforces_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_provider_enforces_size_limit")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let root_dir = tempdir()?;
    let report_path = root_dir.path().join("report.json");
    fs::write(&report_path, r#"{"payload":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#)?;

    let json_config = json_provider_config(root_dir.path(), 8)?;
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.name == "json")
        .ok_or("missing json provider")?;
    provider.config = Some(json_config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("json-size-limit", "run-1", 0);
    let request = decision_gate_mcp::tools::EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({
                "file": report_path.to_string_lossy(),
            })),
        },
        context: fixture.evidence_context("json-trigger", Timestamp::Logical(1)),
    };

    let input = serde_json::to_value(&request)?;
    let response: decision_gate_mcp::tools::EvidenceQueryResponse =
        client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if error.code != "size_limit_exceeded" {
        return Err(format!("expected size_limit_exceeded, got {}", error.code).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json provider enforces size limit".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn json_provider_rejects_symlink_escape() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_provider_rejects_symlink_escape")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let root_dir = tempdir()?;
    let outside_dir = tempdir()?;
    let outside_path = outside_dir.path().join("outside.json");
    fs::write(&outside_path, r#"{"ok":true}"#)?;
    let link_path = root_dir.path().join("link.json");
    if let Err(err) = create_symlink(&outside_path, &link_path) {
        if matches!(err.kind(), io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported) {
            reporter.finish(
                "skip",
                vec![format!("symlink creation unavailable: {err}")],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
        return Err(err.into());
    }

    let json_config = json_provider_config(root_dir.path(), 1024)?;
    set_provider_config(&mut config, "json", json_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("json-symlink", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({
                "file": link_path.to_string_lossy(),
            })),
        },
        context: fixture.evidence_context("json-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if error.code != "path_outside_root" {
        return Err(format!("expected path_outside_root, got {}", error.code).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json provider blocks symlink escapes".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn json_provider_invalid_jsonpath_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_provider_invalid_jsonpath_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let root_dir = tempdir()?;
    let report_path = root_dir.path().join("report.json");
    fs::write(&report_path, r#"{"summary":{"ok":true}}"#)?;

    let json_config = json_provider_config(root_dir.path(), 1024)?;
    set_provider_config(&mut config, "json", json_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("json-bad-jsonpath", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({
                "file": report_path.to_string_lossy(),
                "jsonpath": "$..["
            })),
        },
        context: fixture.evidence_context("json-trigger", Timestamp::Logical(1)),
    };

    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if error.code != "invalid_jsonpath" {
        return Err(format!("expected invalid_jsonpath, got {}", error.code).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json provider rejects invalid jsonpath".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Full JSON provider flow kept in one block for review.")]
async fn json_provider_contains_array_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_provider_contains_array_succeeds")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let dir = tempdir()?;
    let report_path = dir.path().join("report.json");
    fs::write(&report_path, r#"{"summary":{"status":"ok","tags":["alpha","beta","gamma"]}}"#)?;

    let scenario_id = ScenarioId::new("json-provider-contains");
    let stage_id = StageId::new("stage-1");
    let condition_id = ConditionId::new("summary-tags");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id: namespace_id_one(),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-contains"),
                requirement: ret_logic::Requirement::condition(condition_id.clone()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id,
            query: EvidenceQuery {
                provider_id: ProviderId::new("json"),
                check_id: "path".to_string(),
                params: Some(json!({
                    "file": report_path.to_string_lossy(),
                    "jsonpath": "$.summary.tags"
                })),
            },
            comparator: Comparator::Contains,
            expected: Some(json!(["alpha", "gamma"])),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    };

    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: RunConfig {
            tenant_id: tenant_id_one(),
            namespace_id: namespace_id_one(),
            run_id: RunId::new("run-1"),
            scenario_id: define_output.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        },
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id,
        trigger: decision_gate_core::TriggerEvent {
            run_id: RunId::new("run-1"),
            tenant_id: tenant_id_one(),
            namespace_id: namespace_id_one(),
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "json-provider-test".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let outcome = &trigger_result.decision.outcome;
    if !matches!(outcome, DecisionOutcome::Complete { .. }) {
        return Err(format!("unexpected decision outcome: {outcome:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json provider contains comparator evaluated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_blocks_http_scheme_by_default() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_blocks_http_scheme_by_default")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("http-scheme", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({ "url": "http://127.0.0.1:1/ok" })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("unsupported url scheme") {
        return Err(format!("expected unsupported url scheme, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider blocks cleartext http by default".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_enforces_allowlist() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_enforces_allowlist")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let http_config = http_provider_config(true, 5_000, 1024, Some(vec!["127.0.0.1".to_string()]))?;
    set_provider_config(&mut config, "http", http_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("http-allowlist", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({ "url": "http://localhost:1/ok" })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("url host not allowed") {
        return Err(format!("expected url host not allowed, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider enforces host allowlist".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_redirect_not_followed() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_redirect_not_followed")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let http_config = http_provider_config(true, 5_000, 1024, None)?;
    set_provider_config(&mut config, "http", http_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let http_server = spawn_http_test_server().await?;
    let fixture = ScenarioFixture::time_after("http-redirect", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({ "url": http_server.url("/redirect") })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    if response.result.error.is_some() {
        return Err("unexpected error for redirect response".into());
    }
    let Some(decision_gate_core::EvidenceValue::Json(value)) = response.result.value else {
        return Err("missing status value for redirect".into());
    };
    let status = value.as_u64().ok_or("status value not numeric")?;
    if status != 302 {
        return Err(format!("expected status 302, got {status}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider returns redirect status without following".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_body_hash_matches() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_body_hash_matches")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let http_config = http_provider_config(true, 5_000, 1024, None)?;
    set_provider_config(&mut config, "http", http_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let http_server = spawn_http_test_server().await?;
    let fixture = ScenarioFixture::time_after("http-body-hash", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "body_hash".to_string(),
            params: Some(json!({ "url": http_server.url("/ok") })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    if response.result.error.is_some() {
        return Err("unexpected error for body_hash".into());
    }
    let expected = hash_bytes(decision_gate_core::HashAlgorithm::Sha256, b"hello");
    let Some(decision_gate_core::EvidenceValue::Json(value)) = response.result.value else {
        return Err("missing hash value".into());
    };
    let digest: decision_gate_core::HashDigest = serde_json::from_value(value)?;
    if digest != expected {
        return Err(
            format!("hash mismatch: expected {}, got {}", expected.value, digest.value).into()
        );
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider body_hash returns canonical hash".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_response_size_limit_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_response_size_limit_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let http_config = http_provider_config(true, 5_000, 8, None)?;
    set_provider_config(&mut config, "http", http_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let http_server = spawn_http_test_server().await?;
    let fixture = ScenarioFixture::time_after("http-size", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "body_hash".to_string(),
            params: Some(json!({ "url": http_server.url("/large") })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("http response exceeds size limit") {
        return Err(format!("expected size limit error, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider enforces response size limit".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_timeout_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_timeout_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let http_config = http_provider_config(true, 10, 1024, None)?;
    set_provider_config(&mut config, "http", http_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let http_server = spawn_http_test_server().await?;
    let fixture = ScenarioFixture::time_after("http-timeout", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({ "url": http_server.url("/slow") })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("timed out") && !error.message.contains("http request failed") {
        return Err(format!("expected timeout or request failed, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider request timeouts are enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_provider_tls_failure_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_provider_tls_failure_fails_closed")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let http_config = http_provider_config(true, 5_000, 1024, None)?;
    set_provider_config(&mut config, "http", http_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let http_server = spawn_http_test_server().await?;
    let tls_url = http_server.url("/ok").replace("http://", "https://");
    let fixture = ScenarioFixture::time_after("http-tls", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({ "url": tls_url })),
        },
        context: fixture.evidence_context("http-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("http request failed") {
        return Err(format!("expected request failed, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["http provider fails closed on TLS errors".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn env_provider_missing_key_returns_empty() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("env_provider_missing_key_returns_empty")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let env_config = env_provider_config(
        None,
        Vec::new(),
        vec![("KNOWN".to_string(), "ok".to_string())],
        32,
        32,
    )?;
    set_provider_config(&mut config, "env", env_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("env-missing", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            check_id: "get".to_string(),
            params: Some(json!({ "key": "MISSING" })),
        },
        context: fixture.evidence_context("env-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    if response.result.error.is_some() {
        return Err("unexpected error for missing env key".into());
    }
    if response.result.value.is_some() || response.result.content_type.is_some() {
        return Err("expected missing env key to return empty result".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["env provider returns empty result for missing key".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn env_provider_denylist_blocks() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("env_provider_denylist_blocks")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let env_config = env_provider_config(
        None,
        vec!["BLOCKED".to_string()],
        vec![("BLOCKED".to_string(), "nope".to_string())],
        32,
        32,
    )?;
    set_provider_config(&mut config, "env", env_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("env-deny", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            check_id: "get".to_string(),
            params: Some(json!({ "key": "BLOCKED" })),
        },
        context: fixture.evidence_context("env-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("env key blocked by policy") {
        return Err(format!("expected env key blocked, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["env provider denylist enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn env_provider_allowlist_blocks_unlisted() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("env_provider_allowlist_blocks_unlisted")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let env_config = env_provider_config(
        Some(vec!["ALLOWED".to_string()]),
        Vec::new(),
        vec![("ALLOWED".to_string(), "ok".to_string())],
        32,
        32,
    )?;
    set_provider_config(&mut config, "env", env_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("env-allow", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            check_id: "get".to_string(),
            params: Some(json!({ "key": "OTHER" })),
        },
        context: fixture.evidence_context("env-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("env key blocked by policy") {
        return Err(format!("expected env key blocked, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["env provider allowlist blocks unlisted keys".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn env_provider_value_size_limit_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("env_provider_value_size_limit_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let env_config = env_provider_config(
        None,
        Vec::new(),
        vec![("BIG".to_string(), "0123456789".to_string())],
        4,
        32,
    )?;
    set_provider_config(&mut config, "env", env_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("env-size", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            check_id: "get".to_string(),
            params: Some(json!({ "key": "BIG" })),
        },
        context: fixture.evidence_context("env-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("env value exceeds limit") {
        return Err(format!("expected env value limit error, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["env provider value size limit enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn env_provider_key_size_limit_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("env_provider_key_size_limit_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let env_config = env_provider_config(None, Vec::new(), Vec::new(), 32, 3)?;
    set_provider_config(&mut config, "env", env_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("env-key", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            check_id: "get".to_string(),
            params: Some(json!({ "key": "TOO_LONG" })),
        },
        context: fixture.evidence_context("env-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("env key exceeds limit") {
        return Err(format!("expected env key limit error, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["env provider key size limit enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn time_provider_rejects_logical_when_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("time_provider_rejects_logical_when_disabled")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let time_config = time_provider_config(false);
    set_provider_config(&mut config, "time", time_config)?;

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("time-logical", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            check_id: "now".to_string(),
            params: None,
        },
        context: fixture.evidence_context("time-trigger", Timestamp::Logical(5)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("logical timestamps are not permitted") {
        return Err(format!("expected logical timestamps error, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["time provider rejects logical timestamps when disabled".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn time_provider_rfc3339_parsing() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("time_provider_rfc3339_parsing")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("time-rfc3339", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            check_id: "after".to_string(),
            params: Some(json!({ "timestamp": "2024-01-01T00:00:00+00:00" })),
        },
        context: fixture.evidence_context("time-trigger", Timestamp::UnixMillis(1_704_067_201_000)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    if response.result.error.is_some() {
        return Err("unexpected error for rfc3339 parsing".into());
    }
    let Some(decision_gate_core::EvidenceValue::Json(value)) = response.result.value else {
        return Err("missing time check value".into());
    };
    let result = value.as_bool().ok_or("expected boolean result")?;
    if !result {
        return Err("expected after check to be true".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["time provider parses rfc3339 timestamps".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn time_provider_invalid_rfc3339_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("time_provider_invalid_rfc3339_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("time-bad-rfc3339", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            check_id: "after".to_string(),
            params: Some(json!({ "timestamp": "not-a-time" })),
        },
        context: fixture.evidence_context("time-trigger", Timestamp::UnixMillis(1_704_067_200_000)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("invalid rfc3339 timestamp") {
        return Err(format!("expected invalid rfc3339 error, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["time provider rejects invalid rfc3339 strings".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_malformed_jsonrpc_response() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_malformed_jsonrpc_response")?;
    let bind = allocate_bind_addr()?.to_string();
    let app = Router::new()
        .route("/rpc", post(|_bytes: Bytes| async move { (StatusCode::OK, "not-json") }));
    let provider = spawn_mcp_provider(app).await?;
    let capabilities_path = write_echo_contract(&reporter, "mcp-malformed")?;
    let mut config =
        config_with_provider(&bind, "mcp-malformed", provider.base_url(), &capabilities_path);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("mcp-malformed", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("mcp-malformed"),
            check_id: "echo".to_string(),
            params: Some(json!({ "value": true })),
        },
        context: fixture.evidence_context("mcp-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("invalid json-rpc response") {
        return Err(format!("expected invalid json-rpc response, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["malformed MCP provider response fails closed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_text_content_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_text_content_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let app = Router::new().route(
        "/rpc",
        post(|bytes: Bytes| async move {
            let id = jsonrpc_id_from_bytes(&bytes);
            let result = serde_json::to_value(ToolCallResult {
                content: vec![ToolContent::Text {
                    text: "nope".to_string(),
                }],
            })
            .unwrap_or(Value::Null);
            axum::Json(jsonrpc_success(id, result))
        }),
    );
    let provider = spawn_mcp_provider(app).await?;
    let capabilities_path = write_echo_contract(&reporter, "mcp-text")?;
    let mut config =
        config_with_provider(&bind, "mcp-text", provider.base_url(), &capabilities_path);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("mcp-text", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("mcp-text"),
            check_id: "echo".to_string(),
            params: Some(json!({ "value": true })),
        },
        context: fixture.evidence_context("mcp-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("unexpected text response") {
        return Err(format!("expected unexpected text response, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["text content from MCP provider rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_empty_result_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_empty_result_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let app = Router::new().route(
        "/rpc",
        post(|bytes: Bytes| async move {
            let id = jsonrpc_id_from_bytes(&bytes);
            let result = serde_json::to_value(ToolCallResult {
                content: Vec::new(),
            })
            .unwrap_or(Value::Null);
            axum::Json(jsonrpc_success(id, result))
        }),
    );
    let provider = spawn_mcp_provider(app).await?;
    let capabilities_path = write_echo_contract(&reporter, "mcp-empty")?;
    let mut config =
        config_with_provider(&bind, "mcp-empty", provider.base_url(), &capabilities_path);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("mcp-empty", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("mcp-empty"),
            check_id: "echo".to_string(),
            params: Some(json!({ "value": true })),
        },
        context: fixture.evidence_context("mcp-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("empty tool result") {
        return Err(format!("expected empty tool result, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["empty MCP provider result rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_flaky_response() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_flaky_response")?;
    let bind = allocate_bind_addr()?.to_string();
    let counter = Arc::new(AtomicUsize::new(0));
    let state = counter.clone();
    let app = Router::new().route(
        "/rpc",
        post(move |bytes: Bytes| {
            let state = state.clone();
            async move {
                let id = jsonrpc_id_from_bytes(&bytes);
                let call = state.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    axum::Json(jsonrpc_error(id, "flaky failure"))
                } else {
                    axum::Json(jsonrpc_success(id, tool_result_for_value(json!(true))))
                }
            }
        }),
    );
    let provider = spawn_mcp_provider(app).await?;
    let capabilities_path = write_echo_contract(&reporter, "mcp-flaky")?;
    let mut config =
        config_with_provider(&bind, "mcp-flaky", provider.base_url(), &capabilities_path);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("mcp-flaky", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("mcp-flaky"),
            check_id: "echo".to_string(),
            params: Some(json!({ "value": true })),
        },
        context: fixture.evidence_context("mcp-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let first: EvidenceQueryResponse =
        client.call_tool_typed("evidence_query", input.clone()).await?;
    let first_error = first.result.error.ok_or("expected flaky error")?;
    if !first_error.message.contains("flaky failure") {
        return Err(format!("expected flaky failure, got {}", first_error.message).into());
    }

    let second: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    if second.result.error.is_some() {
        return Err("expected flaky provider to recover".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["flaky MCP provider fails closed then recovers".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_wrong_namespace_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_wrong_namespace_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let app = Router::new().route(
        "/rpc",
        post(|bytes: Bytes| async move {
            let (id, parsed) = match parse_evidence_request(&bytes) {
                Ok(parsed) => parsed,
                Err(message) => return axum::Json(jsonrpc_error(Value::Null, &message)),
            };
            if parsed.context.namespace_id.get() != 2 {
                return axum::Json(jsonrpc_error(id, "namespace mismatch"));
            }
            axum::Json(jsonrpc_success(id, tool_result_for_value(json!(true))))
        }),
    );
    let provider = spawn_mcp_provider(app).await?;
    let capabilities_path = write_echo_contract(&reporter, "mcp-namespace")?;
    let mut config =
        config_with_provider(&bind, "mcp-namespace", provider.base_url(), &capabilities_path);
    enable_raw_evidence(&mut config);

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("mcp-namespace", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("mcp-namespace"),
            check_id: "echo".to_string(),
            params: Some(json!({ "value": true })),
        },
        context: fixture.evidence_context("mcp-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("namespace mismatch") {
        return Err(format!("expected namespace mismatch, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["MCP provider rejects wrong namespace".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_missing_signature_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_missing_signature_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let app = Router::new().route(
        "/rpc",
        post(|bytes: Bytes| async move {
            let id = jsonrpc_id_from_bytes(&bytes);
            axum::Json(jsonrpc_success(id, tool_result_for_value(json!(true))))
        }),
    );
    let provider = spawn_mcp_provider(app).await?;
    let capabilities_path = write_echo_contract(&reporter, "mcp-signed")?;
    let mut config =
        config_with_provider(&bind, "mcp-signed", provider.base_url(), &capabilities_path);
    enable_raw_evidence(&mut config);

    let key_dir = tempdir()?;
    let key_path = key_dir.path().join("ed25519.pub");
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();
    fs::write(&key_path, verifying_key.to_bytes())?;

    let provider_config = config
        .providers
        .iter_mut()
        .find(|provider| provider.name == "mcp-signed")
        .ok_or("missing provider config")?;
    provider_config.trust = Some(TrustPolicy::RequireSignature {
        keys: vec![key_path.to_string_lossy().to_string()],
    });

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("mcp-signed", "run-1", 0);
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("mcp-signed"),
            check_id: "echo".to_string(),
            params: Some(json!({ "value": true })),
        },
        context: fixture.evidence_context("mcp-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: EvidenceQueryResponse = client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or("missing error metadata")?;
    if !error.message.contains("missing evidence signature") {
        return Err(format!("expected missing evidence signature, got {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["signature-required MCP provider rejects unsigned evidence".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_provider_contract_mismatch_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("mcp_provider_contract_mismatch_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let contract = ProviderContract {
        provider_id: "wrong-id".to_string(),
        name: "Broken Provider".to_string(),
        description: "Contract mismatch for testing.".to_string(),
        transport: "mcp".to_string(),
        config_schema: json!({ "type": "object" }),
        checks: Vec::new(),
        notes: Vec::new(),
    };
    let path = reporter.artifacts().write_json("bad_contract.json", &contract)?;
    let config = config_with_provider(&bind, "mcp-contract", "http://127.0.0.1:1/rpc", &path);
    let Err(err) = spawn_mcp_server(config).await else {
        return Err("expected server to reject mismatched contract".into());
    };
    reporter.finish(
        "pass",
        vec![format!("contract mismatch rejected: {err}")],
        vec!["summary.json".to_string(), "summary.md".to_string(), "bad_contract.json".to_string()],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Interop echo flow stays linear for auditability.")]
async fn federated_provider_echo() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("federated_provider_echo")?;
    let provider = spawn_provider_stub(json!(true)).await?;

    let bind = allocate_bind_addr()?.to_string();
    let capabilities_path = write_echo_contract(&reporter, "echo")?;
    let config = config_with_provider(&bind, "echo", provider.base_url(), &capabilities_path);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let scenario_id = ScenarioId::new("federated-provider");
    let stage_id = StageId::new("stage-1");
    let condition_id = ConditionId::new("echo");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id: namespace_id_one(),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-echo"),
                requirement: ret_logic::Requirement::condition(condition_id.clone()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id,
            query: EvidenceQuery {
                provider_id: ProviderId::new("echo"),
                check_id: "echo".to_string(),
                params: Some(json!({"value": true})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    };

    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: decision_gate_core::RunConfig {
            tenant_id: tenant_id_one(),
            namespace_id: namespace_id_one(),
            run_id: decision_gate_core::RunId::new("run-1"),
            scenario_id: define_output.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        },
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id,
        trigger: decision_gate_core::TriggerEvent {
            run_id: decision_gate_core::RunId::new("run-1"),
            tenant_id: tenant_id_one(),
            namespace_id: namespace_id_one(),
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "provider-test".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let outcome = &trigger_result.decision.outcome;
    if !matches!(outcome, DecisionOutcome::Complete { .. }) {
        return Err(format!("unexpected decision outcome: {outcome:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["federated provider executed evidence query".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn federated_provider_timeout_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("federated_provider_timeout_enforced")?;
    let provider =
        spawn_provider_stub_with_delay(json!(true), Duration::from_millis(1_500)).await?;

    let bind = allocate_bind_addr()?.to_string();
    let capabilities_path = write_echo_contract(&reporter, "echo-timeout")?;
    let timeouts = ProviderTimeoutConfig {
        connect_timeout_ms: 500,
        request_timeout_ms: 500,
    };
    let config = config_with_provider_timeouts(
        &bind,
        "echo-timeout",
        provider.base_url(),
        &capabilities_path,
        timeouts,
    );
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("timeout-scenario", "run-1", 0);
    let request = decision_gate_mcp::tools::EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("echo-timeout"),
            check_id: "echo".to_string(),
            params: Some(json!({"value": true})),
        },
        context: fixture.evidence_context("timeout-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let response: decision_gate_mcp::tools::EvidenceQueryResponse =
        client.call_tool_typed("evidence_query", input).await?;
    let error = response.result.error.ok_or_else(|| io::Error::other("missing error metadata"))?;
    if !error.message.contains("timed out") {
        return Err(format!("expected timeout error, got: {}", error.message).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["federated provider timeouts are enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "echo_provider_contract.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Interop fixture test keeps the full flow in one place.")]
async fn assetcore_interop_fixtures() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("assetcore_interop_fixtures")?;
    let fixture_root_dir = fixture_root("assetcore/interop");
    let spec: ScenarioSpec =
        load_fixture(&fixture_root_dir.join("scenarios/assetcore-interop-full.json"))?;
    let run_config: RunConfig =
        load_fixture(&fixture_root_dir.join("run-configs/assetcore-interop-full.json"))?;
    let trigger: decision_gate_core::TriggerEvent =
        load_fixture(&fixture_root_dir.join("triggers/assetcore-interop-full.json"))?;
    let fixture_map: FixtureMap = load_fixture(&fixture_root_dir.join("fixture_map.json"))?;

    let namespace_id = fixture_map.assetcore_namespace_id.unwrap_or(0);
    let commit_id = fixture_map.fixture_version.clone().unwrap_or_else(|| "fixture".to_string());
    let fixtures = fixture_map
        .fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            let anchor_value = json!({
                "assetcore.namespace_id": namespace_id,
                "assetcore.commit_id": commit_id,
                "assetcore.world_seq": index as u64 + 1
            });
            ProviderFixture {
                check_id: fixture.check_id.clone(),
                params: fixture.params.clone(),
                result: fixture.expected.clone(),
                anchor: Some(EvidenceAnchor {
                    anchor_type: "assetcore.anchor_set".to_string(),
                    anchor_value: serde_json::to_string(&anchor_value)
                        .unwrap_or_else(|_| "{}".to_string()),
                }),
            }
        })
        .collect();

    let provider = spawn_provider_fixture_stub(fixtures).await?;
    let bind = allocate_bind_addr()?.to_string();
    let provider_contract = fixture_root("assetcore/providers").join("assetcore_read.json");
    let mut config =
        config_with_provider(&bind, "assetcore_read", provider.base_url(), &provider_contract);
    config.anchors.providers.push(AnchorProviderConfig {
        provider_id: "assetcore_read".to_string(),
        anchor_type: "assetcore.anchor_set".to_string(),
        required_fields: vec![
            "assetcore.namespace_id".to_string(),
            "assetcore.commit_id".to_string(),
            "assetcore.world_seq".to_string(),
        ],
    });
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let started_at = trigger.time;
    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at,
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: trigger.clone(),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let outcome = &trigger_result.decision.outcome;
    if !matches!(outcome, DecisionOutcome::Complete { .. }) {
        return Err(format!("unexpected decision outcome: {outcome:?}").into());
    }

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: decision_gate_core::runtime::StatusRequest {
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            run_id: run_config.run_id.clone(),
            requested_at: trigger.time,
            correlation_id: trigger.correlation_id.clone(),
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    if status.status != RunStatus::Completed {
        return Err(format!("unexpected run status: {:?}", status.status).into());
    }

    reporter.artifacts().write_json("interop_spec.json", &spec)?;
    reporter.artifacts().write_json("interop_run_config.json", &run_config)?;
    reporter.artifacts().write_json("interop_trigger.json", &trigger)?;
    reporter.artifacts().write_json("interop_fixture_map.json", &fixture_map)?;
    reporter.artifacts().write_json("interop_status.json", &status)?;
    reporter.artifacts().write_json("interop_decision.json", &trigger_result.decision)?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["assetcore interop fixtures executed via federated provider".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "interop_spec.json".to_string(),
            "interop_run_config.json".to_string(),
            "interop_trigger.json".to_string(),
            "interop_fixture_map.json".to_string(),
            "interop_status.json".to_string(),
            "interop_decision.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

fn write_echo_contract(
    reporter: &TestReporter,
    provider_id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let contract = ProviderContract {
        provider_id: provider_id.to_string(),
        name: "Echo Provider".to_string(),
        description: "Echo check used by system-tests for MCP federation.".to_string(),
        transport: "mcp".to_string(),
        config_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        }),
        checks: vec![CheckContract {
            check_id: "echo".to_string(),
            description: "Return the configured echo value.".to_string(),
            determinism: DeterminismClass::External,
            params_required: true,
            params_schema: json!({
                "type": "object",
                "required": ["value"],
                "properties": {
                    "value": { "type": "boolean" }
                },
                "additionalProperties": false
            }),
            result_schema: json!({ "type": "boolean" }),
            allowed_comparators: vec![
                Comparator::Equals,
                Comparator::NotEquals,
                Comparator::Exists,
                Comparator::NotExists,
            ],
            anchor_types: vec![String::from("stub")],
            content_types: vec![String::from("application/json")],
            examples: vec![CheckExample {
                description: "Return true for echo=true.".to_string(),
                params: json!({ "value": true }),
                result: json!(true),
            }],
        }],
        notes: vec![String::from("Used only for system-tests MCP federation flows.")],
    };
    let path = reporter.artifacts().write_json("echo_provider_contract.json", &contract)?;
    Ok(path)
}

fn fixture_root(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(path)
}

fn load_fixture<T: DeserializeOwned>(path: &Path) -> Result<T, Box<dyn std::error::Error>> {
    let data = fs::read(path)
        .map_err(|err| format!("failed to read fixture {}: {err}", path.display()))?;
    let parsed = serde_json::from_slice(&data)
        .map_err(|err| format!("failed to parse fixture {}: {err}", path.display()))?;
    Ok(parsed)
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct FixtureMap {
    #[serde(default)]
    assetcore_namespace_id: Option<u64>,
    #[serde(default)]
    fixture_version: Option<String>,
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct FixtureEntry {
    check_id: String,
    params: Value,
    expected: Value,
}
