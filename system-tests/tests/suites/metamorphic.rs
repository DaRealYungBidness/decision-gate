// system-tests/tests/suites/metamorphic.rs
// ============================================================================
// Module: Metamorphic Determinism Tests
// Description: Concurrency and ordering-insensitive determinism coverage.
// Purpose: Ensure deterministic outcomes/runpacks under concurrent runs.
// Dependencies: system-tests helpers, decision-gate-core, decision-gate-mcp
// ============================================================================

//! Metamorphic determinism tests for Decision Gate system-tests.

use std::time::Duration;

use decision_gate_core::DecisionRecord;
use decision_gate_core::GateEvalRecord;
use decision_gate_core::RunpackManifest;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerRecord;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use tempfile::tempdir;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn metamorphic_concurrent_runs_identical_runpacks() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reporter = TestReporter::new("metamorphic_concurrent_runs_identical_runpacks")?;
    let bind_a = allocate_bind_addr()?.to_string();
    let bind_b = allocate_bind_addr()?.to_string();
    let server_a = spawn_mcp_server(base_http_config(&bind_a)).await?;
    let server_b = spawn_mcp_server(base_http_config(&bind_b)).await?;
    let client_a = server_a.client(Duration::from_secs(5))?;
    let client_b = server_b.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client_a, Duration::from_secs(5)).await?;
    wait_for_server_ready(&client_b, Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("metamorphic-concurrent", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output_a: ScenarioDefineResponse =
        client_a.call_tool_typed("scenario_define", define_input.clone()).await?;
    let define_output_b: ScenarioDefineResponse =
        client_b.call_tool_typed("scenario_define", define_input).await?;

    let task_a = run_flow(client_a, &define_output_a, &fixture);
    let task_b = run_flow(client_b, &define_output_b, &fixture);

    let (manifest_a, manifest_b) = tokio::try_join!(task_a, task_b)?;

    if manifest_a.integrity.root_hash != manifest_b.integrity.root_hash {
        return Err(format!(
            "runpack root hash mismatch: {} vs {}",
            manifest_a.integrity.root_hash.value, manifest_b.integrity.root_hash.value
        )
        .into());
    }

    reporter.artifacts().write_json("run_a_manifest.json", &manifest_a)?;
    reporter.artifacts().write_json("run_b_manifest.json", &manifest_b)?;
    reporter.finish(
        "pass",
        vec!["concurrent runs yielded identical runpack root hashes".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "run_a_manifest.json".to_string(),
            "run_b_manifest.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn metamorphic_evidence_order_canonical_in_runpack() -> Result<(), Box<dyn std::error::Error>>
{
    let mut reporter = TestReporter::new("metamorphic_evidence_order_canonical_in_runpack")?;
    let bind = allocate_bind_addr()?.to_string();
    let server = spawn_mcp_server(base_http_config(&bind)).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let scenario_id = decision_gate_core::ScenarioId::new("metamorphic-evidence-order");
    let namespace_id = decision_gate_core::NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let tenant_id = decision_gate_core::TenantId::from_raw(1).expect("nonzero tenantid");
    let stage_id = decision_gate_core::StageId::new("stage-1");
    let condition_a = decision_gate_core::ConditionId::new("alpha");
    let condition_b = decision_gate_core::ConditionId::new("beta");
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![decision_gate_core::GateSpec {
                gate_id: decision_gate_core::GateId::new("gate-1"),
                requirement: ret_logic::Requirement::and(vec![
                    ret_logic::Requirement::condition(condition_b.clone()),
                    ret_logic::Requirement::condition(condition_a.clone()),
                ]),
                trust: None,
            }],
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            decision_gate_core::ConditionSpec {
                condition_id: condition_b.clone(),
                query: decision_gate_core::EvidenceQuery {
                    provider_id: decision_gate_core::ProviderId::new("time"),
                    check_id: "after".to_string(),
                    params: Some(serde_json::json!({"timestamp": 0})),
                },
                comparator: decision_gate_core::Comparator::Equals,
                expected: Some(serde_json::json!(false)),
                policy_tags: Vec::new(),
                trust: None,
            },
            decision_gate_core::ConditionSpec {
                condition_id: condition_a.clone(),
                query: decision_gate_core::EvidenceQuery {
                    provider_id: decision_gate_core::ProviderId::new("time"),
                    check_id: "after".to_string(),
                    params: Some(serde_json::json!({"timestamp": 0})),
                },
                comparator: decision_gate_core::Comparator::Equals,
                expected: Some(serde_json::json!(false)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
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

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: decision_gate_core::RunConfig {
            tenant_id,
            namespace_id,
            run_id: decision_gate_core::RunId::new("run-1"),
            scenario_id: scenario_id.clone(),
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
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: decision_gate_core::RunId::new("run-1"),
            tenant_id,
            namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-1"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "system-tests".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let temp = tempdir()?;
    let runpack_dir = temp.path().to_path_buf();
    let export_request = RunpackExportRequest {
        scenario_id: scenario_id.clone(),
        tenant_id,
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let gate_eval_bytes = std::fs::read(runpack_dir.join("artifacts").join("gate_evals.json"))?;
    let gate_evals: Vec<GateEvalRecord> = serde_json::from_slice(&gate_eval_bytes)?;
    let evidence_ids = gate_evals
        .first()
        .ok_or("missing gate eval record")?
        .evidence
        .iter()
        .map(|record| record.condition_id.as_str().to_string())
        .collect::<Vec<_>>();
    if evidence_ids != vec!["alpha".to_string(), "beta".to_string()] {
        return Err(format!("expected canonical evidence order, got {evidence_ids:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["runpack gate eval evidence ordering is canonical".to_string()],
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
async fn metamorphic_evidence_order_canonical_across_triggers()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("metamorphic_evidence_order_canonical_across_triggers")?;
    let bind = allocate_bind_addr()?.to_string();
    let server = spawn_mcp_server(base_http_config(&bind)).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let scenario_id = decision_gate_core::ScenarioId::new("metamorphic-evidence-reorder");
    let namespace_id = decision_gate_core::NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let tenant_id = decision_gate_core::TenantId::from_raw(1).expect("nonzero tenantid");
    let stage_id = decision_gate_core::StageId::new("stage-1");
    let condition_a = decision_gate_core::ConditionId::new("alpha");
    let condition_b = decision_gate_core::ConditionId::new("beta");
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: stage_id.clone(),
            entry_packets: Vec::new(),
            gates: vec![decision_gate_core::GateSpec {
                gate_id: decision_gate_core::GateId::new("gate-1"),
                requirement: ret_logic::Requirement::and(vec![
                    ret_logic::Requirement::condition(condition_b.clone()),
                    ret_logic::Requirement::condition(condition_a.clone()),
                ]),
                trust: None,
            }],
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            decision_gate_core::ConditionSpec {
                condition_id: condition_b.clone(),
                query: decision_gate_core::EvidenceQuery {
                    provider_id: decision_gate_core::ProviderId::new("time"),
                    check_id: "after".to_string(),
                    params: Some(serde_json::json!({"timestamp": 0})),
                },
                comparator: decision_gate_core::Comparator::Equals,
                expected: Some(serde_json::json!(false)),
                policy_tags: Vec::new(),
                trust: None,
            },
            decision_gate_core::ConditionSpec {
                condition_id: condition_a.clone(),
                query: decision_gate_core::EvidenceQuery {
                    provider_id: decision_gate_core::ProviderId::new("time"),
                    check_id: "after".to_string(),
                    params: Some(serde_json::json!({"timestamp": 0})),
                },
                comparator: decision_gate_core::Comparator::Equals,
                expected: Some(serde_json::json!(false)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
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

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: decision_gate_core::RunConfig {
            tenant_id,
            namespace_id,
            run_id: decision_gate_core::RunId::new("run-1"),
            scenario_id: scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        },
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    for (index, trigger_id) in ["trigger-1", "trigger-2"].iter().enumerate() {
        let trigger_request = ScenarioTriggerRequest {
            scenario_id: define_output.scenario_id.clone(),
            trigger: decision_gate_core::TriggerEvent {
                run_id: decision_gate_core::RunId::new("run-1"),
                tenant_id,
                namespace_id,
                trigger_id: decision_gate_core::TriggerId::new(*trigger_id),
                kind: decision_gate_core::TriggerKind::ExternalEvent,
                time: Timestamp::Logical(2 + index as u64),
                source_id: "system-tests".to_string(),
                payload: None,
                correlation_id: None,
            },
        };
        let trigger_input = serde_json::to_value(&trigger_request)?;
        let _trigger: decision_gate_core::runtime::TriggerResult =
            client.call_tool_typed("scenario_trigger", trigger_input).await?;
    }

    let temp = tempdir()?;
    let runpack_dir = temp.path().to_path_buf();
    let export_request = RunpackExportRequest {
        scenario_id: scenario_id.clone(),
        tenant_id,
        namespace_id,
        run_id: decision_gate_core::RunId::new("run-1"),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let gate_eval_bytes = std::fs::read(runpack_dir.join("artifacts").join("gate_evals.json"))?;
    let gate_evals: Vec<GateEvalRecord> = serde_json::from_slice(&gate_eval_bytes)?;
    if gate_evals.len() != 2 {
        return Err(format!("expected 2 gate eval records, got {}", gate_evals.len()).into());
    }
    for record in gate_evals {
        let evidence_ids = record
            .evidence
            .iter()
            .map(|item| item.condition_id.as_str().to_string())
            .collect::<Vec<_>>();
        if evidence_ids != vec!["alpha".to_string(), "beta".to_string()] {
            return Err(format!("expected canonical evidence order, got {evidence_ids:?}").into());
        }
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["evidence ordering is canonical across triggers".to_string()],
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
async fn metamorphic_concurrent_triggers_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("metamorphic_concurrent_triggers_deterministic")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.server.limits.max_inflight = 1;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let scenario_id = decision_gate_core::ScenarioId::new("metamorphic-trigger-order");
    let namespace_id = decision_gate_core::NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let tenant_id = decision_gate_core::TenantId::from_raw(1).expect("nonzero tenantid");
    let run_id = decision_gate_core::RunId::new("run-1");
    let stage1_id = decision_gate_core::StageId::new("stage-1");
    let stage2_id = decision_gate_core::StageId::new("stage-2");
    let condition_id = decision_gate_core::ConditionId::new("after");
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![
            decision_gate_core::StageSpec {
                stage_id: stage1_id,
                entry_packets: Vec::new(),
                gates: vec![decision_gate_core::GateSpec {
                    gate_id: decision_gate_core::GateId::new("gate-1"),
                    requirement: ret_logic::Requirement::condition(condition_id.clone()),
                    trust: None,
                }],
                advance_to: decision_gate_core::AdvanceTo::Linear,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
            decision_gate_core::StageSpec {
                stage_id: stage2_id,
                entry_packets: Vec::new(),
                gates: vec![decision_gate_core::GateSpec {
                    gate_id: decision_gate_core::GateId::new("gate-2"),
                    requirement: ret_logic::Requirement::condition(condition_id.clone()),
                    trust: None,
                }],
                advance_to: decision_gate_core::AdvanceTo::Terminal,
                timeout: None,
                on_timeout: decision_gate_core::TimeoutPolicy::Fail,
            },
        ],
        conditions: vec![decision_gate_core::ConditionSpec {
            condition_id: condition_id.clone(),
            query: decision_gate_core::EvidenceQuery {
                provider_id: decision_gate_core::ProviderId::new("time"),
                check_id: "after".to_string(),
                params: Some(serde_json::json!({"timestamp": 0})),
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(serde_json::json!(true)),
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

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: decision_gate_core::RunConfig {
            tenant_id,
            namespace_id,
            run_id: run_id.clone(),
            scenario_id: scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        },
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_a = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: run_id.clone(),
            tenant_id,
            namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-a"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "system-tests".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_b = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: decision_gate_core::TriggerEvent {
            run_id: run_id.clone(),
            tenant_id,
            namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-b"),
            kind: decision_gate_core::TriggerKind::ExternalEvent,
            time: Timestamp::Logical(3),
            source_id: "system-tests".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let (result_a, result_b) = tokio::join!(
        trigger_with_retry(&client, &trigger_a),
        trigger_with_retry(&client, &trigger_b)
    );
    let result_a = result_a?;
    let result_b = result_b?;

    let decision_a_trigger = result_a.decision.trigger_id.as_str();
    let decision_b_trigger = result_b.decision.trigger_id.as_str();
    if decision_a_trigger != "trigger-a" {
        return Err(
            format!("unexpected decision trigger id for trigger-a: {decision_a_trigger}").into()
        );
    }
    if decision_b_trigger != "trigger-b" {
        return Err(
            format!("unexpected decision trigger id for trigger-b: {decision_b_trigger}").into()
        );
    }

    let temp = tempdir()?;
    let runpack_dir = temp.path().to_path_buf();
    let export_request = RunpackExportRequest {
        scenario_id: scenario_id.clone(),
        tenant_id,
        namespace_id,
        run_id: run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let triggers_bytes = std::fs::read(runpack_dir.join("artifacts").join("triggers.json"))?;
    let decisions_bytes = std::fs::read(runpack_dir.join("artifacts").join("decisions.json"))?;
    let triggers: Vec<TriggerRecord> = serde_json::from_slice(&triggers_bytes)?;
    let decisions: Vec<DecisionRecord> = serde_json::from_slice(&decisions_bytes)?;

    if triggers.len() != 2 {
        return Err(format!("expected 2 trigger records, got {}", triggers.len()).into());
    }
    if decisions.len() != 2 {
        return Err(format!("expected 2 decision records, got {}", decisions.len()).into());
    }

    let mut trigger_ids = triggers
        .iter()
        .map(|record| record.event.trigger_id.as_str().to_string())
        .collect::<Vec<_>>();
    trigger_ids.sort();
    if trigger_ids != vec!["trigger-a".to_string(), "trigger-b".to_string()] {
        return Err(format!("unexpected trigger ids: {trigger_ids:?}").into());
    }

    let mut decision_trigger_ids =
        decisions.iter().map(|record| record.trigger_id.as_str().to_string()).collect::<Vec<_>>();
    decision_trigger_ids.sort();
    if decision_trigger_ids != vec!["trigger-a".to_string(), "trigger-b".to_string()] {
        return Err(format!("unexpected decision trigger ids: {decision_trigger_ids:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["concurrent trigger ordering yields deterministic records".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

async fn run_flow(
    client: helpers::mcp_client::McpHttpClient,
    define_output: &ScenarioDefineResponse,
    fixture: &ScenarioFixture,
) -> Result<RunpackManifest, Box<dyn std::error::Error>> {
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
        trigger: fixture.trigger_event("trigger-1", Timestamp::Logical(2)),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let temp = tempdir()?;
    let runpack_dir = temp.path().to_path_buf();
    let export_request = RunpackExportRequest {
        scenario_id: fixture.spec.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let manifest_bytes = std::fs::read(runpack_dir.join("manifest.json"))?;
    let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;

    Ok(manifest)
}

async fn trigger_with_retry(
    client: &helpers::mcp_client::McpHttpClient,
    request: &ScenarioTriggerRequest,
) -> Result<decision_gate_core::runtime::TriggerResult, Box<dyn std::error::Error>> {
    const MAX_ATTEMPTS: u8 = 3;
    for attempt in 1 ..= MAX_ATTEMPTS {
        let trigger_input = serde_json::to_value(request)?;
        match client.call_tool_typed("scenario_trigger", trigger_input).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                let message = err.to_lowercase();
                let is_overloaded = message.contains("overloaded") || message.contains("inflight");
                if is_overloaded && attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
                if is_overloaded {
                    return Err(format!("scenario_trigger retries exhausted: {err}").into());
                }
                return Err(err.into());
            }
        }
    }
    Err("scenario_trigger retries exhausted".into())
}
