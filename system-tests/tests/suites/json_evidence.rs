// system-tests/tests/suites/json_evidence.rs
// ============================================================================
// Module: JSON Evidence Playbook Tests
// Description: Validate JSON evidence playbook and LLM-native precheck flows.
// Purpose: Ensure playbook examples are executable end-to-end.
// Dependencies: system-tests helpers
// ============================================================================

//! JSON evidence playbook system tests.

use std::fs;
use std::num::NonZeroU64;
use std::time::Duration;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PredicateKey;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use ret_logic::Requirement;
use serde_json::Value;
use serde_json::json;
use tempfile::tempdir;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "End-to-end playbook flow kept in one block for auditability."
)]
async fn json_evidence_playbook_templates_pass() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("json_evidence_playbook_templates_pass")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let dir = tempdir()?;
    let report_path = dir.path().join("report.json");
    let coverage_path = dir.path().join("coverage.json");
    let scan_path = dir.path().join("scan.json");
    let reviews_path = dir.path().join("reviews.json");
    let quality_path = dir.path().join("quality.json");

    write_json(
        &report_path,
        &json!({
            "summary": {"failed": 0, "passed": 128},
            "tool": "tests",
            "version": "1.0"
        }),
    )?;
    write_json(&coverage_path, &json!({"coverage": {"percent": 92}}))?;
    write_json(
        &scan_path,
        &json!({
            "summary": {"critical": 0, "high": 0, "medium": 2},
            "tool": "scanner"
        }),
    )?;
    write_json(&reviews_path, &json!({"reviews": {"approvals": 2}}))?;
    write_json(&quality_path, &json!({"checks": {"lint_ok": true, "format_ok": true}}))?;

    let scenario_id = ScenarioId::new("json-evidence-playbook");
    let namespace_id = namespace_id_one();
    let stage_id = StageId::new("main");
    let tenant_id = tenant_id_one();

    let tests_ok = PredicateKey::new("tests_ok");
    let coverage_ok = PredicateKey::new("coverage_ok");
    let scan_ok = PredicateKey::new("scan_ok");
    let approvals_ok = PredicateKey::new("approvals_ok");
    let lint_ok = PredicateKey::new("lint_ok");

    let predicates = vec![
        PredicateSpec {
            predicate: tests_ok.clone(),
            query: json_path_query(&report_path, "$.summary.failed"),
            comparator: Comparator::Equals,
            expected: Some(json!(0)),
            policy_tags: Vec::new(),
            trust: None,
        },
        PredicateSpec {
            predicate: coverage_ok.clone(),
            query: json_path_query(&coverage_path, "$.coverage.percent"),
            comparator: Comparator::GreaterThanOrEqual,
            expected: Some(json!(85)),
            policy_tags: Vec::new(),
            trust: None,
        },
        PredicateSpec {
            predicate: scan_ok.clone(),
            query: json_path_query(&scan_path, "$.summary.critical"),
            comparator: Comparator::Equals,
            expected: Some(json!(0)),
            policy_tags: Vec::new(),
            trust: None,
        },
        PredicateSpec {
            predicate: approvals_ok.clone(),
            query: json_path_query(&reviews_path, "$.reviews.approvals"),
            comparator: Comparator::GreaterThanOrEqual,
            expected: Some(json!(2)),
            policy_tags: Vec::new(),
            trust: None,
        },
        PredicateSpec {
            predicate: lint_ok.clone(),
            query: json_path_query(&quality_path, "$.checks.lint_ok"),
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        },
    ];

    let gates = vec![
        gate("gate-tests", tests_ok.clone()),
        gate("gate-coverage", coverage_ok.clone()),
        gate("gate-scan", scan_ok.clone()),
        gate("gate-approvals", approvals_ok.clone()),
        gate("gate-lint", lint_ok.clone()),
    ];

    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: SpecVersion::new("v1"),
        stages: vec![StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates,
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates,
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id),
    };

    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let run_config = RunConfig {
        tenant_id,
        namespace_id,
        run_id: RunId::new("run-1"),
        scenario_id: scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger = TriggerEvent {
        run_id: RunId::new("run-1"),
        tenant_id,
        namespace_id,
        trigger_id: TriggerId::new("trigger-1"),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(2),
        source_id: "json-evidence-playbook".to_string(),
        payload: None,
        correlation_id: None,
    };
    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id,
        trigger,
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    if !matches!(trigger_result.decision.outcome, DecisionOutcome::Complete { .. }) {
        return Err(
            format!("unexpected decision outcome: {:?}", trigger_result.decision.outcome).into()
        );
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["json evidence playbook templates passed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn llm_native_precheck_payload_flow() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("llm_native_precheck_payload_flow")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let scenario_id = ScenarioId::new("llm-precheck");
    let namespace_id = namespace_id_one();
    let stage_id = StageId::new("main");
    let tenant_id = tenant_id_one();

    let predicate_key = PredicateKey::new("report_ok");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: SpecVersion::new("v1"),
        stages: vec![StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![gate("gate-quality", predicate_key.clone())],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
            predicate: predicate_key.clone(),
            query: json_path_query_pathless("$.summary.failed"),
            comparator: Comparator::Equals,
            expected: Some(json!(0)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id),
    };

    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let record = DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::new("llm-precheck"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "report_ok": { "type": "number" }
            },
            "required": ["report_ok"]
        }),
        description: Some("llm precheck payload schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: Value =
        client.call_tool_typed("schemas_register", register_input).await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: Some(define_output.scenario_id),
        spec: None,
        stage_id: Some(stage_id),
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"report_ok": 0}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let response: PrecheckToolResponse = client.call_tool_typed("precheck", precheck_input).await?;

    if !matches!(response.decision, DecisionOutcome::Complete { .. }) {
        return Err(format!("unexpected decision outcome: {:?}", response.decision).into());
    }

    if response.gate_evaluations.len() != 1 {
        return Err(
            format!("expected 1 gate evaluation, got {}", response.gate_evaluations.len()).into()
        );
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["llm-native precheck payload flow passed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

fn gate(gate_id: &str, predicate: PredicateKey) -> GateSpec {
    GateSpec {
        gate_id: GateId::new(gate_id),
        requirement: Requirement::predicate(predicate),
        trust: None,
    }
}

fn json_path_query(path: &std::path::Path, jsonpath: &str) -> decision_gate_core::EvidenceQuery {
    decision_gate_core::EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({
            "file": path.display().to_string(),
            "jsonpath": jsonpath
        })),
    }
}

fn json_path_query_pathless(jsonpath: &str) -> decision_gate_core::EvidenceQuery {
    decision_gate_core::EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({
            "file": "report.json",
            "jsonpath": jsonpath
        })),
    }
}

fn write_json(path: &std::path::Path, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(value)?;
    fs::write(path, bytes)?;
    Ok(())
}
