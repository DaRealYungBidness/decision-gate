// system-tests/tests/suites/transport_parity.rs
// ============================================================================
// Module: Transport Parity Tests
// Description: Cross-transport parity for HTTP, stdio, and CLI interop.
// Purpose: Ensure identical runs produce identical outcomes/runpacks.
// Dependencies: system-tests helpers, decision-gate-cli
// ============================================================================

//! Transport parity tests for Decision Gate system-tests.


use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_cli::interop::InteropConfig;
use decision_gate_cli::interop::run_interop;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::RunStatus;
use decision_gate_core::RunpackManifest;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;
use helpers::readiness::wait_for_stdio_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::stdio_client::StdioMcpClient;
use serde::Serialize;
use tempfile::TempDir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn multi_transport_parity() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("multi_transport_parity")?;
    let mut fixture = ScenarioFixture::time_after("transport-parity", "run-parity", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let trigger = TriggerEvent {
        run_id: fixture.run_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(2),
        source_id: "transport-parity".to_string(),
        payload: None,
        correlation_id: None,
    };

    let http_outcome =
        run_http_transport(&fixture, &trigger, reporter.artifacts().runpack_dir().join("http"))
            .await?;
    let stdio_outcome = run_stdio_transport(
        &fixture,
        &trigger,
        reporter.artifacts().runpack_dir().join("stdio"),
        reporter.artifacts().root().join("mcp.stderr.log"),
    )
    .await?;
    let cli_outcome =
        run_cli_transport(&fixture, &trigger, reporter.artifacts().runpack_dir().join("cli"))
            .await?;

    assert_outcome_parity("http", &http_outcome, "stdio", &stdio_outcome)?;
    assert_outcome_parity("http", &http_outcome, "cli", &cli_outcome)?;

    reporter.artifacts().write_json(
        "tool_transcript.json",
        &vec![http_outcome.transcript, stdio_outcome.transcript, cli_outcome.transcript],
    )?;
    reporter.artifacts().write_json("http_manifest.json", &http_outcome.manifest)?;
    reporter.artifacts().write_json("stdio_manifest.json", &stdio_outcome.manifest)?;
    reporter.artifacts().write_json("cli_manifest.json", &cli_outcome.manifest)?;

    reporter.finish(
        "pass",
        vec!["transport parity verified across HTTP, stdio, and CLI".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "http_manifest.json".to_string(),
            "stdio_manifest.json".to_string(),
            "cli_manifest.json".to_string(),
            "runpack/".to_string(),
            "mcp.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);

    Ok(())
}

#[derive(Debug, Serialize)]
struct TransportTranscript {
    transport: String,
    transcript: Vec<helpers::mcp_client::TranscriptEntry>,
}

struct TransportOutcome {
    manifest: RunpackManifest,
    decision_outcome: DecisionOutcome,
    status: RunStatus,
    transcript: TransportTranscript,
}

async fn run_http_transport(
    fixture: &ScenarioFixture,
    trigger: &TriggerEvent,
    runpack_dir: PathBuf,
) -> Result<TransportOutcome, Box<dyn std::error::Error>> {
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let (decision_outcome, status) = run_scenario_http(&client, fixture, trigger).await?;
    let manifest = export_runpack_http(&client, fixture, &runpack_dir).await?;

    Ok(TransportOutcome {
        manifest,
        decision_outcome,
        status,
        transcript: TransportTranscript {
            transport: "http".to_string(),
            transcript: client.transcript(),
        },
    })
}

async fn run_stdio_transport(
    fixture: &ScenarioFixture,
    trigger: &TriggerEvent,
    runpack_dir: PathBuf,
    stderr_path: PathBuf,
) -> Result<TransportOutcome, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let config_contents = r#"[server]
transport = "stdio"
mode = "strict"

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
    fs::write(&config_path, config_contents)?;

    fs::create_dir_all(&runpack_dir)?;
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_decision_gate_stdio_server"));
    let client = StdioMcpClient::spawn(&binary, &config_path, &stderr_path)?;
    wait_for_stdio_ready(&client, Duration::from_secs(5)).await?;

    let (decision_outcome, status) = run_scenario_stdio(&client, fixture, trigger).await?;
    let manifest = export_runpack_stdio(&client, fixture, &runpack_dir).await?;

    Ok(TransportOutcome {
        manifest,
        decision_outcome,
        status,
        transcript: TransportTranscript {
            transport: "stdio".to_string(),
            transcript: client.transcript(),
        },
    })
}

async fn run_cli_transport(
    fixture: &ScenarioFixture,
    trigger: &TriggerEvent,
    runpack_dir: PathBuf,
) -> Result<TransportOutcome, Box<dyn std::error::Error>> {
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let report = run_interop(InteropConfig {
        mcp_url: server.base_url().to_string(),
        spec: fixture.spec.clone(),
        run_config: fixture.run_config(),
        trigger: trigger.clone(),
        started_at: Timestamp::Logical(1),
        status_requested_at: Timestamp::Logical(3),
        issue_entry_packets: false,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(5),
    })
    .await?;

    let manifest = export_runpack_http(&client, fixture, &runpack_dir).await?;

    Ok(TransportOutcome {
        manifest,
        decision_outcome: report.trigger_result.decision.outcome,
        status: report.status.status,
        transcript: TransportTranscript {
            transport: "cli".to_string(),
            transcript: report
                .transcript
                .into_iter()
                .map(|entry| helpers::mcp_client::TranscriptEntry {
                    sequence: entry.sequence,
                    method: entry.method,
                    request: entry.request,
                    response: entry.response,
                    error: entry.error,
                })
                .collect(),
        },
    })
}

async fn run_scenario_http(
    client: &McpHttpClient,
    fixture: &ScenarioFixture,
    trigger: &TriggerEvent,
) -> Result<(DecisionOutcome, RunStatus), Box<dyn std::error::Error>> {
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
        trigger: trigger.clone(),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: decision_gate_core::runtime::StatusRequest {
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            run_id: fixture.run_id.clone(),
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    Ok((trigger_result.decision.outcome, status.status))
}

async fn run_scenario_stdio(
    client: &StdioMcpClient,
    fixture: &ScenarioFixture,
    trigger: &TriggerEvent,
) -> Result<(DecisionOutcome, RunStatus), Box<dyn std::error::Error>> {
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output = client.call_tool("scenario_define", define_input).await?;
    let define_response: ScenarioDefineResponse = serde_json::from_value(define_output)?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_response.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        serde_json::from_value(client.call_tool("scenario_start", start_input).await?)?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_response.scenario_id.clone(),
        trigger: trigger.clone(),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: decision_gate_core::runtime::TriggerResult =
        serde_json::from_value(client.call_tool("scenario_trigger", trigger_input).await?)?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_response.scenario_id,
        request: decision_gate_core::runtime::StatusRequest {
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            run_id: fixture.run_id.clone(),
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        serde_json::from_value(client.call_tool("scenario_status", status_input).await?)?;

    Ok((trigger_result.decision.outcome, status.status))
}

async fn export_runpack_http(
    client: &McpHttpClient,
    fixture: &ScenarioFixture,
    runpack_dir: &PathBuf,
) -> Result<RunpackManifest, Box<dyn std::error::Error>> {
    fs::create_dir_all(runpack_dir)?;
    let export_request = RunpackExportRequest {
        scenario_id: fixture.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(4),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;
    read_manifest(runpack_dir)
}

async fn export_runpack_stdio(
    client: &StdioMcpClient,
    fixture: &ScenarioFixture,
    runpack_dir: &PathBuf,
) -> Result<RunpackManifest, Box<dyn std::error::Error>> {
    fs::create_dir_all(runpack_dir)?;
    let export_request = RunpackExportRequest {
        scenario_id: fixture.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(4),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        serde_json::from_value(client.call_tool("runpack_export", export_input).await?)?;
    read_manifest(runpack_dir)
}

fn read_manifest(runpack_dir: &Path) -> Result<RunpackManifest, Box<dyn std::error::Error>> {
    let manifest_path = runpack_dir.join("manifest.json");
    let bytes = fs::read(&manifest_path)?;
    let manifest: RunpackManifest = serde_json::from_slice(&bytes)?;
    Ok(manifest)
}

fn assert_outcome_parity(
    left_label: &str,
    left: &TransportOutcome,
    right_label: &str,
    right: &TransportOutcome,
) -> Result<(), Box<dyn std::error::Error>> {
    if left.manifest.integrity.root_hash != right.manifest.integrity.root_hash {
        return Err(format!(
            "root hash mismatch ({left_label} vs {right_label}): {} vs {}",
            left.manifest.integrity.root_hash.value, right.manifest.integrity.root_hash.value
        )
        .into());
    }
    if left.decision_outcome != right.decision_outcome {
        return Err(format!(
            "decision mismatch ({left_label} vs {right_label}): {:?} vs {:?}",
            left.decision_outcome, right.decision_outcome
        )
        .into());
    }
    if left.status != right.status {
        return Err(format!(
            "status mismatch ({left_label} vs {right_label}): {:?} vs {:?}",
            left.status, right.status
        )
        .into());
    }
    Ok(())
}
