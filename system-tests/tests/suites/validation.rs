// system-tests/tests/suites/validation.rs
// ============================================================================
// Module: Validation Tests
// Description: End-to-end validation for strict comparator enforcement.
// Purpose: Ensure invalid comparator/type combos fail closed at tool boundaries.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! System tests for strict comparator validation behavior.

use std::num::NonZeroU64;
use std::path::PathBuf;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceQuery;
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
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::config_with_provider;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use ret_logic::Requirement;
use ret_logic::TriState;
use serde_json::Value;
use serde_json::json;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

fn time_now_spec(scenario_id: &str) -> ScenarioSpec {
    let scenario_id = ScenarioId::new(scenario_id);
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
            query: EvidenceQuery {
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

fn time_now_in_set_spec(scenario_id: &str, expected: Value) -> ScenarioSpec {
    let scenario_id = ScenarioId::new(scenario_id);
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
            query: EvidenceQuery {
                provider_id: ProviderId::new("time"),
                check_id: "now".to_string(),
                params: None,
            },
            comparator: Comparator::InSet,
            expected: Some(expected),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    }
}

fn env_contains_spec(scenario_id: &str) -> ScenarioSpec {
    let scenario_id = ScenarioId::new(scenario_id);
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
            query: EvidenceQuery {
                provider_id: ProviderId::new("env"),
                check_id: "get".to_string(),
                params: Some(json!({"key": "ENV_TEST"})),
            },
            comparator: Comparator::Contains,
            expected: Some(json!("a")),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    }
}

fn strict_provider_spec(scenario_id: &str) -> ScenarioSpec {
    let scenario_id = ScenarioId::new(scenario_id);
    let namespace_id = namespace_id_one();
    let stage_id = StageId::new("stage-1");
    let lex_key = ConditionId::new("lex");
    let deep_key = ConditionId::new("deep");
    ScenarioSpec {
        scenario_id,
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id,
            entry_packets: Vec::new(),
            gates: vec![
                GateSpec {
                    gate_id: GateId::new("gate-lex"),
                    requirement: Requirement::condition(lex_key.clone()),
                    trust: None,
                },
                GateSpec {
                    gate_id: GateId::new("gate-deep"),
                    requirement: Requirement::condition(deep_key.clone()),
                    trust: None,
                },
            ],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![
            ConditionSpec {
                condition_id: lex_key,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("strict"),
                    check_id: "lex_value".to_string(),
                    params: None,
                },
                comparator: Comparator::LexGreaterThan,
                expected: Some(json!("beta")),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: deep_key,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("strict"),
                    check_id: "deep_value".to_string(),
                    params: None,
                },
                comparator: Comparator::DeepEquals,
                expected: Some(json!({"a": 1})),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn strict_validation_precheck_rejects_comparator_mismatch()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("strict_validation_precheck_rejects_comparator_mismatch")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = decision_gate_core::TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let spec = time_now_spec("strict-precheck-mismatch");
    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

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
                "value": { "type": "string" }
            },
            "required": ["value"]
        }),
        description: Some("strict mismatch schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

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
        payload: json!({"value": "alpha"}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let Err(err) = client.call_tool("precheck", precheck_input).await else {
        return Err("expected comparator mismatch rejection".into());
    };
    if !err.contains("comparator greater_than not allowed") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["precheck rejects comparator/schema mismatch".to_string()],
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
async fn strict_validation_precheck_allows_permissive() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("strict_validation_precheck_allows_permissive")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = decision_gate_core::TrustLane::Asserted;
    config.validation.strict = false;
    config.validation.allow_permissive = true;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let spec = time_now_spec("strict-permissive-precheck");
    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

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
                "value": { "type": "string" }
            },
            "required": ["value"]
        }),
        description: Some("permissive mismatch schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

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
        payload: json!({"value": "alpha"}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let response: PrecheckToolResponse = client.call_tool_typed("precheck", precheck_input).await?;
    let eval =
        response.gate_evaluations.first().ok_or_else(|| "missing gate evaluation".to_string())?;
    if eval.status != TriState::Unknown {
        return Err(format!("expected TriState::Unknown, got {:?}", eval.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["permissive precheck proceeds with unknown comparator".to_string()],
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
async fn strict_validation_rejects_in_set_non_array() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("strict_validation_rejects_in_set_non_array")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let spec = time_now_in_set_spec("strict-in-set-non-array", json!(5));
    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let Err(err) = client.call_tool("scenario_define", define_input).await else {
        return Err("expected in_set expected array rejection".into());
    };
    if !err.contains("expected array required") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["in_set requires expected array".to_string()],
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
async fn strict_validation_precheck_allows_union_contains() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reporter = TestReporter::new("strict_validation_precheck_allows_union_contains")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.trust.min_lane = decision_gate_core::TrustLane::Asserted;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let spec = env_contains_spec("strict-union-contains");
    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

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
                "value": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                }
            },
            "required": ["value"]
        }),
        description: Some("union schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

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
        payload: json!({"value": "alpha"}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let response: PrecheckToolResponse = client.call_tool_typed("precheck", precheck_input).await?;
    match response.decision {
        DecisionOutcome::Complete {
            stage_id,
        } => {
            if stage_id.as_str() != "stage-1" {
                return Err(format!("unexpected stage id: {}", stage_id.as_str()).into());
            }
        }
        other => return Err(format!("unexpected decision: {other:?}").into()),
    }
    let eval =
        response.gate_evaluations.first().ok_or_else(|| "missing gate evaluation".to_string())?;
    if eval.status != TriState::True {
        return Err(format!("expected TriState::True, got {:?}", eval.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["union string/null schema allows contains comparator".to_string()],
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
async fn strict_validation_rejects_disabled_comparators() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reporter = TestReporter::new("strict_validation_rejects_disabled_comparators")?;
    let bind = allocate_bind_addr()?.to_string();
    let contract_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/contracts/strict_validation_provider.json");
    let config = config_with_provider(&bind, "strict", "http://127.0.0.1:1", &contract_path);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let spec = strict_provider_spec("strict-disabled-comparators");
    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let Err(err) = client.call_tool("scenario_define", define_input).await else {
        return Err("expected disabled comparator rejection".into());
    };
    if !err.contains("disabled comparator lex_greater_than") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["disabled comparator families rejected".to_string()],
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
async fn strict_validation_allows_enabled_comparators() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("strict_validation_allows_enabled_comparators")?;
    let bind = allocate_bind_addr()?.to_string();
    let contract_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/contracts/strict_validation_provider.json");
    let mut config = config_with_provider(&bind, "strict", "http://127.0.0.1:1", &contract_path);
    config.validation.enable_lexicographic = true;
    config.validation.enable_deep_equals = true;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let spec = strict_provider_spec("strict-enabled-comparators");
    let define_request = ScenarioDefineRequest {
        spec,
    };
    let define_input = serde_json::to_value(&define_request)?;
    let _define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["enabled comparator families accepted".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
