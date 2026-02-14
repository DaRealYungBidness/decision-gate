// system-tests/tests/suites/operations.rs
// ============================================================================
// Module: Operations Tests
// Description: Operational posture and warning behavior validation.
// Purpose: Ensure insecure or dev-only modes emit explicit warnings.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Operational posture and warning behavior validation.
//! Purpose: Ensure insecure or dev-only modes emit explicit warnings.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::harness::spawn_mcp_server_with_overrides;
use helpers::readiness::wait_for_stdio_ready;
use helpers::stdio_client::StdioMcpClient;
use ret_logic::Requirement;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

#[tokio::test(flavor = "multi_thread")]
async fn dev_permissive_emits_warning() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("dev_permissive_emits_warning")?;
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[server]
transport = "stdio"
mode = "dev_permissive"

[namespace]
allow_default = false

[[providers]]
name = "time"
type = "builtin"
"#;
    std::fs::write(&config_path, config_contents)?;

    let stderr_path = reporter.artifacts().root().join("mcp.stderr.log");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
    wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;

    let _ = client.list_tools().await?;
    let stderr = std::fs::read_to_string(&stderr_path)?;
    if !stderr.contains("dev-permissive mode enabled") {
        return Err("missing dev-permissive warning in stderr".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["dev-permissive mode logs an explicit warning".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "mcp.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_health_endpoints_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_health_endpoints_ok")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;

    let base_url = server.base_url().trim_end_matches("/rpc").to_string();
    let client = reqwest::Client::new();

    let health_url = format!("{base_url}/healthz");
    let health_response = client.get(&health_url).send().await?;
    let health_status = health_response.status();
    let health_body: Value = health_response.json().await?;
    if health_status != reqwest::StatusCode::OK {
        return Err(format!("unexpected health status: {health_status}").into());
    }
    let Some(Value::String(status)) = health_body.get("status") else {
        return Err("health response missing status".into());
    };
    if status != "ok" {
        return Err(format!("unexpected health status value: {status}").into());
    }

    let ready_url = format!("{base_url}/readyz");
    let ready_response = client.get(&ready_url).send().await?;
    let ready_status = ready_response.status();
    let ready_body: Value = ready_response.json().await?;
    if ready_status != reqwest::StatusCode::OK {
        return Err(format!("unexpected ready status: {ready_status}").into());
    }
    let Some(Value::String(status)) = ready_body.get("status") else {
        return Err("ready response missing status".into());
    };
    if status != "ready" {
        return Err(format!("unexpected ready status value: {status}").into());
    }

    reporter.finish(
        "pass",
        vec!["health endpoints return ok/ready JSON".to_string()],
        vec!["summary.json".to_string(), "summary.md".to_string()],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_readiness_fails_when_store_unavailable() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_readiness_fails_when_store_unavailable")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let overrides = decision_gate_mcp::ServerOverrides {
        run_state_store: Some(SharedRunStateStore::from_store(FailingRunStateStore)),
        schema_registry: Some(SharedDataShapeRegistry::from_registry(
            InMemoryDataShapeRegistry::new(),
        )),
        ..decision_gate_mcp::ServerOverrides::default()
    };
    let server = spawn_mcp_server_with_overrides(config, overrides).await?;

    let base_url = server.base_url().trim_end_matches("/rpc").to_string();
    let client = reqwest::Client::new();

    let ready_url = format!("{base_url}/readyz");
    let ready_response = client.get(&ready_url).send().await?;
    let ready_status = ready_response.status();
    let ready_body: Value = ready_response.json().await?;
    if ready_status != reqwest::StatusCode::SERVICE_UNAVAILABLE {
        return Err(format!("unexpected ready status: {ready_status}").into());
    }
    let Some(Value::String(status)) = ready_body.get("status") else {
        return Err("ready response missing status".into());
    };
    if status != "not_ready" {
        return Err(format!("unexpected ready status value: {status}").into());
    }

    reporter.finish(
        "pass",
        vec!["ready endpoint returns not_ready when store is unavailable".to_string()],
        vec!["summary.json".to_string(), "summary.md".to_string()],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

struct FailingRunStateStore;

impl RunStateStore for FailingRunStateStore {
    fn load(
        &self,
        _tenant_id: &TenantId,
        _namespace_id: &NamespaceId,
        _run_id: &decision_gate_core::RunId,
    ) -> Result<Option<RunState>, StoreError> {
        Ok(None)
    }

    fn save(&self, _state: &RunState) -> Result<(), StoreError> {
        Ok(())
    }

    fn readiness(&self) -> Result<(), StoreError> {
        Err(StoreError::Store("store unavailable".to_string()))
    }
}

fn precheck_spec() -> ScenarioSpec {
    let scenario_id = ScenarioId::new("precheck-audit");
    let namespace_id = namespace_id_one();
    let stage_id = StageId::new("stage-1");
    let condition_id = ConditionId::new("value");
    ScenarioSpec {
        scenario_id,
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id,
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: Requirement::condition(condition_id.clone()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id,
            query: decision_gate_core::EvidenceQuery {
                provider_id: ProviderId::new("time"),
                check_id: "now".to_string(),
                params: None,
            },
            comparator: Comparator::GreaterThan,
            expected: Some(json!(100)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "Precheck audit setup is clearer as a full integration test."
)]
async fn precheck_audit_hash_only() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("precheck_audit_hash_only")?;
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[server]
transport = "stdio"
mode = "strict"

[server.audit]
enabled = true
log_precheck_payloads = false

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "stdio"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[[providers]]
name = "time"
type = "builtin"
"#;
    std::fs::write(&config_path, config_contents)?;

    let stderr_path = reporter.artifacts().root().join("mcp.stderr.log");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
    wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;

    let spec = precheck_spec();
    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        serde_json::from_value(client.call_tool("scenario_define", define_input).await?)?;

    let tenant_id = tenant_id_one();
    let record = DataShapeRecord {
        tenant_id,
        namespace_id: spec.namespace_id,
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "value": { "type": "number" }
            },
            "required": ["value"]
        }),
        description: Some("precheck audit schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: Value =
        serde_json::from_value(client.call_tool("schemas_register", register_input).await?)?;

    let precheck_request = PrecheckToolRequest {
        tenant_id,
        namespace_id: spec.namespace_id,
        scenario_id: Some(define_output.scenario_id),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"value": 200}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let _response: PrecheckToolResponse =
        serde_json::from_value(client.call_tool("precheck", precheck_input).await?)?;

    let stderr = std::fs::read_to_string(&stderr_path)?;
    let mut precheck_event: Option<Value> = None;
    for line in stderr.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("event").and_then(Value::as_str) == Some("precheck_audit") {
            precheck_event = Some(value);
            break;
        }
    }
    let event = precheck_event.ok_or("missing precheck audit event")?;
    let redaction = event.get("redaction").and_then(Value::as_str);
    if redaction != Some("hash_only") {
        return Err(format!("unexpected redaction: {redaction:?}").into());
    }
    if event.get("request_hash").is_none() || event.get("response_hash").is_none() {
        return Err("missing precheck audit hashes".into());
    }
    if event.get("request").is_some_and(|value| !value.is_null()) {
        return Err("unexpected raw precheck request in audit".into());
    }
    if event.get("response").is_some_and(|value| !value.is_null()) {
        return Err("unexpected raw precheck response in audit".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["precheck audit emits hash-only event".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "mcp.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
