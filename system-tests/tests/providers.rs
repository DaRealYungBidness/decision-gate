// system-tests/tests/providers.rs
// ============================================================================
// Module: Provider Tests
// Description: Built-in and federated provider coverage.
// Purpose: Validate provider predicates and MCP federation.
// Dependencies: system-tests helpers
// ============================================================================

//! Provider integration tests for Decision Gate system-tests.

mod helpers;

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_contract::types::DeterminismClass;
use decision_gate_contract::types::PredicateContract;
use decision_gate_contract::types::PredicateExample;
use decision_gate_contract::types::ProviderContract;
use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PredicateKey;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStatus;
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
use decision_gate_mcp::config::AnchorProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
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
use serde::de::DeserializeOwned;
use serde_json::Value;
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
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
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
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-echo"),
                requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
                trust: None,
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
            trust: None,
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
            namespace_id: NamespaceId::new("default"),
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
            tenant_id: decision_gate_core::TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
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
            predicate: "echo".to_string(),
            params: Some(json!({"value": true})),
        },
        context: fixture.evidence_context("timeout-trigger", Timestamp::Logical(1)),
    };
    let input = serde_json::to_value(&request)?;
    let Err(error) = client.call_tool("evidence_query", input).await else {
        return Err("expected evidence_query to time out".into());
    };
    if !error.contains("timed out") {
        return Err(format!("expected timeout error, got: {error}").into());
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
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
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
                predicate: fixture.predicate.clone(),
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
            tenant_id: run_config.tenant_id.clone(),
            namespace_id: run_config.namespace_id.clone(),
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
    predicate: String,
    params: Value,
    expected: Value,
}
