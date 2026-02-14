// system-tests/tests/suites/cli_conformance.rs
// ============================================================================
// Module: CLI MCP Tool Conformance Tests
// Description: End-to-end CLI coverage for MCP tool wrappers.
// Purpose: Validate CLI tool wrappers execute against a live MCP server.
// Dependencies: system-tests helpers, decision-gate-mcp, decision-gate-cli
// ============================================================================

//! ## Overview
//! End-to-end CLI coverage for MCP tool wrappers.
//! Purpose: Validate CLI tool wrappers execute against a live MCP server.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::PacketPayload;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_core::runtime::SubmitRequest;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::ProviderCheckSchemaGetRequest;
use decision_gate_mcp::tools::ProviderContractGetRequest;
use decision_gate_mcp::tools::ProvidersListRequest;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackVerifyRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioSubmitRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::ScenariosListRequest;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use crate::helpers::artifacts::TestReporter;
use crate::helpers::cli::cli_binary;
use crate::helpers::harness::allocate_bind_addr;
use crate::helpers::harness::base_http_config;
use crate::helpers::harness::spawn_mcp_server;
use crate::helpers::readiness::wait_for_server_ready;
use crate::helpers::scenarios::ScenarioFixture;

#[derive(serde::Serialize)]
struct CliTranscriptEntry {
    sequence: u64,
    command: String,
    status: i32,
    stdout: String,
    stderr: String,
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|err| format!("serialize json: {err}"))?;
    fs::write(path, bytes).map_err(|err| format!("write json: {err}"))
}

fn run_cli_tool(
    cli: &Path,
    base_url: &str,
    tool: &str,
    input_path: &Path,
    transcript: &mut Vec<CliTranscriptEntry>,
    sequence: &mut u64,
) -> Result<Value, String> {
    let output = Command::new(cli)
        .args([
            "mcp",
            "tool",
            tool,
            "--endpoint",
            base_url,
            "--input",
            input_path.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|err| format!("run decision-gate mcp tool {tool} failed: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    *sequence = sequence.saturating_add(1);
    transcript.push(CliTranscriptEntry {
        sequence: *sequence,
        command: format!("mcp tool {tool}"),
        status: output.status.code().unwrap_or(-1),
        stdout,
        stderr,
    });
    if !output.status.success() {
        let stderr = transcript.last().map(|entry| entry.stderr.as_str()).unwrap_or_default();
        return Err(format!("cli mcp tool {tool} failed: {stderr}"));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("invalid json output for {tool}: {err}"))
}

fn run_config_for(fixture: &ScenarioFixture, run_id: &str) -> RunConfig {
    RunConfig {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: RunId::new(run_id),
        scenario_id: fixture.scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    }
}

fn trigger_for(
    fixture: &ScenarioFixture,
    run_id: &RunId,
    trigger_id: &str,
    time: Timestamp,
) -> TriggerEvent {
    TriggerEvent {
        trigger_id: TriggerId::new(trigger_id),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: run_id.clone(),
        kind: TriggerKind::ExternalEvent,
        time,
        source_id: "cli-conformance".to_string(),
        payload: None,
        correlation_id: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "CLI tool conformance validates the full wrapper surface."
)]
async fn cli_mcp_tool_wrappers_conformance() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_mcp_tool_wrappers_conformance")?;
    let Some(cli) = cli_binary() else {
        reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "tool_transcript.json".to_string(),
            ],
        )?;
        drop(reporter);
        return Ok(());
    };

    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;
    let base_url = server.base_url().to_string();

    let temp_dir = TempDir::new()?;
    let mut transcript: Vec<CliTranscriptEntry> = Vec::new();
    let mut sequence = 0u64;

    let mut fixture = ScenarioFixture::time_after("cli-tool-conformance", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_path = temp_dir.path().join("scenario_define.json");
    write_json_file(&define_path, &define_request)?;
    let define_output = run_cli_tool(
        &cli,
        &base_url,
        "scenario-define",
        &define_path,
        &mut transcript,
        &mut sequence,
    )?;
    let scenario_id = define_output
        .get("scenario_id")
        .and_then(Value::as_str)
        .ok_or("scenario_define missing scenario_id")?;

    let list_request = ScenariosListRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        cursor: None,
        limit: Some(10),
    };
    let list_path = temp_dir.path().join("scenarios_list.json");
    write_json_file(&list_path, &list_request)?;
    let list_output = run_cli_tool(
        &cli,
        &base_url,
        "scenarios-list",
        &list_path,
        &mut transcript,
        &mut sequence,
    )?;
    let list_items =
        list_output.get("items").and_then(Value::as_array).ok_or("scenarios_list missing items")?;
    if !list_items.iter().any(|item| {
        item.get("scenario_id").and_then(Value::as_str).is_some_and(|id| id == scenario_id)
    }) {
        return Err("scenarios_list missing defined scenario".into());
    }

    let run_next = run_config_for(&fixture, "run-next");
    let start_request = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: run_next.clone(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_path = temp_dir.path().join("scenario_start.json");
    write_json_file(&start_path, &start_request)?;
    let start_output = run_cli_tool(
        &cli,
        &base_url,
        "scenario-start",
        &start_path,
        &mut transcript,
        &mut sequence,
    )?;
    if start_output.get("run_id").is_none() {
        return Err("scenario_start missing run_id".into());
    }

    let status_request = ScenarioStatusRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: StatusRequest {
            run_id: run_next.run_id.clone(),
            tenant_id: run_next.tenant_id,
            namespace_id: run_next.namespace_id,
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let status_path = temp_dir.path().join("scenario_status.json");
    write_json_file(&status_path, &status_request)?;
    let status_output = run_cli_tool(
        &cli,
        &base_url,
        "scenario-status",
        &status_path,
        &mut transcript,
        &mut sequence,
    )?;
    if status_output.get("status").is_none() {
        return Err("scenario_status missing status".into());
    }

    let next_request = ScenarioNextRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: NextRequest {
            run_id: run_next.run_id.clone(),
            tenant_id: run_next.tenant_id,
            namespace_id: run_next.namespace_id,
            trigger_id: TriggerId::new("trigger-next"),
            agent_id: "agent-1".to_string(),
            time: Timestamp::Logical(3),
            correlation_id: None,
        },
        feedback: None,
    };
    let next_path = temp_dir.path().join("scenario_next.json");
    write_json_file(&next_path, &next_request)?;
    let next_output =
        run_cli_tool(&cli, &base_url, "scenario-next", &next_path, &mut transcript, &mut sequence)?;
    if next_output.get("decision").is_none() {
        return Err("scenario_next missing decision".into());
    }

    let run_submit = run_config_for(&fixture, "run-submit");
    let submit_start = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: run_submit.clone(),
        started_at: Timestamp::Logical(4),
        issue_entry_packets: false,
    };
    let submit_start_path = temp_dir.path().join("scenario_start_submit.json");
    write_json_file(&submit_start_path, &submit_start)?;
    run_cli_tool(
        &cli,
        &base_url,
        "scenario-start",
        &submit_start_path,
        &mut transcript,
        &mut sequence,
    )?;

    let submit_request = ScenarioSubmitRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        request: SubmitRequest {
            run_id: run_submit.run_id.clone(),
            tenant_id: run_submit.tenant_id,
            namespace_id: run_submit.namespace_id,
            submission_id: "submission-1".to_string(),
            payload: PacketPayload::Json {
                value: serde_json::json!({"artifact": "alpha"}),
            },
            content_type: "application/json".to_string(),
            submitted_at: Timestamp::Logical(5),
            correlation_id: None,
        },
    };
    let submit_path = temp_dir.path().join("scenario_submit.json");
    write_json_file(&submit_path, &submit_request)?;
    let submit_output = run_cli_tool(
        &cli,
        &base_url,
        "scenario-submit",
        &submit_path,
        &mut transcript,
        &mut sequence,
    )?;
    if submit_output.get("record").is_none() {
        return Err("scenario_submit missing record".into());
    }

    let run_trigger = run_config_for(&fixture, "run-trigger");
    let trigger_start = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: run_trigger.clone(),
        started_at: Timestamp::Logical(6),
        issue_entry_packets: false,
    };
    let trigger_start_path = temp_dir.path().join("scenario_start_trigger.json");
    write_json_file(&trigger_start_path, &trigger_start)?;
    run_cli_tool(
        &cli,
        &base_url,
        "scenario-start",
        &trigger_start_path,
        &mut transcript,
        &mut sequence,
    )?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        trigger: trigger_for(&fixture, &run_trigger.run_id, "trigger-1", Timestamp::Logical(7)),
    };
    let trigger_path = temp_dir.path().join("scenario_trigger.json");
    write_json_file(&trigger_path, &trigger_request)?;
    let trigger_output = run_cli_tool(
        &cli,
        &base_url,
        "scenario-trigger",
        &trigger_path,
        &mut transcript,
        &mut sequence,
    )?;
    if trigger_output.get("decision").is_none() {
        return Err("scenario_trigger missing decision".into());
    }

    let providers_request = ProvidersListRequest {};
    let providers_path = temp_dir.path().join("providers_list.json");
    write_json_file(&providers_path, &providers_request)?;
    let providers_output = run_cli_tool(
        &cli,
        &base_url,
        "providers-list",
        &providers_path,
        &mut transcript,
        &mut sequence,
    )?;
    if providers_output.get("providers").is_none() {
        return Err("providers_list missing providers".into());
    }

    let contract_request = ProviderContractGetRequest {
        provider_id: "time".to_string(),
    };
    let contract_path = temp_dir.path().join("provider_contract_get.json");
    write_json_file(&contract_path, &contract_request)?;
    let contract_output = run_cli_tool(
        &cli,
        &base_url,
        "provider-contract-get",
        &contract_path,
        &mut transcript,
        &mut sequence,
    )?;
    if contract_output.get("provider_id").is_none() {
        return Err("provider_contract_get missing provider_id".into());
    }

    let check_schema_request = ProviderCheckSchemaGetRequest {
        provider_id: "time".to_string(),
        check_id: "after".to_string(),
    };
    let check_schema_path = temp_dir.path().join("provider_check_schema_get.json");
    write_json_file(&check_schema_path, &check_schema_request)?;
    let check_schema_output = run_cli_tool(
        &cli,
        &base_url,
        "provider-check-schema-get",
        &check_schema_path,
        &mut transcript,
        &mut sequence,
    )?;
    if check_schema_output.get("check_id").is_none() {
        return Err("provider_check_schema_get missing check_id".into());
    }

    let schema_record = DataShapeRecord {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        schema_id: DataShapeId::new("cli-shape"),
        version: DataShapeVersion::new("v1"),
        schema: serde_json::json!({
            "type": "object",
            "properties": { "after": { "type": "boolean" } },
            "required": ["after"]
        }),
        description: Some("cli conformance schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let schema_register_request = SchemasRegisterRequest {
        record: schema_record.clone(),
    };
    let schema_register_path = temp_dir.path().join("schemas_register.json");
    write_json_file(&schema_register_path, &schema_register_request)?;
    let schema_register_output = run_cli_tool(
        &cli,
        &base_url,
        "schemas-register",
        &schema_register_path,
        &mut transcript,
        &mut sequence,
    )?;
    if schema_register_output.get("record").is_none() {
        return Err("schemas_register missing record".into());
    }

    let schema_list_request = SchemasListRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        cursor: None,
        limit: Some(10),
    };
    let schema_list_path = temp_dir.path().join("schemas_list.json");
    write_json_file(&schema_list_path, &schema_list_request)?;
    let schema_list_output = run_cli_tool(
        &cli,
        &base_url,
        "schemas-list",
        &schema_list_path,
        &mut transcript,
        &mut sequence,
    )?;
    let schema_items = schema_list_output
        .get("items")
        .and_then(Value::as_array)
        .ok_or("schemas_list missing items")?;
    if !schema_items.iter().any(|item| {
        item.get("schema_id").and_then(Value::as_str).is_some_and(|id| id == "cli-shape")
    }) {
        return Err("schemas_list missing cli-shape".into());
    }

    let schema_get_request = SchemasGetRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        schema_id: schema_record.schema_id.clone(),
        version: schema_record.version.clone(),
    };
    let schema_get_path = temp_dir.path().join("schemas_get.json");
    write_json_file(&schema_get_path, &schema_get_request)?;
    let schema_get_output = run_cli_tool(
        &cli,
        &base_url,
        "schemas-get",
        &schema_get_path,
        &mut transcript,
        &mut sequence,
    )?;
    if schema_get_output.get("record").is_none() {
        return Err("schemas_get missing record".into());
    }

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        scenario_id: Some(fixture.spec.scenario_id.clone()),
        spec: None,
        stage_id: None,
        data_shape: decision_gate_core::DataShapeRef {
            schema_id: schema_record.schema_id.clone(),
            version: schema_record.version.clone(),
        },
        payload: serde_json::json!({"after": true}),
    };
    let precheck_path = temp_dir.path().join("precheck.json");
    write_json_file(&precheck_path, &precheck_request)?;
    let precheck_output =
        run_cli_tool(&cli, &base_url, "precheck", &precheck_path, &mut transcript, &mut sequence)?;
    if precheck_output.get("decision").is_none() {
        return Err("precheck missing decision".into());
    }

    let evidence_request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: decision_gate_core::ProviderId::new("time"),
            check_id: "after".to_string(),
            params: Some(serde_json::json!({"timestamp": 0})),
        },
        context: fixture.evidence_context("trigger-evidence", Timestamp::Logical(8)),
    };
    let evidence_path = temp_dir.path().join("evidence_query.json");
    write_json_file(&evidence_path, &evidence_request)?;
    let evidence_output = run_cli_tool(
        &cli,
        &base_url,
        "evidence-query",
        &evidence_path,
        &mut transcript,
        &mut sequence,
    )?;
    if evidence_output.get("result").is_none() {
        return Err("evidence_query missing result".into());
    }

    let run_export = run_config_for(&fixture, "run-export");
    let export_start = ScenarioStartRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        run_config: run_export.clone(),
        started_at: Timestamp::Logical(9),
        issue_entry_packets: false,
    };
    let export_start_path = temp_dir.path().join("scenario_start_export.json");
    write_json_file(&export_start_path, &export_start)?;
    run_cli_tool(
        &cli,
        &base_url,
        "scenario-start",
        &export_start_path,
        &mut transcript,
        &mut sequence,
    )?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    fs::create_dir_all(&runpack_dir)?;
    let export_request = RunpackExportRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: run_export.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: false,
    };
    let export_path = temp_dir.path().join("runpack_export.json");
    write_json_file(&export_path, &export_request)?;
    let export_output = run_cli_tool(
        &cli,
        &base_url,
        "runpack-export",
        &export_path,
        &mut transcript,
        &mut sequence,
    )?;
    if export_output.get("manifest").is_none() {
        return Err("runpack_export missing manifest".into());
    }

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_path = temp_dir.path().join("runpack_verify.json");
    write_json_file(&verify_path, &verify_request)?;
    let verify_output = run_cli_tool(
        &cli,
        &base_url,
        "runpack-verify",
        &verify_path,
        &mut transcript,
        &mut sequence,
    )?;
    if verify_output.get("status").is_none() {
        return Err("runpack_verify missing status".into());
    }

    let docs_request = serde_json::json!({
        "query": "decision gate",
        "max_sections": 2
    });
    let docs_path = temp_dir.path().join("docs_search.json");
    write_json_file(&docs_path, &docs_request)?;
    let docs_output = run_cli_tool(
        &cli,
        &base_url,
        "decision-gate-docs-search",
        &docs_path,
        &mut transcript,
        &mut sequence,
    )?;
    if docs_output.get("sections").is_none() {
        return Err("decision_gate_docs_search missing sections".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["CLI MCP tool wrappers executed across full tool surface".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}
