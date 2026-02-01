// decision-gate-cli/src/tests/interop.rs
// ============================================================================
// Module: Interop Input Validation Tests
// Description: Unit tests for interop input consistency checks.
// Purpose: Ensure scenario/run/trigger identifiers must align.
// Dependencies: decision-gate-core, decision-gate-cli interop module
// ============================================================================

//! ## Overview
//! Validates the interop input checker rejects mismatched identifiers and
//! accepts aligned inputs.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use decision_gate_core::AdvanceTo;
use decision_gate_core::DecisionId;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DecisionRecord;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::HashDigest;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::ScenarioStatus;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::StatusRequest;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TriggerResult;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::body::Incoming;
use hyper::header::CONNECTION;
use hyper::header::CONTENT_LENGTH;
use hyper::header::CONTENT_TYPE;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::interop::InteropConfig;
use crate::interop::InteropTransport;
use crate::interop::MAX_INTEROP_RESPONSE_BYTES;
use crate::interop::run_interop;
use crate::interop::validate_inputs;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

fn minimal_spec(id: &str) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new(id),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("v1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn minimal_run_config(scenario_id: &ScenarioId) -> RunConfig {
    RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    }
}

fn minimal_trigger(run_config: &RunConfig) -> TriggerEvent {
    TriggerEvent {
        trigger_id: TriggerId::new("trigger-1"),
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id.clone(),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    }
}

fn run_state_for(
    run_config: &RunConfig,
    spec_hash: &HashDigest,
    started_at: Timestamp,
) -> RunState {
    RunState {
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id.clone(),
        scenario_id: run_config.scenario_id.clone(),
        spec_hash: spec_hash.clone(),
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: started_at,
        status: RunStatus::Active,
        dispatch_targets: Vec::new(),
        triggers: Vec::new(),
        gate_evals: Vec::new(),
        decisions: Vec::new(),
        packets: Vec::new(),
        submissions: Vec::new(),
        tool_calls: Vec::new(),
    }
}

fn decision_for(trigger: &TriggerEvent, decided_at: Timestamp) -> DecisionRecord {
    DecisionRecord {
        decision_id: DecisionId::new("decision-1"),
        seq: 1,
        trigger_id: trigger.trigger_id.clone(),
        stage_id: StageId::new("stage-1"),
        decided_at,
        outcome: DecisionOutcome::Start {
            stage_id: StageId::new("stage-1"),
        },
        correlation_id: trigger.correlation_id.clone(),
    }
}

fn trigger_result_for(trigger: &TriggerEvent, decided_at: Timestamp) -> TriggerResult {
    TriggerResult {
        decision: decision_for(trigger, decided_at),
        packets: Vec::new(),
        status: RunStatus::Active,
    }
}

fn status_for(run_config: &RunConfig, last_decision: Option<DecisionRecord>) -> ScenarioStatus {
    ScenarioStatus {
        run_id: run_config.run_id.clone(),
        scenario_id: run_config.scenario_id.clone(),
        current_stage_id: StageId::new("stage-1"),
        status: RunStatus::Active,
        last_decision,
        issued_packet_ids: Vec::new(),
        safe_summary: None,
    }
}

// ============================================================================
// SECTION: Test Server
// ============================================================================

type Responder = Arc<Mutex<Box<dyn FnMut(Value) -> TestResponse + Send>>>;

#[derive(Clone, Debug)]
struct TestResponse {
    status: StatusCode,
    headers: hyper::HeaderMap,
    body: Bytes,
}

impl TestResponse {
    fn json(value: &Value) -> Self {
        let body = serde_json::to_vec(value).expect("serialize response body");
        let mut headers = hyper::HeaderMap::new();
        headers.insert(CONTENT_TYPE, hyper::header::HeaderValue::from_static("application/json"));
        Self::raw(StatusCode::OK, headers, Bytes::from(body))
    }

    fn raw(status: StatusCode, mut headers: hyper::HeaderMap, body: Bytes) -> Self {
        if !headers.contains_key(CONTENT_LENGTH) {
            let length = body.len().to_string();
            headers.insert(
                CONTENT_LENGTH,
                hyper::header::HeaderValue::from_str(&length).expect("content-length header value"),
            );
        }
        headers
            .entry(CONNECTION)
            .or_insert_with(|| hyper::header::HeaderValue::from_static("close"));
        Self {
            status,
            headers,
            body,
        }
    }

    fn into_response(self) -> Response<Full<Bytes>> {
        let mut builder = Response::builder().status(self.status);
        {
            let headers = builder.headers_mut().expect("response headers");
            for (name, value) in self.headers {
                if let Some(name) = name {
                    headers.insert(name, value);
                }
            }
        }
        builder.body(Full::new(self.body)).expect("build response body")
    }
}

struct TestMcpServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl TestMcpServer {
    async fn start<F>(expected_calls: usize, responder: F) -> Self
    where
        F: FnMut(Value) -> TestResponse + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test server");
        let addr = listener.local_addr().expect("server addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responder: Responder = Arc::new(Mutex::new(Box::new(responder)));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        let task = tokio::spawn({
            let requests = Arc::clone(&requests);
            let responder = Arc::clone(&responder);
            async move {
                let _ = ready_tx.send(());
                let mut remaining = expected_calls;
                let mut connections = Vec::new();
                loop {
                    if remaining == 0 {
                        break;
                    }
                    tokio::select! {
                        _ = &mut shutdown_rx => break,
                        accept = listener.accept() => {
                            let Ok((stream, _)) = accept else {
                                continue;
                            };
                            remaining = remaining.saturating_sub(1);
                            let requests = Arc::clone(&requests);
                            let responder = Arc::clone(&responder);
                            connections.push(tokio::spawn(async move {
                                let io = TokioIo::new(stream);
                                let service = service_fn(move |req| {
                                    handle_request(req, Arc::clone(&requests), Arc::clone(&responder))
                                });
                                let _ = http1::Builder::new()
                                    .keep_alive(false)
                                    .serve_connection(io, service)
                                    .await;
                            }));
                        }
                    }
                }
                for connection in connections {
                    let _ = connection.await;
                }
            }
        });

        let _ = ready_rx.await;
        Self {
            addr,
            requests,
            shutdown: Some(shutdown_tx),
            task: Some(task),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/rpc", self.addr)
    }

    async fn requests(&self) -> Vec<Value> {
        self.requests.lock().await.clone()
    }

    async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for TestMcpServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn handle_request(
    request: Request<Incoming>,
    requests: Arc<Mutex<Vec<Value>>>,
    responder: Responder,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let body_bytes = match request.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => {
            let response = TestResponse::raw(
                StatusCode::BAD_REQUEST,
                hyper::HeaderMap::new(),
                Bytes::from(format!("failed to read body: {err}")),
            );
            return Ok(response.into_response());
        }
    };
    let request_value = serde_json::from_slice(&body_bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body_bytes).into_owned()));
    requests.lock().await.push(request_value.clone());
    let response = {
        let mut handler = responder.lock().await;
        (handler)(request_value)
    };
    Ok(response.into_response())
}

// ============================================================================
// SECTION: JSON Helpers
// ============================================================================

fn jsonrpc_response<T: Serialize>(request: &Value, payload: &T) -> Value {
    let payload = serde_json::to_value(payload).expect("serialize payload");
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [
                { "type": "json", "json": payload }
            ]
        }
    })
}

fn jsonrpc_error(request: &Value, code: i64, message: &str) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn jsonrpc_response_with_len<T: Serialize>(
    request: &Value,
    payload: &T,
    target_len: usize,
) -> Bytes {
    let response = jsonrpc_response(request, payload);
    let mut body = serde_json::to_vec(&response).expect("serialize sized response");
    assert!(
        body.len() <= target_len,
        "response body too large for target len ({} > {})",
        body.len(),
        target_len
    );
    let padding = target_len - body.len();
    if padding > 0 {
        body.extend(std::iter::repeat_n(b' ', padding));
    }
    Bytes::from(body)
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn validate_inputs_accepts_matching_ids() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    assert!(validate_inputs(&spec, &run_config, &trigger).is_ok());
}

#[test]
fn validate_inputs_rejects_scenario_mismatch() {
    let spec = minimal_spec("scenario-1");
    let mut run_config = minimal_run_config(&spec.scenario_id);
    run_config.scenario_id = ScenarioId::new("scenario-2");
    let trigger = minimal_trigger(&run_config);
    assert!(validate_inputs(&spec, &run_config, &trigger).is_err());
}

#[test]
fn validate_inputs_rejects_run_mismatch() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let mut trigger = minimal_trigger(&run_config);
    trigger.run_id = RunId::new("run-2");
    assert!(validate_inputs(&spec, &run_config, &trigger).is_err());
}

#[test]
fn validate_inputs_rejects_tenant_mismatch() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let mut trigger = minimal_trigger(&run_config);
    trigger.tenant_id = TenantId::from_raw(2).expect("nonzero tenantid");
    assert!(validate_inputs(&spec, &run_config, &trigger).is_err());
}

#[test]
fn validate_inputs_rejects_namespace_mismatch() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let mut trigger = minimal_trigger(&run_config);
    trigger.namespace_id = NamespaceId::from_raw(2).expect("nonzero namespaceid");
    assert!(validate_inputs(&spec, &run_config, &trigger).is_err());
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "End-to-end interop test keeps the full sequence in one place."
)]
async fn run_interop_executes_full_sequence() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let started_at = Timestamp::Logical(10);
    let status_requested_at = Timestamp::Logical(11);
    let spec_hash = HashDigest::new(HashAlgorithm::Sha256, b"spec-hash");
    let run_state = run_state_for(&run_config, &spec_hash, started_at);
    let trigger_result = trigger_result_for(&trigger, Timestamp::Logical(12));
    let status = status_for(&run_config, Some(trigger_result.decision.clone()));

    let define_response = ScenarioDefineResponse {
        scenario_id: spec.scenario_id.clone(),
        spec_hash: spec_hash.clone(),
    };
    let run_state_response = run_state.clone();
    let trigger_response = trigger_result.clone();
    let status_response = status.clone();

    let server = TestMcpServer::start(4, move |request| {
        let name = request
            .get("params")
            .and_then(|params| params.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        match name {
            "scenario_define" => TestResponse::json(&jsonrpc_response(&request, &define_response)),
            "scenario_start" => {
                TestResponse::json(&jsonrpc_response(&request, &run_state_response))
            }
            "scenario_trigger" => {
                TestResponse::json(&jsonrpc_response(&request, &trigger_response))
            }
            "scenario_status" => TestResponse::json(&jsonrpc_response(&request, &status_response)),
            _ => TestResponse::json(&jsonrpc_error(&request, -32601, "unknown tool")),
        }
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec: spec.clone(),
        run_config: run_config.clone(),
        trigger: trigger.clone(),
        started_at,
        status_requested_at,
        issue_entry_packets: true,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let report = run_interop(config).await.expect("run interop");
    assert_eq!(report.spec_hash, spec_hash);
    assert_eq!(report.trigger_result, trigger_result);
    assert_eq!(report.status, status);
    assert_eq!(report.transcript.len(), 4);
    assert!(report.transcript.iter().all(|entry| entry.error.is_none()));

    let requests = server.requests().await;
    assert_eq!(requests.len(), 4);
    let names: Vec<&str> = requests
        .iter()
        .map(|request| {
            request
                .get("params")
                .and_then(|params| params.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        names,
        vec!["scenario_define", "scenario_start", "scenario_trigger", "scenario_status"]
    );

    let define_args = serde_json::to_value(ScenarioDefineRequest {
        spec: spec.clone(),
    })
    .expect("serialize define");
    let start_args = serde_json::to_value(ScenarioStartRequest {
        scenario_id: spec.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at,
        issue_entry_packets: true,
    })
    .expect("serialize start");
    let trigger_args = serde_json::to_value(ScenarioTriggerRequest {
        scenario_id: spec.scenario_id.clone(),
        trigger: trigger.clone(),
    })
    .expect("serialize trigger");
    let status_args = serde_json::to_value(ScenarioStatusRequest {
        scenario_id: spec.scenario_id.clone(),
        request: StatusRequest {
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            run_id: run_config.run_id.clone(),
            requested_at: status_requested_at,
            correlation_id: trigger.correlation_id.clone(),
        },
    })
    .expect("serialize status");

    assert_eq!(requests[0]["params"]["arguments"], define_args);
    assert_eq!(requests[1]["params"]["arguments"], start_args);
    assert_eq!(requests[2]["params"]["arguments"], trigger_args);
    assert_eq!(requests[3]["params"]["arguments"], status_args);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_rejects_define_scenario_mismatch() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let spec_hash = HashDigest::new(HashAlgorithm::Sha256, b"spec-hash");
    let define_response = ScenarioDefineResponse {
        scenario_id: ScenarioId::new("scenario-2"),
        spec_hash,
    };

    let server = TestMcpServer::start(1, move |request| {
        TestResponse::json(&jsonrpc_response(&request, &define_response))
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(2),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let err = run_interop(config).await.expect_err("expected mismatch error");
    assert!(err.contains("scenario_define returned unexpected scenario_id"));
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_accepts_response_at_size_limit() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let started_at = Timestamp::Logical(10);
    let status_requested_at = Timestamp::Logical(11);
    let spec_hash = HashDigest::new(HashAlgorithm::Sha256, b"spec-hash");
    let define_response = ScenarioDefineResponse {
        scenario_id: spec.scenario_id.clone(),
        spec_hash: spec_hash.clone(),
    };
    let run_state = run_state_for(&run_config, &spec_hash, started_at);
    let trigger_result = trigger_result_for(&trigger, Timestamp::Logical(12));
    let status = status_for(&run_config, None);

    let run_state_response = run_state.clone();
    let trigger_response = trigger_result.clone();
    let status_response = status.clone();
    let server = TestMcpServer::start(4, move |request| {
        let name = request
            .get("params")
            .and_then(|params| params.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        match name {
            "scenario_define" => {
                let body = jsonrpc_response_with_len(
                    &request,
                    &define_response,
                    MAX_INTEROP_RESPONSE_BYTES,
                );
                TestResponse::raw(StatusCode::OK, hyper::HeaderMap::new(), body)
            }
            "scenario_start" => {
                TestResponse::json(&jsonrpc_response(&request, &run_state_response))
            }
            "scenario_trigger" => {
                TestResponse::json(&jsonrpc_response(&request, &trigger_response))
            }
            "scenario_status" => TestResponse::json(&jsonrpc_response(&request, &status_response)),
            _ => TestResponse::json(&jsonrpc_error(&request, -32601, "unknown tool")),
        }
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at,
        status_requested_at,
        issue_entry_packets: true,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let report = run_interop(config).await.expect("run interop");
    assert_eq!(report.spec_hash, spec_hash);
    assert_eq!(report.status, status);
    assert_eq!(report.transcript.len(), 4);
    assert!(report.transcript.iter().all(|entry| entry.error.is_none()));
    assert_eq!(server.requests().await.len(), 4);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_rejects_oversized_response_body() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let oversized = Value::String("x".repeat(64));
    let server = TestMcpServer::start(1, move |request| {
        let body = jsonrpc_response_with_len(&request, &oversized, MAX_INTEROP_RESPONSE_BYTES + 1);
        TestResponse::raw(StatusCode::OK, hyper::HeaderMap::new(), body)
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(2),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let err = run_interop(config).await.expect_err("expected size limit error");
    assert!(err.contains("response body exceeds size limit"), "unexpected error: {err}");
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_rejects_invalid_jsonrpc_response() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let server = TestMcpServer::start(1, move |_request| {
        TestResponse::raw(StatusCode::OK, hyper::HeaderMap::new(), Bytes::from_static(b"{not-json"))
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(2),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let err = run_interop(config).await.expect_err("expected invalid json-rpc error");
    assert!(err.contains("invalid json-rpc response"), "unexpected error: {err}");
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_rejects_http_error_status() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let server = TestMcpServer::start(1, move |_request| {
        TestResponse::raw(
            StatusCode::INTERNAL_SERVER_ERROR,
            hyper::HeaderMap::new(),
            Bytes::from_static(b"server error"),
        )
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(2),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let err = run_interop(config).await.expect_err("expected http error");
    assert!(err.contains("http status 500"), "unexpected error: {err}");
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_rejects_jsonrpc_error_payload() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let server = TestMcpServer::start(1, move |request| {
        TestResponse::json(&jsonrpc_error(&request, -32000, "boom"))
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(2),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let err = run_interop(config).await.expect_err("expected json-rpc error");
    assert!(err.contains("json-rpc error"), "unexpected error: {err}");
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn run_interop_rejects_tool_without_json_content() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let trigger = minimal_trigger(&run_config);
    let server = TestMcpServer::start(1, move |request| {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": []
            }
        });
        TestResponse::json(&response)
    })
    .await;

    let config = InteropConfig {
        transport: InteropTransport::Http,
        endpoint: Some(server.url()),
        stdio_command: None,
        stdio_args: Vec::new(),
        stdio_env: Vec::new(),
        spec,
        run_config,
        trigger,
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(2),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(2),
    };

    let err = run_interop(config).await.expect_err("expected missing json content error");
    assert!(err.contains("returned no json content"), "unexpected error: {err}");
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}
