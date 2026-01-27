// decision-gate-mcp/tests/contract_schema_e2e.rs
// ============================================================================
// Module: Contract Schema End-to-End Tests
// Description: Validate MCP tool inputs/outputs against contract schemas.
// Purpose: Ensure runtime tool responses conform to canonical JSON schemas.
// Dependencies: decision-gate-contract, decision-gate-mcp, jsonschema
// ============================================================================

//! ## Overview
//! Exercises all MCP tools end-to-end and validates payloads against the
//! Decision Gate contract schemas to guarantee schema correctness.
//!
//! Security posture: Ensures tool outputs remain deterministic and validated.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    clippy::missing_docs_in_private_items,
    reason = "Test-only schema validation uses panic-based assertions for clarity."
)]

mod common;

use std::collections::BTreeMap;
use std::error::Error;
use std::io;
use std::sync::Arc;

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
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use jsonschema::SchemaResolver;
use jsonschema::SchemaResolverError;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use url::Url;

use crate::common::local_request_context;

#[derive(Clone)]
struct ContractSchemaResolver {
    registry: Arc<BTreeMap<String, Value>>,
}

impl ContractSchemaResolver {
    fn new(registry: BTreeMap<String, Value>) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }
}

impl SchemaResolver for ContractSchemaResolver {
    fn resolve(
        &self,
        _root_schema: &Value,
        url: &Url,
        _original_reference: &str,
    ) -> Result<Arc<Value>, SchemaResolverError> {
        let key = url.as_str();
        self.registry.get(key).map_or_else(
            || Err(io::Error::new(io::ErrorKind::NotFound, key.to_string()).into()),
            |schema| Ok(Arc::new(schema.clone())),
        )
    }
}

struct ToolSchemas {
    input: JSONSchema,
    output: JSONSchema,
}

fn build_resolver() -> Result<ContractSchemaResolver, Box<dyn Error>> {
    let scenario_schema = schemas::scenario_schema();
    let config_schema = schemas::config_schema();
    let mut registry = BTreeMap::new();
    for schema in [scenario_schema, config_schema] {
        let Some(id) = schema.get("$id").and_then(Value::as_str) else {
            return Err("schema missing $id".into());
        };
        registry.insert(id.to_string(), schema);
    }
    Ok(ContractSchemaResolver::new(registry))
}

fn compile_schema(
    schema: &Value,
    resolver: &ContractSchemaResolver,
) -> Result<JSONSchema, Box<dyn Error>> {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.with_resolver(resolver.clone());
    let compiled = options.compile(schema).map_err(|err| io::Error::other(err.to_string()))?;
    Ok(compiled)
}

fn compile_tool_schemas(
    resolver: &ContractSchemaResolver,
) -> Result<BTreeMap<ToolName, ToolSchemas>, Box<dyn Error>> {
    let mut output = BTreeMap::new();
    for contract in tool_contracts() {
        let input = compile_schema(&contract.input_schema, resolver)?;
        let output_schema = compile_schema(&contract.output_schema, resolver)?;
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

fn assert_valid(schema: &JSONSchema, instance: &Value, label: &str) -> Result<(), Box<dyn Error>> {
    match schema.validate(instance) {
        Ok(()) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors.map(|err| err.to_string()).collect();
            Err(format!("validation failed ({label}): {}", messages.join("; ")).into())
        }
    }
}

fn tool_schema(
    map: &BTreeMap<ToolName, ToolSchemas>,
    name: ToolName,
) -> Result<&ToolSchemas, Box<dyn Error>> {
    map.get(&name).ok_or_else(|| format!("missing tool schema: {name}").into())
}

#[test]
#[allow(clippy::too_many_lines, reason = "End-to-end schema validation is intentionally verbose.")]
fn mcp_tool_outputs_match_contract_schemas() -> Result<(), Box<dyn Error>> {
    let resolver = build_resolver()?;
    let tool_schemas = compile_tool_schemas(&resolver)?;

    let router = common::sample_router();
    let spec = common::sample_spec();

    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_schema = tool_schema(&tool_schemas, ToolName::ScenarioDefine)?;
    assert_valid(&define_schema.input, &define_input, "scenario_define input")?;
    let define_output =
        router.handle_tool_call(&local_request_context(), "scenario_define", define_input)?;
    assert_valid(&define_schema.output, &define_output, "scenario_define output")?;
    let define_response: ScenarioDefineResponse = serde_json::from_value(define_output)?;

    let run_config = common::sample_run_config_with_ids(
        "tenant-1",
        "run-1",
        define_response.scenario_id.as_str(),
    );
    let start_request = ScenarioStartRequest {
        scenario_id: define_response.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let start_schema = tool_schema(&tool_schemas, ToolName::ScenarioStart)?;
    assert_valid(&start_schema.input, &start_input, "scenario_start input")?;
    let start_output =
        router.handle_tool_call(&local_request_context(), "scenario_start", start_input)?;
    assert_valid(&start_schema.output, &start_output, "scenario_start output")?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_response.scenario_id.clone(),
        request: StatusRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id.clone(),
            namespace_id: run_config.namespace_id.clone(),
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status_schema = tool_schema(&tool_schemas, ToolName::ScenarioStatus)?;
    assert_valid(&status_schema.input, &status_input, "scenario_status input")?;
    let status_output =
        router.handle_tool_call(&local_request_context(), "scenario_status", status_input)?;
    assert_valid(&status_schema.output, &status_output, "scenario_status output")?;

    let next_request = ScenarioNextRequest {
        scenario_id: define_response.scenario_id.clone(),
        request: NextRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id.clone(),
            namespace_id: run_config.namespace_id.clone(),
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "agent-1".to_string(),
            time: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let next_input = serde_json::to_value(&next_request)?;
    let next_schema = tool_schema(&tool_schemas, ToolName::ScenarioNext)?;
    assert_valid(&next_schema.input, &next_input, "scenario_next input")?;
    let next_output =
        router.handle_tool_call(&local_request_context(), "scenario_next", next_input)?;
    assert_valid(&next_schema.output, &next_output, "scenario_next output")?;

    let submit_request = ScenarioSubmitRequest {
        scenario_id: define_response.scenario_id.clone(),
        request: SubmitRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id.clone(),
            namespace_id: run_config.namespace_id.clone(),
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
    let submit_output =
        router.handle_tool_call(&local_request_context(), "scenario_submit", submit_input)?;
    assert_valid(&submit_schema.output, &submit_output, "scenario_submit output")?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_response.scenario_id.clone(),
        trigger: TriggerEvent {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id.clone(),
            namespace_id: run_config.namespace_id.clone(),
            trigger_id: TriggerId::new("trigger-2"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(5),
            source_id: "external-agent".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_schema = tool_schema(&tool_schemas, ToolName::ScenarioTrigger)?;
    assert_valid(&trigger_schema.input, &trigger_input, "scenario_trigger input")?;
    let trigger_output =
        router.handle_tool_call(&local_request_context(), "scenario_trigger", trigger_input)?;
    assert_valid(&trigger_schema.output, &trigger_output, "scenario_trigger output")?;

    let context = EvidenceContext {
        tenant_id: run_config.tenant_id.clone(),
        namespace_id: run_config.namespace_id.clone(),
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
            predicate: "now".to_string(),
            params: None,
        },
        context,
    };
    let evidence_input = serde_json::to_value(&evidence_request)?;
    let evidence_schema = tool_schema(&tool_schemas, ToolName::EvidenceQuery)?;
    assert_valid(&evidence_schema.input, &evidence_input, "evidence_query input")?;
    let evidence_output =
        router.handle_tool_call(&local_request_context(), "evidence_query", evidence_input)?;
    assert_valid(&evidence_schema.output, &evidence_output, "evidence_query output")?;

    let temp_dir = TempDir::new()?;
    let output_dir = temp_dir.path().to_string_lossy().to_string();
    let manifest_name = "manifest.json".to_string();
    let export_request = RunpackExportRequest {
        scenario_id: define_response.scenario_id,
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id,
        output_dir: Some(output_dir.clone()),
        manifest_name: Some(manifest_name.clone()),
        generated_at: Timestamp::Logical(7),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let export_schema = tool_schema(&tool_schemas, ToolName::RunpackExport)?;
    assert_valid(&export_schema.input, &export_input, "runpack_export input")?;
    let export_output =
        router.handle_tool_call(&local_request_context(), "runpack_export", export_input)?;
    assert_valid(&export_schema.output, &export_output, "runpack_export output")?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: output_dir,
        manifest_path: manifest_name,
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verify_schema = tool_schema(&tool_schemas, ToolName::RunpackVerify)?;
    assert_valid(&verify_schema.input, &verify_input, "runpack_verify input")?;
    let verify_output =
        router.handle_tool_call(&local_request_context(), "runpack_verify", verify_input)?;
    assert_valid(&verify_schema.output, &verify_output, "runpack_verify output")?;

    Ok(())
}

#[test]
fn mcp_config_matches_contract_schema() -> Result<(), Box<dyn Error>> {
    let resolver = build_resolver()?;
    let config_schema = compile_schema(&schemas::config_schema(), &resolver)?;
    let toml_config = r#"
[server]
transport = "stdio"

[trust]
default_policy = "audit"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[[providers]]
name = "time"
type = "builtin"
"#;
    let toml_value: toml::Value = toml::from_str(toml_config)?;
    let config_value = serde_json::to_value(toml_value)?;
    assert_valid(&config_schema, &config_value, "decision gate config")?;
    Ok(())
}
