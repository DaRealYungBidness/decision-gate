// system-tests/tests/suites/anchor_fuzz.rs
// ============================================================================
// Module: Anchor Fuzz Tests
// Description: Malformed anchor and injection coverage for AssetCore anchors.
// Purpose: Ensure malformed anchors fail closed with explicit error codes.
// Dependencies: system-tests helpers, decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Malformed anchor and injection coverage for AssetCore anchors.
//! Purpose: Ensure malformed anchors fail closed with explicit error codes.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::num::NonZeroU64;
use std::time::Duration;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateEvalRecord;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_mcp::config::AnchorProviderConfig;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::config_with_provider;
use helpers::harness::spawn_mcp_server;
use helpers::provider_stub::ProviderFixture;
use helpers::provider_stub::spawn_provider_fixture_stub;
use helpers::readiness::wait_for_server_ready;
use serde::Serialize;
use serde_json::json;

use crate::helpers;

fn tenant_id(value: u64) -> TenantId {
    TenantId::new(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
}

fn namespace_id(value: u64) -> NamespaceId {
    NamespaceId::new(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
}

const ASSETCORE_PROVIDER_ID: &str = "assetcore_read";
const ASSETCORE_ANCHOR_TYPE: &str = "assetcore.anchor_set";

#[tokio::test(flavor = "multi_thread")]
async fn anchor_validation_fuzz_cases_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("anchor_validation_fuzz_cases_fail_closed")?;

    let cases = vec![
        AnchorCase::new(
            "malformed_json",
            EvidenceAnchor {
                anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
                anchor_value: "not-json".to_string(),
            },
            "anchor_invalid",
        ),
        AnchorCase::new(
            "non_object",
            EvidenceAnchor {
                anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
                anchor_value: "[1,2]".to_string(),
            },
            "anchor_invalid",
        ),
        AnchorCase::new(
            "missing_field",
            EvidenceAnchor {
                anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
                anchor_value: json!({
                    "assetcore.namespace_id": 42,
                    "assetcore.world_seq": 9
                })
                .to_string(),
            },
            "anchor_invalid",
        ),
        AnchorCase::new(
            "wrong_type",
            EvidenceAnchor {
                anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
                anchor_value: json!({
                    "assetcore.namespace_id": {"nested": true},
                    "assetcore.commit_id": "c1",
                    "assetcore.world_seq": 9
                })
                .to_string(),
            },
            "anchor_invalid",
        ),
        AnchorCase::new(
            "oversize_anchor",
            EvidenceAnchor {
                anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
                anchor_value: oversized_anchor_value(),
            },
            "provider_error",
        ),
    ];

    let mut transcripts: Vec<CaseTranscript> = Vec::new();
    for case in cases {
        let outcome = run_case(&case, &reporter).await?;
        if outcome.error_code != case.expected_error {
            return Err(format!(
                "case {} expected error {}, got {}",
                case.label, case.expected_error, outcome.error_code
            )
            .into());
        }
        transcripts.push(CaseTranscript {
            case: case.label.clone(),
            transcript: outcome.transcript,
        });
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["anchor fuzz cases failed closed with expected errors".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[derive(Clone)]
struct AnchorCase {
    label: String,
    anchor: EvidenceAnchor,
    expected_error: String,
}

impl AnchorCase {
    fn new(label: &str, anchor: EvidenceAnchor, expected_error: &str) -> Self {
        Self {
            label: label.to_string(),
            anchor,
            expected_error: expected_error.to_string(),
        }
    }
}

struct CaseOutcome {
    error_code: String,
    transcript: Vec<helpers::mcp_client::TranscriptEntry>,
}

#[derive(Serialize)]
struct CaseTranscript {
    case: String,
    transcript: Vec<helpers::mcp_client::TranscriptEntry>,
}

async fn run_case(
    case: &AnchorCase,
    reporter: &TestReporter,
) -> Result<CaseOutcome, Box<dyn std::error::Error>> {
    let fixture = assetcore_fixture(&format!("anchor-fuzz-{}", case.label), "run-1");
    let provider_fixture = ProviderFixture {
        check_id: "slot_occupied".to_string(),
        params: json!({"container_id": "slots-gear", "slot_index": 1}),
        result: json!(true),
        anchor: Some(case.anchor.clone()),
    };
    let provider = spawn_provider_fixture_stub(vec![provider_fixture]).await?;

    let bind = allocate_bind_addr()?.to_string();
    let provider_contract = fixture_root("assetcore/providers").join("assetcore_read.json");
    let mut config =
        config_with_provider(&bind, ASSETCORE_PROVIDER_ID, provider.base_url(), &provider_contract);
    config.anchors.providers.push(assetcore_anchor_policy());

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

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
        trigger: fixture.trigger(None),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match trigger_result.decision.outcome {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => return Err(format!("expected hold decision, got {other:?}").into()),
    }

    let runpack_dir =
        reporter.artifacts().runpack_dir().join(format!("anchor-fuzz-{}", case.label));
    fs::create_dir_all(&runpack_dir)?;
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id,
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(4),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let gate_evals_path = runpack_dir.join("artifacts/gate_evals.json");
    let gate_evals_bytes = fs::read(&gate_evals_path)?;
    let gate_evals: Vec<GateEvalRecord> = serde_json::from_slice(&gate_evals_bytes)?;
    let error_code = gate_evals
        .first()
        .and_then(|record| record.evidence.first())
        .and_then(|record| record.result.error.as_ref())
        .map_or_else(|| "missing_error".to_string(), |error| error.code.clone());

    Ok(CaseOutcome {
        error_code,
        transcript: client.transcript(),
    })
}

fn assetcore_anchor_policy() -> AnchorProviderConfig {
    AnchorProviderConfig {
        provider_id: ASSETCORE_PROVIDER_ID.to_string(),
        anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
        required_fields: vec![
            "assetcore.namespace_id".to_string(),
            "assetcore.commit_id".to_string(),
            "assetcore.world_seq".to_string(),
        ],
    }
}

fn oversized_anchor_value() -> String {
    let padding = "X".repeat(1024 * 1024);
    json!({
        "assetcore.namespace_id": 42,
        "assetcore.commit_id": "oversize",
        "assetcore.world_seq": 9,
        "padding": padding
    })
    .to_string()
}

fn fixture_root(path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(path)
}

struct AssetcoreFixture {
    scenario_id: ScenarioId,
    run_id: RunId,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    spec: ScenarioSpec,
}

impl AssetcoreFixture {
    fn run_config(&self) -> RunConfig {
        RunConfig {
            tenant_id: self.tenant_id,
            namespace_id: self.namespace_id,
            run_id: self.run_id.clone(),
            scenario_id: self.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        }
    }

    fn trigger(&self, correlation_id: Option<decision_gate_core::CorrelationId>) -> TriggerEvent {
        TriggerEvent {
            run_id: self.run_id.clone(),
            tenant_id: self.tenant_id,
            namespace_id: self.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "assetcore-fuzz".to_string(),
            payload: None,
            correlation_id,
        }
    }
}

fn assetcore_fixture(scenario: &str, run: &str) -> AssetcoreFixture {
    let scenario_id = ScenarioId::new(scenario);
    let namespace_id = namespace_id(11);
    let stage_id = StageId::new("stage-1");
    let condition_id = ConditionId::new("slot_occupied");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id,
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::condition(condition_id.clone()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id,
            query: EvidenceQuery {
                provider_id: ProviderId::new(ASSETCORE_PROVIDER_ID),
                check_id: "slot_occupied".to_string(),
                params: Some(json!({"container_id": "slots-gear", "slot_index": 1})),
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

    AssetcoreFixture {
        scenario_id,
        run_id: RunId::new(run),
        tenant_id: tenant_id(1),
        namespace_id,
        spec,
    }
}
