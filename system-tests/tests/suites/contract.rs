// system-tests/tests/suites/contract.rs
// ============================================================================
// Module: Contract Tests
// Description: Schema conformance validation for MCP tools.
// Purpose: Ensure runtime tool payloads match the canonical contract schemas.
// Dependencies: decision-gate-contract, jsonschema
// ============================================================================

//! ## Overview
//! Schema conformance validation for MCP tools.
//! Purpose: Ensure runtime tool payloads match the canonical contract schemas.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::collections::BTreeMap;
use std::error::Error;
use std::io;

use decision_gate_contract::ToolName;
use decision_gate_contract::schemas;
use decision_gate_contract::tooling::tool_contracts;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::PacketPayload;
use decision_gate_core::ProviderId;
use decision_gate_core::StageId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_core::runtime::SubmitRequest;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackVerifyRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioSubmitRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use jsonschema::Draft;
use jsonschema::Registry;
use jsonschema::Validator;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

struct ToolSchemas {
    input: Validator,
    output: Validator,
}

fn build_registry() -> Result<Registry, Box<dyn Error>> {
    let scenario_schema = schemas::scenario_schema();
    let config_schema = schemas::config_schema();
    let mut resources = Vec::new();
    for schema in [scenario_schema, config_schema] {
        let Some(id) = schema.get("$id").and_then(Value::as_str) else {
            return Err("schema missing $id".into());
        };
        resources.push((id.to_string(), Draft::Draft202012.create_resource(schema)));
    }
    Ok(Registry::try_from_resources(resources)?)
}

fn compile_schema(schema: &Value, registry: &Registry) -> Result<Validator, Box<dyn Error>> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(registry.clone())
        .build(schema)
        .map_err(|err| io::Error::other(err.to_string()).into())
}

fn compile_tool_schemas(
    registry: &Registry,
) -> Result<BTreeMap<ToolName, ToolSchemas>, Box<dyn Error>> {
    let mut output = BTreeMap::new();
    for contract in tool_contracts() {
        let input = compile_schema(&contract.input_schema, registry)?;
        let output_schema = compile_schema(&contract.output_schema, registry)?;
        output.insert(
            contract.name,
            ToolSchemas {
                input,
                output: output_schema,
            },
        );
    }
    Ok(output)
}

fn assert_valid(schema: &Validator, instance: &Value, label: &str) -> Result<(), Box<dyn Error>> {
    let messages: Vec<String> = schema.iter_errors(instance).map(|err| err.to_string()).collect();
    if messages.is_empty() {
        Ok(())
    } else {
        Err(format!("validation failed ({label}): {}", messages.join("; ")).into())
    }
}

fn tool_schema(
    map: &BTreeMap<ToolName, ToolSchemas>,
    name: ToolName,
) -> Result<&ToolSchemas, Box<dyn Error>> {
    map.get(&name).ok_or_else(|| format!("missing tool schema: {name}").into())
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Schema validation is intentionally exhaustive.")]
async fn schema_conformance_all_tools() -> Result<(), Box<dyn Error>> {
    let mut reporter = TestReporter::new("schema_conformance_all_tools")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(10))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(10)).await?;

    let registry = build_registry()?;
    let tool_schemas = compile_tool_schemas(&registry)?;

    let mut fixture = ScenarioFixture::time_after("contract-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_schema = tool_schema(&tool_schemas, ToolName::ScenarioDefine)?;
    assert_valid(&define_schema.input, &define_input, "scenario_define input")?;
    let define_output = client.call_tool("scenario_define", define_input).await?;
    assert_valid(&define_schema.output, &define_output, "scenario_define output")?;
    let define_response: ScenarioDefineResponse = serde_json::from_value(define_output)?;

    let run_config = fixture.run_config();
    let start_request = ScenarioStartRequest {
        scenario_id: define_response.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let start_schema = tool_schema(&tool_schemas, ToolName::ScenarioStart)?;
    assert_valid(&start_schema.input, &start_input, "scenario_start input")?;
    let start_output = client.call_tool("scenario_start", start_input).await?;
    assert_valid(&start_schema.output, &start_output, "scenario_start output")?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_response.scenario_id.clone(),
        request: StatusRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status_schema = tool_schema(&tool_schemas, ToolName::ScenarioStatus)?;
    assert_valid(&status_schema.input, &status_input, "scenario_status input")?;
    let status_output = client.call_tool("scenario_status", status_input).await?;
    assert_valid(&status_schema.output, &status_output, "scenario_status output")?;

    let next_request = ScenarioNextRequest {
        scenario_id: define_response.scenario_id.clone(),
        request: NextRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "agent-1".to_string(),
            time: Timestamp::Logical(3),
            correlation_id: None,
        },
        feedback: None,
    };
    let next_input = serde_json::to_value(&next_request)?;
    let next_schema = tool_schema(&tool_schemas, ToolName::ScenarioNext)?;
    assert_valid(&next_schema.input, &next_input, "scenario_next input")?;
    let next_output = client.call_tool("scenario_next", next_input).await?;
    assert_valid(&next_schema.output, &next_output, "scenario_next output")?;

    let submit_request = ScenarioSubmitRequest {
        scenario_id: define_response.scenario_id.clone(),
        request: SubmitRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            submission_id: "submission-1".to_string(),
            payload: PacketPayload::Json {
                value: json!({"artifact": "alpha"}),
            },
            content_type: "application/json".to_string(),
            submitted_at: Timestamp::Logical(4),
            correlation_id: None,
        },
    };
    let submit_input = serde_json::to_value(&submit_request)?;
    let submit_schema = tool_schema(&tool_schemas, ToolName::ScenarioSubmit)?;
    assert_valid(&submit_schema.input, &submit_input, "scenario_submit input")?;
    let submit_output = client.call_tool("scenario_submit", submit_input).await?;
    assert_valid(&submit_schema.output, &submit_output, "scenario_submit output")?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_response.scenario_id.clone(),
        trigger: TriggerEvent {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            trigger_id: TriggerId::new("trigger-2"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(5),
            source_id: "contract".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_schema = tool_schema(&tool_schemas, ToolName::ScenarioTrigger)?;
    assert_valid(&trigger_schema.input, &trigger_input, "scenario_trigger input")?;
    let trigger_output = client.call_tool("scenario_trigger", trigger_input).await?;
    assert_valid(&trigger_schema.output, &trigger_output, "scenario_trigger output")?;

    let context = EvidenceContext {
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id.clone(),
        scenario_id: define_response.scenario_id.clone(),
        stage_id: StageId::new("stage-1"),
        trigger_id: TriggerId::new("trigger-ctx"),
        trigger_time: Timestamp::Logical(6),
        correlation_id: None,
    };
    let evidence_request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            check_id: "now".to_string(),
            params: None,
        },
        context,
    };
    let evidence_input = serde_json::to_value(&evidence_request)?;
    let evidence_schema = tool_schema(&tool_schemas, ToolName::EvidenceQuery)?;
    assert_valid(&evidence_schema.input, &evidence_input, "evidence_query input")?;
    let evidence_output = client.call_tool("evidence_query", evidence_input).await?;
    assert_valid(&evidence_schema.output, &evidence_output, "evidence_query output")?;

    let temp_dir = TempDir::new()?;
    let output_dir = temp_dir.path().to_string_lossy().to_string();
    let manifest_name = "manifest.json".to_string();
    let export_request = RunpackExportRequest {
        scenario_id: define_response.scenario_id.clone(),
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id.clone(),
        output_dir: Some(output_dir.clone()),
        manifest_name: Some(manifest_name.clone()),
        generated_at: Timestamp::Logical(7),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let export_schema = tool_schema(&tool_schemas, ToolName::RunpackExport)?;
    assert_valid(&export_schema.input, &export_input, "runpack_export input")?;
    let export_output = client.call_tool("runpack_export", export_input).await?;
    assert_valid(&export_schema.output, &export_output, "runpack_export output")?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: output_dir,
        manifest_path: manifest_name,
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verify_schema = tool_schema(&tool_schemas, ToolName::RunpackVerify)?;
    assert_valid(&verify_schema.input, &verify_input, "runpack_verify input")?;
    let verify_output = client.call_tool("runpack_verify", verify_input).await?;
    assert_valid(&verify_schema.output, &verify_output, "runpack_verify output")?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["all tool payloads matched contract schemas".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
