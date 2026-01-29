// system-tests/tests/suites/ret_logic_authoring.rs
// ============================================================================
// Module: RET Logic Authoring Tests
// Description: End-to-end authoring normalization and execution coverage.
// Purpose: Ensure RON authoring inputs execute through Decision Gate flows.
// Dependencies: system-tests helpers, decision-gate-cli
// ============================================================================

//! RET logic authoring coverage for Decision Gate system-tests.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use decision_gate_core::DecisionOutcome;
use decision_gate_core::PredicateKey;
use decision_gate_core::RunStatus;
use decision_gate_core::Timestamp;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use serde_json::Value;
use tempfile::TempDir;
use toml::Value as TomlValue;

use crate::helpers;

fn cli_binary() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_decision_gate").map(PathBuf::from)
}

#[tokio::test(flavor = "multi_thread")]
async fn authoring_ron_normalize_and_execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("authoring_ron_normalize_and_execute")?;
    let Some(cli) = cli_binary() else {
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec!["summary.json".to_string(), "summary.md".to_string()],
        )?;
        return Ok(());
    };
    let temp_dir = TempDir::new()?;
    let ron_path = PathBuf::from("Docs/generated/decision-gate/examples/scenario.ron");
    let normalized_path = temp_dir.path().join("scenario.json");

    let output = Command::new(&cli)
        .args([
            "authoring",
            "normalize",
            "--input",
            ron_path.to_str().unwrap_or_default(),
            "--format",
            "ron",
            "--output",
            normalized_path.to_str().unwrap_or_default(),
        ])
        .output()?;
    reporter
        .artifacts()
        .write_text("authoring.normalize.stdout.log", &String::from_utf8_lossy(&output.stdout))?;
    reporter
        .artifacts()
        .write_text("authoring.normalize.stderr.log", &String::from_utf8_lossy(&output.stderr))?;
    if !output.status.success() {
        return Err("authoring normalize failed".into());
    }

    let spec: decision_gate_core::ScenarioSpec =
        serde_json::from_slice(&fs::read(&normalized_path)?)?;

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    if let Some(env_provider) = config.providers.iter_mut().find(|provider| provider.name == "env")
    {
        let mut overrides = toml::value::Table::new();
        overrides.insert("DEPLOY_ENV".to_string(), TomlValue::String("production".to_string()));
        let mut env_config = toml::value::Table::new();
        env_config.insert("overrides".to_string(), TomlValue::Table(overrides));
        env_provider.config = Some(TomlValue::Table(env_config));
    }
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let define_request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec: spec.clone(),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            serde_json::to_value(&define_request)?,
        )
        .await?;

    let run_config = decision_gate_core::RunConfig {
        tenant_id: decision_gate_core::TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: spec.namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: spec.scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    let start_request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: spec.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at: Timestamp::UnixMillis(1710000000001),
        issue_entry_packets: true,
    };
    client
        .call_tool_typed::<decision_gate_core::RunState>(
            "scenario_start",
            serde_json::to_value(&start_request)?,
        )
        .await?;

    let trigger_request = decision_gate_mcp::tools::ScenarioTriggerRequest {
        scenario_id: spec.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::UnixMillis(1710000000001),
            source_id: "authoring".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", serde_json::to_value(&trigger_request)?).await?;

    if !matches!(trigger.decision.outcome, DecisionOutcome::Complete { .. }) {
        return Err("expected decision to complete for normalized authoring spec".into());
    }
    if trigger.status != RunStatus::Completed {
        return Err(format!("expected completed status, got {:?}", trigger.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["authoring RON normalized and executed successfully".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "authoring.normalize.stdout.log".to_string(),
            "authoring.normalize.stderr.log".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn authoring_invalid_ron_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("authoring_invalid_ron_rejected")?;
    let Some(cli) = cli_binary() else {
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec!["summary.json".to_string(), "summary.md".to_string()],
        )?;
        return Ok(());
    };
    let temp_dir = TempDir::new()?;
    let invalid_path = temp_dir.path().join("invalid.ron");
    fs::write(&invalid_path, "{ this is not ron")?;

    let output = Command::new(&cli)
        .args([
            "authoring",
            "validate",
            "--input",
            invalid_path.to_str().unwrap_or_default(),
            "--format",
            "ron",
        ])
        .output()?;
    reporter
        .artifacts()
        .write_text("authoring.invalid.stdout.log", &String::from_utf8_lossy(&output.stdout))?;
    reporter
        .artifacts()
        .write_text("authoring.invalid.stderr.log", &String::from_utf8_lossy(&output.stderr))?;
    if output.status.success() {
        return Err("expected invalid authoring input to fail".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<Value>::new())?;
    reporter.finish(
        "pass",
        vec!["invalid RON authoring input rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "authoring.invalid.stdout.log".to_string(),
            "authoring.invalid.stderr.log".to_string(),
        ],
    )?;
    Ok(())
}

struct PredicateResolver {
    map: HashMap<String, PredicateKey>,
}

impl ret_logic::dsl::PredicateResolver<PredicateKey> for PredicateResolver {
    fn resolve(&self, name: &str) -> Option<PredicateKey> {
        self.map.get(name).cloned()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn authoring_dsl_evaluates_and_rejects_deep_inputs() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reporter = TestReporter::new("authoring_dsl_evaluates_and_rejects_deep_inputs")?;

    let predicate_key = PredicateKey::new("after");
    let resolver = PredicateResolver {
        map: HashMap::from([("after".to_string(), predicate_key.clone())]),
    };
    let requirement = ret_logic::dsl::parse_requirement("all(after)", &resolver)
        .map_err(|err| err.to_string())?;

    let scenario_id = decision_gate_core::ScenarioId::new("dsl-scenario");
    let namespace_id = decision_gate_core::NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let stage_id = decision_gate_core::StageId::new("stage-1");
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id: namespace_id.clone(),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id,
            entry_packets: Vec::new(),
            gates: vec![decision_gate_core::GateSpec {
                gate_id: decision_gate_core::GateId::new("dsl-gate"),
                requirement,
                trust: None,
            }],
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: vec![decision_gate_core::PredicateSpec {
            predicate: predicate_key,
            query: decision_gate_core::EvidenceQuery {
                provider_id: decision_gate_core::ProviderId::new("time"),
                predicate: "after".to_string(),
                params: Some(serde_json::json!({ "timestamp": 0 })),
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(serde_json::json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(
            decision_gate_core::TenantId::from_raw(1).expect("nonzero tenantid"),
        ),
    };

    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let define_request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec: spec.clone(),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            serde_json::to_value(&define_request)?,
        )
        .await?;
    let run_config = decision_gate_core::RunConfig {
        tenant_id: decision_gate_core::TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    let start_request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: scenario_id.clone(),
        run_config: run_config.clone(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    client
        .call_tool_typed::<decision_gate_core::RunState>(
            "scenario_start",
            serde_json::to_value(&start_request)?,
        )
        .await?;
    let trigger_request = decision_gate_mcp::tools::ScenarioTriggerRequest {
        scenario_id,
        trigger: decision_gate_core::TriggerEvent {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "dsl".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", serde_json::to_value(&trigger_request)?).await?;
    if !matches!(trigger.decision.outcome, DecisionOutcome::Complete { .. }) {
        return Err("dsl requirement did not complete as expected".into());
    }

    let deep_input = format!("{}after{}", "all(".repeat(40), ")".repeat(40));
    let deep_result = ret_logic::dsl::parse_requirement::<PredicateKey, _>(&deep_input, &resolver);
    if deep_result.is_ok() {
        return Err("expected deep DSL input to be rejected".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["DSL authoring executes and deep inputs are rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}
