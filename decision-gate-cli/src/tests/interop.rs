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

use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

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
use serde::Serialize;
use serde_json::Value;

use crate::interop::InteropConfig;
use crate::interop::run_interop;
use crate::interop::validate_inputs;

fn minimal_spec(id: &str) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new(id),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("v1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn minimal_run_config(scenario_id: &ScenarioId) -> RunConfig {
    RunConfig {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
        scenario_id: scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    }
}

fn minimal_trigger(run_config: &RunConfig) -> TriggerEvent {
    TriggerEvent {
        trigger_id: TriggerId::new("trigger-1"),
        tenant_id: run_config.tenant_id.clone(),
        namespace_id: run_config.namespace_id.clone(),
        run_id: run_config.run_id.clone(),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(1),
        source_id: "test".to_string(),
        payload: None,
        correlation_id: None,
    }
}

type Responder = Arc<Mutex<Box<dyn FnMut(Value) -> Value + Send>>>;

struct TestMcpServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestMcpServer {
    fn start<F>(expected_calls: usize, responder: F) -> Self
    where
        F: FnMut(Value) -> Value + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("server addr");
        listener.set_nonblocking(true).expect("nonblocking listener");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responder: Responder = Arc::new(Mutex::new(Box::new(responder)));

        let requests_handle = Arc::clone(&requests);
        let responder_handle = Arc::clone(&responder);
        let handle = thread::spawn(move || {
            for _ in 0 .. expected_calls {
                let Some(mut stream) = accept_with_timeout(&listener, Duration::from_secs(5))
                else {
                    break;
                };
                let _ = handle_connection(&mut stream, &requests_handle, &responder_handle);
            }
        });

        Self {
            addr,
            requests,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/rpc", self.addr)
    }

    fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl Drop for TestMcpServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn accept_with_timeout(listener: &TcpListener, timeout: Duration) -> Option<TcpStream> {
    let start = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Some(stream),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= timeout {
                    return None;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    requests: &Arc<Mutex<Vec<Value>>>,
    responder: &Responder,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|err| format!("set read timeout: {err}"))?;
    let request = read_json_body(stream)?;
    requests.lock().expect("requests lock").push(request.clone());
    let response = {
        let mut handler = responder.lock().expect("responder lock");
        handler(request)
    };
    write_json_response(stream, &response)?;
    Ok(())
}

fn read_json_body(stream: &mut TcpStream) -> Result<Value, String> {
    let mut buffer = Vec::new();
    let mut header_end = None;
    let mut content_length = None;

    loop {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).map_err(|err| format!("read request: {err}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[.. read]);
        if header_end.is_none()
            && let Some(end) = find_header_end(&buffer)
        {
            header_end = Some(end);
            content_length = Some(parse_content_length(&buffer[.. end])?);
        }
        if let (Some(end), Some(length)) = (header_end, content_length) {
            let available = buffer.len().saturating_sub(end);
            if available >= length {
                let body = &buffer[end .. end + length];
                return serde_json::from_slice(body).map_err(|err| format!("parse json: {err}"));
            }
        }
    }
    Err("incomplete request body".to_string())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n").map(|pos| pos + 4)
}

fn parse_content_length(header: &[u8]) -> Result<usize, String> {
    let header = std::str::from_utf8(header).map_err(|err| format!("invalid header: {err}"))?;
    for line in header.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            return value.trim().parse().map_err(|err| format!("invalid content-length: {err}"));
        }
    }
    Err("missing content-length".to_string())
}

fn write_json_response(stream: &mut TcpStream, response: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(response).map_err(|err| format!("serialize response: {err}"))?;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: \
         close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).map_err(|err| format!("write header: {err}"))?;
    stream.write_all(&body).map_err(|err| format!("write body: {err}"))?;
    stream.flush().map_err(|err| format!("flush: {err}"))?;
    Ok(())
}

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
    trigger.tenant_id = TenantId::new("tenant-2");
    assert!(validate_inputs(&spec, &run_config, &trigger).is_err());
}

#[test]
fn validate_inputs_rejects_namespace_mismatch() {
    let spec = minimal_spec("scenario-1");
    let run_config = minimal_run_config(&spec.scenario_id);
    let mut trigger = minimal_trigger(&run_config);
    trigger.namespace_id = NamespaceId::new("namespace-2");
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

    let run_state = RunState {
        tenant_id: run_config.tenant_id.clone(),
        namespace_id: run_config.namespace_id.clone(),
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
    };

    let decision = DecisionRecord {
        decision_id: DecisionId::new("decision-1"),
        seq: 1,
        trigger_id: trigger.trigger_id.clone(),
        stage_id: StageId::new("stage-1"),
        decided_at: Timestamp::Logical(12),
        outcome: DecisionOutcome::Start {
            stage_id: StageId::new("stage-1"),
        },
        correlation_id: trigger.correlation_id.clone(),
    };

    let trigger_result = TriggerResult {
        decision: decision.clone(),
        packets: Vec::new(),
        status: RunStatus::Active,
    };

    let status = ScenarioStatus {
        run_id: run_config.run_id.clone(),
        scenario_id: run_config.scenario_id.clone(),
        current_stage_id: StageId::new("stage-1"),
        status: RunStatus::Active,
        last_decision: Some(decision),
        issued_packet_ids: Vec::new(),
        safe_summary: None,
    };

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
            "scenario_define" => jsonrpc_response(&request, &define_response),
            "scenario_start" => jsonrpc_response(&request, &run_state_response),
            "scenario_trigger" => jsonrpc_response(&request, &trigger_response),
            "scenario_status" => jsonrpc_response(&request, &status_response),
            _ => jsonrpc_error(&request, -32601, "unknown tool"),
        }
    });

    let config = InteropConfig {
        mcp_url: server.url(),
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

    let requests = server.requests();
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
            tenant_id: run_config.tenant_id.clone(),
            namespace_id: run_config.namespace_id.clone(),
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

    let server =
        TestMcpServer::start(1, move |request| jsonrpc_response(&request, &define_response));

    let config = InteropConfig {
        mcp_url: server.url(),
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
    assert_eq!(server.requests().len(), 1);
}
