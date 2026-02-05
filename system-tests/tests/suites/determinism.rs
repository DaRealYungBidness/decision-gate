// system-tests/tests/suites/determinism.rs
// ============================================================================
// Module: Determinism Tests
// Description: Determinism and replay coverage for AssetCore fixtures.
// Purpose: Ensure identical ASC fixture runs yield identical outcomes/runpacks.
// Dependencies: system-tests helpers, decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Determinism and replay coverage for AssetCore fixtures.
//! Purpose: Ensure identical ASC fixture runs yield identical outcomes/runpacks.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::DecisionOutcome;
use decision_gate_core::RunConfig;
use decision_gate_core::RunStatus;
use decision_gate_core::RunpackManifest;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_mcp::config::AnchorProviderConfig;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::config_with_provider;
use helpers::harness::spawn_mcp_server;
use helpers::provider_stub::ProviderFixture;
use helpers::provider_stub::spawn_provider_fixture_stub;
use helpers::readiness::wait_for_server_ready;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;

use crate::helpers;

const ASSETCORE_PROVIDER_ID: &str = "assetcore_read";
const ASSETCORE_ANCHOR_TYPE: &str = "assetcore.anchor_set";

#[tokio::test(flavor = "multi_thread")]
async fn assetcore_determinism_replay() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("assetcore_determinism_replay")?;

    let fixture_root_dir = fixture_root("assetcore/interop");
    let spec: ScenarioSpec =
        load_fixture(&fixture_root_dir.join("scenarios/assetcore-interop-full.json"))?;
    let run_config: RunConfig =
        load_fixture(&fixture_root_dir.join("run-configs/assetcore-interop-full.json"))?;
    let trigger: TriggerEvent =
        load_fixture(&fixture_root_dir.join("triggers/assetcore-interop-full.json"))?;
    let fixture_map: FixtureMap = load_fixture(&fixture_root_dir.join("fixture_map.json"))?;

    let run_a =
        execute_fixture_run("run-a", &spec, &run_config, &trigger, &fixture_map, &reporter).await?;
    let run_b =
        execute_fixture_run("run-b", &spec, &run_config, &trigger, &fixture_map, &reporter).await?;

    if run_a.manifest.integrity.root_hash != run_b.manifest.integrity.root_hash {
        return Err(format!(
            "root hash mismatch: {} vs {}",
            run_a.manifest.integrity.root_hash.value, run_b.manifest.integrity.root_hash.value
        )
        .into());
    }
    if run_a.decision_outcome != run_b.decision_outcome {
        return Err(format!(
            "decision mismatch: {:?} vs {:?}",
            run_a.decision_outcome, run_b.decision_outcome
        )
        .into());
    }
    if run_a.status != run_b.status {
        return Err(format!("status mismatch: {:?} vs {:?}", run_a.status, run_b.status).into());
    }

    reporter.artifacts().write_json("run_a_manifest.json", &run_a.manifest)?;
    reporter.artifacts().write_json("run_b_manifest.json", &run_b.manifest)?;
    reporter.artifacts().write_json("run_a_decision.json", &run_a.decision_outcome)?;
    reporter.artifacts().write_json("run_b_decision.json", &run_b.decision_outcome)?;
    reporter
        .artifacts()
        .write_json("tool_transcript.json", &vec![run_a.transcript, run_b.transcript])?;
    reporter.finish(
        "pass",
        vec!["determinism verified across identical AssetCore fixtures".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "run_a_manifest.json".to_string(),
            "run_b_manifest.json".to_string(),
            "run_a_decision.json".to_string(),
            "run_b_decision.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[derive(Debug, Serialize)]
struct RunTranscript {
    label: String,
    transcript: Vec<helpers::mcp_client::TranscriptEntry>,
}

struct RunOutcome {
    manifest: RunpackManifest,
    decision_outcome: DecisionOutcome,
    status: RunStatus,
    transcript: RunTranscript,
}

async fn execute_fixture_run(
    label: &str,
    spec: &ScenarioSpec,
    run_config: &RunConfig,
    trigger: &TriggerEvent,
    fixture_map: &FixtureMap,
    reporter: &TestReporter,
) -> Result<RunOutcome, Box<dyn std::error::Error>> {
    let fixtures = build_fixtures(fixture_map);
    let provider = spawn_provider_fixture_stub(fixtures).await?;
    let bind = allocate_bind_addr()?.to_string();
    let provider_contract = fixture_root("assetcore/providers").join("assetcore_read.json");
    let mut config =
        config_with_provider(&bind, ASSETCORE_PROVIDER_ID, provider.base_url(), &provider_contract);
    config.anchors.providers.push(assetcore_anchor_policy());

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at: trigger.time,
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
    let trigger_result: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id,
        request: decision_gate_core::runtime::StatusRequest {
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            run_id: run_config.run_id.clone(),
            requested_at: trigger.time,
            correlation_id: trigger.correlation_id.clone(),
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status: decision_gate_core::runtime::ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir().join(label);
    fs::create_dir_all(&runpack_dir)?;
    let export_request = RunpackExportRequest {
        scenario_id: run_config.scenario_id.clone(),
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let manifest_path = runpack_dir.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;

    Ok(RunOutcome {
        manifest,
        decision_outcome: trigger_result.decision.outcome,
        status: status.status,
        transcript: RunTranscript {
            label: label.to_string(),
            transcript: client.transcript(),
        },
    })
}

fn build_fixtures(fixture_map: &FixtureMap) -> Vec<ProviderFixture> {
    let namespace_id = fixture_map.assetcore_namespace_id.unwrap_or(0);
    let commit_id = fixture_map.fixture_version.clone().unwrap_or_else(|| "fixture".to_string());
    fixture_map
        .fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            let world_seq = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
            let anchor_value = json!({
                "assetcore.namespace_id": namespace_id,
                "assetcore.commit_id": commit_id,
                "assetcore.world_seq": world_seq
            });
            ProviderFixture {
                check_id: fixture.check_id.clone(),
                params: fixture.params.clone(),
                result: fixture.expected.clone(),
                anchor: Some(decision_gate_core::EvidenceAnchor {
                    anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
                    anchor_value: serde_json::to_string(&anchor_value)
                        .unwrap_or_else(|_| "{}".to_string()),
                }),
            }
        })
        .collect()
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

#[derive(Debug, Deserialize)]
struct FixtureMap {
    #[serde(default)]
    assetcore_namespace_id: Option<u64>,
    #[serde(default)]
    fixture_version: Option<String>,
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize)]
struct FixtureEntry {
    check_id: String,
    params: Value,
    expected: Value,
}
