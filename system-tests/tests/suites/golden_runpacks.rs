// system-tests/tests/suites/golden_runpacks.rs
// ============================================================================
// Module: Golden Runpack Tests
// Description: Cross-OS determinism checks against committed golden runpacks.
// Purpose: Enforce bit-for-bit deterministic runpack exports.
// Dependencies: system-tests helpers, decision-gate-mcp, decision-gate-core
// ============================================================================

//! Golden runpack determinism tests for Decision Gate system-tests.


use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::RunpackManifest;
use decision_gate_core::Timestamp;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackVerifyRequest;
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

const GOLDEN_ROOT: &str = "tests/fixtures/runpacks/golden";
const MANIFEST_NAME: &str = "manifest.json";

#[tokio::test(flavor = "multi_thread")]
async fn golden_runpack_cross_os() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("golden_runpack_cross_os")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let scenarios = vec![
        {
            let mut fixture = ScenarioFixture::time_after("golden-time-after", "run-1", 0);
            fixture.spec.default_tenant_id = Some(fixture.tenant_id.clone());
            GoldenScenario::new("golden_time_after_pass", fixture, false)
        },
        {
            let mut fixture = ScenarioFixture::with_visibility_packet(
                "golden-visibility",
                "run-2",
                vec!["confidential".to_string(), "restricted".to_string()],
                vec!["policy-alpha".to_string()],
            );
            fixture.spec.default_tenant_id = Some(fixture.tenant_id.clone());
            GoldenScenario::new("golden_visibility_packet", fixture, true)
        },
    ];

    let mut notes = Vec::new();
    for scenario in scenarios {
        let runpack_dir = export_runpack(&client, &scenario).await?;
        let golden_dir = golden_dir(&scenario.label);

        if update_golden()? {
            sync_golden(&runpack_dir, &golden_dir)?;
            notes.push(format!("updated golden runpack: {}", scenario.label));
        } else {
            compare_runpacks(&golden_dir, &runpack_dir)?;
            notes.push(format!("verified golden runpack: {}", scenario.label));
        }
    }

    reporter.finish("pass", notes, vec!["summary.json".to_string(), "summary.md".to_string()])?;
    Ok(())
}

struct GoldenScenario {
    label: String,
    fixture: ScenarioFixture,
    issue_entry_packets: bool,
}

impl GoldenScenario {
    fn new(label: &str, fixture: ScenarioFixture, issue_entry_packets: bool) -> Self {
        Self {
            label: label.to_string(),
            fixture,
            issue_entry_packets,
        }
    }
}

async fn export_runpack(
    client: &helpers::mcp_client::McpHttpClient,
    scenario: &GoldenScenario,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let define_request = ScenarioDefineRequest {
        spec: scenario.fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: scenario.fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: scenario.issue_entry_packets,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: scenario.fixture.trigger_event("trigger-1", Timestamp::Logical(2)),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let temp = tempdir()?;
    let runpack_dir = temp.path().to_path_buf();
    let export_request = RunpackExportRequest {
        scenario_id: scenario.fixture.spec.scenario_id.clone(),
        tenant_id: scenario.fixture.tenant_id.clone(),
        namespace_id: scenario.fixture.namespace_id.clone(),
        run_id: scenario.fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some(MANIFEST_NAME.to_string()),
        generated_at: Timestamp::Logical(10),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: MANIFEST_NAME.to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verified: decision_gate_mcp::tools::RunpackVerifyResponse =
        client.call_tool_typed("runpack_verify", verify_input).await?;
    if verified.status != decision_gate_core::runtime::VerificationStatus::Pass {
        return Err(format!("expected verification pass, got {:?}", verified.status).into());
    }

    // Persist the temp directory by keeping it alive until the caller copies it.
    Ok(temp.keep())
}

fn golden_dir(label: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_ROOT).join(label)
}

fn update_golden() -> Result<bool, Box<dyn std::error::Error>> {
    Ok(env::var("UPDATE_GOLDEN_RUNPACKS").is_ok())
}

fn sync_golden(source: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if dest.exists() {
        fs::remove_dir_all(dest)?;
    }
    copy_dir_recursive(source, dest)?;
    Ok(())
}

fn compare_runpacks(golden: &Path, candidate: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let golden_manifest_path = golden.join(MANIFEST_NAME);
    let candidate_manifest_path = candidate.join(MANIFEST_NAME);

    let golden_manifest_bytes = fs::read(&golden_manifest_path).map_err(|_| {
        format!(
            "missing golden manifest at {} (set UPDATE_GOLDEN_RUNPACKS=1 to regenerate)",
            golden_manifest_path.display()
        )
    })?;
    let candidate_manifest_bytes = fs::read(&candidate_manifest_path)?;

    if golden_manifest_bytes != candidate_manifest_bytes {
        let golden_manifest: RunpackManifest = serde_json::from_slice(&golden_manifest_bytes)?;
        let candidate_manifest: RunpackManifest =
            serde_json::from_slice(&candidate_manifest_bytes)?;
        return Err(format!(
            "manifest mismatch (golden root hash={}, candidate root hash={})",
            golden_manifest.integrity.root_hash.value, candidate_manifest.integrity.root_hash.value
        )
        .into());
    }

    let manifest: RunpackManifest = serde_json::from_slice(&golden_manifest_bytes)?;
    let mut expected_files: BTreeSet<String> =
        manifest.integrity.file_hashes.iter().map(|entry| entry.path.clone()).collect();
    expected_files.insert(MANIFEST_NAME.to_string());

    for path in &expected_files {
        let golden_path = golden.join(path);
        let candidate_path = candidate.join(path);
        let golden_bytes = fs::read(&golden_path)?;
        let candidate_bytes = fs::read(&candidate_path)?;
        if golden_bytes != candidate_bytes {
            return Err(format!("artifact mismatch: {}", path).into());
        }
    }

    let mut actual_files = BTreeSet::new();
    collect_files(candidate, candidate, &mut actual_files)?;
    if actual_files != expected_files {
        return Err(format!(
            "candidate runpack file set mismatch (expected={:?}, actual={:?})",
            expected_files, actual_files
        )
        .into());
    }

    Ok(())
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let dest_path = dest.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| "runpack path prefix mismatch")?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(rel);
        }
    }
    Ok(())
}
