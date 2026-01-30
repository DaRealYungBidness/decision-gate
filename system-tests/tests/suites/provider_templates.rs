// system-tests/tests/suites/provider_templates.rs
// ============================================================================
// Module: Provider Template Integration Tests
// Description: End-to-end coverage for Go/Python/TypeScript MCP templates.
// Purpose: Validate tools/list, tools/call, framing limits, and Decision Gate wiring.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Provider template integration tests for Decision Gate system-tests.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::sdk_runner;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

async fn lock_provider_template_mutex() -> tokio::sync::MutexGuard<'static, ()> {
    PROVIDER_TEMPLATE_TEST_LOCK.lock().await
}

const MAX_BODY_BYTES: usize = 1024 * 1024;
static PROVIDER_TEMPLATE_TEST_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

struct ProviderProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl ProviderProcess {
    fn spawn(command: &[String]) -> Result<Self, String> {
        let (program, args) =
            command.split_first().ok_or_else(|| "provider command is empty".to_string())?;
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        let mut child = cmd.spawn().map_err(|err| format!("spawn failed: {err}"))?;
        let stdin = child.stdin.take().ok_or_else(|| "stdin missing".to_string())?;
        let stdout = child.stdout.take().ok_or_else(|| "stdout missing".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn send_request(&mut self, payload: &Value) -> Result<JsonRpcResponse, String> {
        write_frame(&mut self.stdin, payload)?;
        read_frame(&mut self.stdout)
    }

    fn send_raw_frame(&mut self, bytes: &[u8]) -> Result<JsonRpcResponse, String> {
        write_raw_frame(&mut self.stdin, bytes)?;
        read_frame(&mut self.stdout)
    }

    fn shutdown(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write_frame(writer: &mut ChildStdin, payload: &Value) -> Result<(), String> {
    let data = serde_json::to_vec(payload).map_err(|err| format!("serialize request: {err}"))?;
    write_raw_frame(writer, &data)
}

fn write_raw_frame(writer: &mut ChildStdin, bytes: &[u8]) -> Result<(), String> {
    let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
    writer.write_all(header.as_bytes()).map_err(|err| format!("write header failed: {err}"))?;
    writer.write_all(bytes).map_err(|err| format!("write body failed: {err}"))?;
    writer.flush().map_err(|err| format!("flush failed: {err}"))?;
    Ok(())
}

fn read_frame(reader: &mut BufReader<ChildStdout>) -> Result<JsonRpcResponse, String> {
    let mut content_length: Option<usize> = None;
    let mut header_bytes = 0usize;
    loop {
        let mut line = String::new();
        let read =
            reader.read_line(&mut line).map_err(|err| format!("read header failed: {err}"))?;
        if read == 0 {
            return Err("unexpected eof".to_string());
        }
        header_bytes += line.len();
        if line == "\r\n" || line == "\n" {
            break;
        }
        if line.to_ascii_lowercase().starts_with("content-length:") {
            let value = line
                .split_once(':')
                .map(|(_, value)| value)
                .ok_or_else(|| "invalid content-length header".to_string())?
                .trim();
            content_length =
                Some(value.parse::<usize>().map_err(|_| "invalid content length".to_string())?);
        }
        if header_bytes > 8 * 1024 {
            return Err("headers too large".to_string());
        }
    }
    let length = content_length.ok_or_else(|| "missing content length".to_string())?;
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).map_err(|err| format!("read body failed: {err}"))?;
    serde_json::from_slice(&payload).map_err(|err| format!("invalid json-rpc: {err}"))
}

fn tools_list_request() -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    })
}

fn tools_call_request(value: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "evidence_query",
            "arguments": {
                "query": {
                    "provider_id": "template",
                    "check_id": "value",
                    "params": { "value": value }
                },
                "context": {
                    "tenant_id": 1,
                    "namespace_id": 1,
                    "run_id": "run-1",
                    "scenario_id": "scenario-1",
                    "stage_id": "stage-1",
                    "trigger_id": "trigger-1",
                    "trigger_time": { "kind": "logical", "value": 1 },
                    "correlation_id": null
                }
            }
        }
    })
}

fn template_contract_path() -> PathBuf {
    repo_root().join("system-tests/tests/fixtures/contracts/template_provider.json")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), Path::to_path_buf)
}

fn template_scenario(value: &str, include_value_param: bool) -> ScenarioSpec {
    let scenario_id = ScenarioId::new("template-scenario");
    let namespace_id = namespace_id_one();
    let stage_id = StageId::new("stage-1");
    let condition_id = ConditionId::new("value_check");
    ScenarioSpec {
        scenario_id,
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id,
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::condition(condition_id.clone()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id,
            query: decision_gate_core::EvidenceQuery {
                provider_id: ProviderId::new("template"),
                check_id: "value".to_string(),
                params: if include_value_param {
                    Some(serde_json::json!({ "value": value }))
                } else {
                    None
                },
            },
            comparator: Comparator::Equals,
            expected: Some(Value::String(value.to_string())),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    }
}

async fn run_template_with_server(
    command: Vec<String>,
    value: &str,
    expect_error: bool,
) -> Result<(), String> {
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.providers.push(ProviderConfig {
        name: "template".to_string(),
        provider_type: ProviderType::Mcp,
        command,
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(template_contract_path()),
        auth: None,
        trust: None,
        allow_raw: true,
        timeouts: decision_gate_mcp::config::ProviderTimeoutConfig::default(),
        config: None,
    });
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let spec = template_scenario(value, true);
    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            serde_json::to_value(&define_request).map_err(|err| format!("define: {err}"))?,
        )
        .await?;
    let start_request = ScenarioStartRequest {
        scenario_id: spec.scenario_id.clone(),
        run_config: decision_gate_core::RunConfig {
            tenant_id: tenant_id_one(),
            namespace_id: spec.namespace_id,
            run_id: decision_gate_core::RunId::new("run-1"),
            scenario_id: spec.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        },
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    client
        .call_tool_typed::<decision_gate_core::RunState>(
            "scenario_start",
            serde_json::to_value(&start_request).map_err(|err| format!("start: {err}"))?,
        )
        .await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: spec.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: decision_gate_core::RunId::new("run-1"),
            tenant_id: tenant_id_one(),
            namespace_id: spec.namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "template".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let result: decision_gate_core::runtime::TriggerResult = client
        .call_tool_typed(
            "scenario_trigger",
            serde_json::to_value(&trigger_request).map_err(|err| format!("trigger: {err}"))?,
        )
        .await?;

    if expect_error {
        match result.decision.outcome {
            decision_gate_core::DecisionOutcome::Fail {
                ..
            }
            | decision_gate_core::DecisionOutcome::Hold {
                ..
            } => {}
            _ => return Err("expected decision to fail closed on provider error".to_string()),
        }
    } else if !matches!(
        result.decision.outcome,
        decision_gate_core::DecisionOutcome::Complete { .. }
    ) {
        return Err("expected decision to complete".to_string());
    }

    server.shutdown().await;
    Ok(())
}

fn assert_tools_list(response: &JsonRpcResponse) -> Result<(), String> {
    let result = response.result.as_ref().ok_or_else(|| "missing tools/list result".to_string())?;
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| "tools/list response missing tools array".to_string())?;
    if tools.is_empty() {
        return Err("tools/list returned empty list".to_string());
    }
    Ok(())
}

fn assert_tool_call_value(response: &JsonRpcResponse, expected: &str) -> Result<(), String> {
    let result = response.result.as_ref().ok_or_else(|| "missing tools/call result".to_string())?;
    let content = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| "tools/call missing content".to_string())?;
    let json = content.get("json").ok_or_else(|| "tools/call missing json content".to_string())?;
    let value = json
        .get("value")
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call missing value".to_string())?;
    if value != expected {
        return Err(format!("unexpected value: {value}"));
    }
    Ok(())
}

fn oversized_payload() -> Vec<u8> {
    let mut bytes = vec![b'a'; MAX_BODY_BYTES + 1];
    if !bytes.is_empty() {
        bytes[0] = b'{';
        let last = bytes.len() - 1;
        bytes[last] = b'}';
    }
    bytes
}

fn python_command() -> Vec<String> {
    vec![
        "python3".to_string(),
        repo_root().join("decision-gate-provider-sdk/python/provider.py").display().to_string(),
    ]
}

fn go_command() -> Vec<String> {
    vec![
        "go".to_string(),
        "run".to_string(),
        repo_root().join("decision-gate-provider-sdk/go/main.go").display().to_string(),
    ]
}

fn typescript_command(node_path: &Path) -> Vec<String> {
    vec![
        node_path.display().to_string(),
        "--experimental-strip-types".to_string(),
        repo_root()
            .join("decision-gate-provider-sdk/typescript/src/index.ts")
            .display()
            .to_string(),
    ]
}

#[allow(
    clippy::future_not_send,
    reason = "TestReporter uses a mutex guard; template tests are not spawned across threads."
)]
async fn run_template_test(
    label: &str,
    command: Vec<String>,
    runtime_notes: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new(&format!("provider_template_{label}"))?;
    reporter.artifacts().write_json("runtime_notes.json", &runtime_notes)?;

    let mut process = ProviderProcess::spawn(&command)?;
    let list_response = process.send_request(&tools_list_request())?;
    assert_tools_list(&list_response)?;

    let call_response = process.send_request(&tools_call_request("ok"))?;
    assert_tool_call_value(&call_response, "ok")?;

    let oversized = oversized_payload();
    let oversize_response = process.send_raw_frame(&oversized)?;
    let error =
        oversize_response.error.as_ref().ok_or_else(|| "expected oversize error".to_string())?;
    if !error.message.contains("payload too large") {
        return Err(format!("unexpected oversize error: {}", error.message).into());
    }

    process.shutdown();

    run_template_with_server(command.clone(), "ok", false).await?;

    let transcript = vec![
        serde_json::json!({
            "request": tools_list_request(),
            "response": serde_json::to_value(&list_response).unwrap_or(Value::Null)
        }),
        serde_json::json!({
            "request": tools_call_request("ok"),
            "response": serde_json::to_value(&call_response).unwrap_or(Value::Null)
        }),
        serde_json::json!({
            "request": format!("oversize_frame_bytes={}", oversized.len()),
            "response": serde_json::to_value(&oversize_response).unwrap_or(Value::Null)
        }),
    ];
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec![format!("provider template {label} validated")],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runtime_notes.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_template_python() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_provider_template_mutex().await;
    let runtime = match sdk_runner::python_runtime() {
        Ok(runtime) => runtime,
        Err(reason) => {
            let mut reporter = TestReporter::new("provider_template_python")?;
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };
    let mut notes = runtime.notes;
    notes.push(format!("python runtime: {}", runtime.path.display()));
    run_template_test("python", python_command(), notes).await
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_template_go() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_provider_template_mutex().await;
    let runtime = match go_runtime() {
        Ok(runtime) => runtime,
        Err(reason) => {
            let mut reporter = TestReporter::new("provider_template_go")?;
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };
    let mut notes = runtime.notes;
    notes.push(format!("go runtime: {}", runtime.path.display()));
    run_template_test("go", go_command(), notes).await
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_template_typescript() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_provider_template_mutex().await;
    let runtime = match sdk_runner::node_runtime_for_typescript() {
        Ok(runtime) => runtime,
        Err(reason) => {
            let mut reporter = TestReporter::new("provider_template_typescript")?;
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };
    let mut notes = runtime.notes;
    notes.push(format!("node runtime: {}", runtime.path.display()));
    run_template_test("typescript", typescript_command(&runtime.path), notes).await
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_template_error_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_provider_template_mutex().await;
    let runtime = match sdk_runner::python_runtime() {
        Ok(runtime) => runtime,
        Err(reason) => {
            let mut reporter = TestReporter::new("provider_template_error_fails_closed")?;
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };
    let mut reporter = TestReporter::new("provider_template_error_fails_closed")?;
    let mut notes = runtime.notes;
    notes.push(format!("python runtime: {}", runtime.path.display()));
    reporter.artifacts().write_json("runtime_notes.json", &notes)?;
    reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;

    run_template_with_server(python_command(), "error", true).await?;

    reporter.finish(
        "pass",
        vec!["provider template errors fail closed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runtime_notes.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

struct RuntimeCheck {
    path: PathBuf,
    notes: Vec<String>,
}

fn go_runtime() -> Result<RuntimeCheck, String> {
    let output = Command::new("go")
        .args(["version"])
        .output()
        .map_err(|err| format!("go unavailable: {err}"))?;
    if !output.status.success() {
        return Err("go runtime unavailable".to_string());
    }
    Ok(RuntimeCheck {
        path: PathBuf::from("go"),
        notes: Vec::new(),
    })
}
