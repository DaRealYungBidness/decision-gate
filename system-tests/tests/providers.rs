// system-tests/tests/providers.rs
// ============================================================================
// Module: Provider Tests
// Description: Built-in and federated provider coverage.
// Purpose: Validate provider predicates and MCP federation.
// Dependencies: system-tests helpers
// ============================================================================

//! Provider integration tests for Decision Gate system-tests.

mod helpers;

use std::path::PathBuf;

use decision_gate_contract::types::DeterminismClass;
use decision_gate_contract::types::PredicateContract;
use decision_gate_contract::types::PredicateExample;
use decision_gate_contract::types::ProviderContract;
use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::PredicateKey;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::config_with_provider;
use helpers::harness::spawn_mcp_server;
use helpers::provider_stub::spawn_provider_stub;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn provider_time_after() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("provider_time_after")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("provider-time", "run-1", 0);

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
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "provider-test".to_string(),
            payload_ref: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match trigger_result.decision.outcome {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("unexpected decision outcome: {other:?}"),
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["time provider predicate evaluated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
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
    let predicate_key = PredicateKey::new("echo");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-echo"),
                requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
            predicate: predicate_key,
            query: EvidenceQuery {
                provider_id: ProviderId::new("echo"),
                predicate: "echo".to_string(),
                params: Some(json!({"value": true})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
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
            tenant_id: decision_gate_core::TenantId::new("tenant-1"),
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
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "provider-test".to_string(),
            payload_ref: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match trigger_result.decision.outcome {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => panic!("unexpected decision outcome: {other:?}"),
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
    Ok(())
}

fn write_echo_contract(
    reporter: &TestReporter,
    provider_id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let contract = ProviderContract {
        provider_id: provider_id.to_string(),
        name: "Echo Provider".to_string(),
        description: "Echo predicate used by system-tests for MCP federation.".to_string(),
        transport: "mcp".to_string(),
        config_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        }),
        predicates: vec![PredicateContract {
            name: "echo".to_string(),
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
            examples: vec![PredicateExample {
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
