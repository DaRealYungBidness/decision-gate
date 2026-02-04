// system-tests/tests/suites/runpack.rs
// ============================================================================
// Module: Runpack Tests
// Description: Runpack export and verification coverage.
// Purpose: Ensure runpack integrity checks are enforced.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Runpack export and verification coverage.
//! Purpose: Ensure runpack integrity checks are enforced.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use decision_gate_core::ArtifactReader;
use decision_gate_core::RunpackManifest;
use decision_gate_core::RunpackVerifier;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_mcp::config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_mcp::config::NamespaceAuthorityMode;
use decision_gate_mcp::config::ObjectStoreConfig;
use decision_gate_mcp::config::ObjectStoreProvider;
use decision_gate_mcp::config::RunpackStorageConfig;
use decision_gate_mcp::runpack::FileArtifactReader;
use decision_gate_mcp::runpack_object_store::ObjectStoreRunpackBackend;
use decision_gate_mcp::runpack_object_store::RunpackObjectKey;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackVerifyRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::env::set_var as set_env;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::infra::S3Fixture;
use helpers::namespace_authority_stub::spawn_namespace_authority_stub;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

use crate::helpers;

static RUNPACK_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
const BACKCOMPAT_ROOT: &str = "tests/fixtures/runpacks/backcompat/v1";
const BACKCOMPAT_MANIFEST: &str = "manifest.json";

async fn lock_runpack_mutex() -> tokio::sync::MutexGuard<'static, ()> {
    RUNPACK_TEST_MUTEX.lock().await
}

#[test]
fn runpack_backcompat_v1_fixture_verifies() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_backcompat_v1_fixture_verifies")?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BACKCOMPAT_ROOT);
    let manifest_path = root.join(BACKCOMPAT_MANIFEST);
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|_| format!("missing backcompat manifest at {}", manifest_path.display()))?;
    let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;
    let reader = FileArtifactReader::new(root)?;
    let verifier = RunpackVerifier::new(manifest.hash_algorithm);
    let report = verifier.verify_manifest(&reader, &manifest)?;
    if report.status != decision_gate_core::runtime::VerificationStatus::Pass {
        return Err(format!("expected verification pass, got {:?}", report.status).into());
    }
    reporter.finish(
        "pass",
        vec!["backcompat v1 runpack verified".to_string()],
        vec!["summary.json".to_string(), "summary.md".to_string()],
    )?;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn runpack_export_verify_happy_path() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_runpack_mutex().await;
    let mut reporter = TestReporter::new("runpack_export_verify_happy_path")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("runpack-scenario", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verified: decision_gate_mcp::tools::RunpackVerifyResponse =
        client.call_tool_typed("runpack_verify", verify_input).await?;

    if verified.status != decision_gate_core::runtime::VerificationStatus::Pass {
        return Err(format!("expected verification pass, got {:?}", verified.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["runpack verification passed".to_string()],
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

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Object-store roundtrip flow kept in one sequence.")]
async fn runpack_export_object_store_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_runpack_mutex().await;
    let mut reporter = TestReporter::new("runpack_export_object_store_roundtrip")?;

    let s3 = match S3Fixture::start().await {
        Ok(fixture) => fixture,
        Err(err) => {
            if err.contains("docker info failed") {
                reporter.finish(
                    "skip",
                    vec![format!("object store fixture unavailable: {err}")],
                    vec!["summary.json".to_string(), "summary.md".to_string()],
                )?;
                drop(reporter);
                return Ok(());
            }
            return Err(err.into());
        }
    };
    set_env("AWS_EC2_METADATA_DISABLED", "true");
    set_env("AWS_ACCESS_KEY_ID", &s3.access_key);
    set_env("AWS_SECRET_ACCESS_KEY", &s3.secret_key);
    set_env("AWS_REGION", &s3.region);

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: s3.bucket.clone(),
        region: Some(s3.region.clone()),
        endpoint: Some(s3.endpoint.clone()),
        prefix: Some("dg/runpacks".to_string()),
        force_path_style: s3.force_path_style,
        allow_http: true,
    }));

    let server = spawn_mcp_server(config.clone()).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("runpack-object-store", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: None,
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: true,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;
    if exported.storage_uri.is_none() {
        return Err("expected storage_uri from object store export".into());
    }

    let object_store = ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: s3.bucket.clone(),
        region: Some(s3.region.clone()),
        endpoint: Some(s3.endpoint.clone()),
        prefix: Some("dg/runpacks".to_string()),
        force_path_style: s3.force_path_style,
        allow_http: true,
    };
    let backend = ObjectStoreRunpackBackend::new(&object_store)?;
    let spec_hash =
        fixture.spec.canonical_hash_with(decision_gate_core::hashing::HashAlgorithm::Sha256)?;
    let key = RunpackObjectKey {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        scenario_id: define_output.scenario_id.clone(),
        run_id: fixture.run_id.clone(),
        spec_hash,
    };
    let reader = backend.reader(&key)?;
    let manifest_bytes = reader.read_with_limit(
        "manifest.json",
        decision_gate_core::runtime::runpack::MAX_RUNPACK_ARTIFACT_BYTES,
    )?;
    let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;
    let verifier = RunpackVerifier::new(manifest.hash_algorithm);
    let report = verifier.verify_manifest(&reader, &manifest)?;
    if report.status != decision_gate_core::runtime::VerificationStatus::Pass {
        return Err(format!("expected verification pass, got {:?}", report.status).into());
    }

    let s3_client = s3.client().await?;
    let root_prefix = object_store.prefix.as_deref().unwrap_or("").trim_matches('/');
    let root_prefix =
        if root_prefix.is_empty() { String::new() } else { format!("{root_prefix}/") };
    let runpack_prefix = format!(
        "tenant/{}/namespace/{}/scenario/{}/run/{}/spec/{}/{}/",
        &fixture.tenant_id.to_string(),
        &fixture.namespace_id.to_string(),
        define_output.scenario_id.as_str(),
        fixture.run_id.as_str(),
        match manifest.spec_hash.algorithm {
            decision_gate_core::hashing::HashAlgorithm::Sha256 => "sha256",
        },
        manifest.spec_hash.value.as_str(),
    );
    let tamper_path = manifest
        .integrity
        .file_hashes
        .iter()
        .find(|entry| entry.path != "manifest.json")
        .map(|entry| entry.path.clone())
        .ok_or("no artifacts to tamper")?;
    let object_key = format!("{root_prefix}{runpack_prefix}{tamper_path}");
    let output = s3_client.get_object().bucket(&s3.bucket).key(&object_key).send().await?;
    let mut stream = output.body.into_async_read();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await?;
    bytes.extend_from_slice(b"tampered");
    s3_client
        .put_object()
        .bucket(&s3.bucket)
        .key(&object_key)
        .body(aws_sdk_s3::primitives::ByteStream::from(bytes))
        .send()
        .await?;

    let report = verifier.verify_manifest(&backend.reader(&key)?, &manifest)?;
    if report.status != decision_gate_core::runtime::VerificationStatus::Fail {
        return Err(format!("expected verification fail, got {:?}", report.status).into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["object-store runpack verification passed and tamper detected".to_string()],
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
async fn runpack_tamper_detection() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_runpack_mutex().await;
    let mut reporter = TestReporter::new("runpack_tamper_detection")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("runpack-tamper", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let tamper_path = runpack_dir.join("artifacts/triggers.json");
    let mut bytes = std::fs::read(&tamper_path)?;
    bytes.extend_from_slice(b"tampered");
    std::fs::write(&tamper_path, bytes)?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verified: decision_gate_mcp::tools::RunpackVerifyResponse =
        client.call_tool_typed("runpack_verify", verify_input).await?;

    if verified.status != decision_gate_core::runtime::VerificationStatus::Fail {
        return Err(format!("expected verification fail, got {:?}", verified.status).into());
    }
    if verified.report.errors.is_empty() {
        return Err("expected verification errors after tampering".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["tampered runpack rejected".to_string()],
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

#[tokio::test(flavor = "multi_thread")]
async fn runpack_missing_manifest_fails() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_runpack_mutex().await;
    let mut reporter = TestReporter::new("runpack_missing_manifest_fails")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("runpack-missing-manifest", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    std::fs::remove_file(runpack_dir.join("manifest.json"))?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let result = client.call_tool_typed::<decision_gate_mcp::tools::RunpackVerifyResponse>(
        "runpack_verify",
        verify_input,
    );
    if result.await.is_ok() {
        return Err("expected runpack verification to fail when manifest is missing".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["missing manifest rejected".to_string()],
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

#[tokio::test(flavor = "multi_thread")]
async fn runpack_missing_artifact_fails() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_runpack_mutex().await;
    let mut reporter = TestReporter::new("runpack_missing_artifact_fails")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("runpack-missing-artifact", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

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
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "runpack".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let _trigger: decision_gate_core::runtime::TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let export_request = RunpackExportRequest {
        scenario_id: define_output.scenario_id.clone(),
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        run_id: fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(3),
        include_verification: false,
    };
    let export_input = serde_json::to_value(&export_request)?;
    let _exported: decision_gate_mcp::tools::RunpackExportResponse =
        client.call_tool_typed("runpack_export", export_input).await?;

    let manifest_bytes = std::fs::read(runpack_dir.join("manifest.json"))?;
    let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;
    let missing_path = manifest
        .integrity
        .file_hashes
        .iter()
        .map(|entry| entry.path.as_str())
        .find(|path| *path != "manifest.json")
        .ok_or("no artifacts available to remove")?;
    std::fs::remove_file(runpack_dir.join(missing_path))?;

    let verify_request = RunpackVerifyRequest {
        runpack_dir: runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let verify_input = serde_json::to_value(&verify_request)?;
    let verified: decision_gate_mcp::tools::RunpackVerifyResponse =
        client.call_tool_typed("runpack_verify", verify_input).await?;

    if verified.status != decision_gate_core::runtime::VerificationStatus::Fail {
        return Err(format!("expected verification fail, got {:?}", verified.status).into());
    }
    if !verified.report.errors.iter().any(|err| err.contains("artifact read failed")) {
        return Err("expected missing artifact error in verification report".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["missing artifact rejected".to_string()],
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

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Security context checks cover two configurations.")]
async fn runpack_export_includes_security_context() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = lock_runpack_mutex().await;
    let mut reporter = TestReporter::new("runpack_export_includes_security_context")?;
    let mut transcripts = Vec::new();

    // Case 1: dev-permissive enabled with default namespace authority.
    {
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.dev.permissive = true;
        let server = spawn_mcp_server(config).await?;
        let client = server.client(std::time::Duration::from_secs(5))?;
        wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

        let mut fixture = ScenarioFixture::time_after("runpack-security-dev", "run-1", 0);
        fixture.spec.default_tenant_id = Some(fixture.tenant_id);
        let define_request = ScenarioDefineRequest {
            spec: fixture.spec.clone(),
        };
        let define_output: ScenarioDefineResponse = client
            .call_tool_typed("scenario_define", serde_json::to_value(&define_request)?)
            .await?;

        let start_request = ScenarioStartRequest {
            scenario_id: define_output.scenario_id.clone(),
            run_config: fixture.run_config(),
            started_at: Timestamp::Logical(1),
            issue_entry_packets: false,
        };
        client
            .call_tool_typed::<decision_gate_core::RunState>(
                "scenario_start",
                serde_json::to_value(&start_request)?,
            )
            .await?;

        let trigger_request = ScenarioTriggerRequest {
            scenario_id: define_output.scenario_id.clone(),
            trigger: decision_gate_core::TriggerEvent {
                run_id: fixture.run_id.clone(),
                tenant_id: fixture.tenant_id,
                namespace_id: fixture.namespace_id,
                trigger_id: TriggerId::new("trigger-1"),
                kind: TriggerKind::ExternalEvent,
                time: Timestamp::Logical(2),
                source_id: "runpack".to_string(),
                payload: None,
                correlation_id: None,
            },
        };
        client
            .call_tool_typed::<decision_gate_core::runtime::TriggerResult>(
                "scenario_trigger",
                serde_json::to_value(&trigger_request)?,
            )
            .await?;

        let runpack_dir = reporter.artifacts().runpack_dir().join("dev");
        std::fs::create_dir_all(&runpack_dir)?;
        let export_request = RunpackExportRequest {
            scenario_id: define_output.scenario_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            run_id: fixture.run_id.clone(),
            output_dir: Some(runpack_dir.to_string_lossy().to_string()),
            manifest_name: Some("manifest.json".to_string()),
            generated_at: Timestamp::Logical(3),
            include_verification: false,
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::RunpackExportResponse>(
                "runpack_export",
                serde_json::to_value(&export_request)?,
            )
            .await?;

        let manifest_bytes = std::fs::read(runpack_dir.join("manifest.json"))?;
        let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;
        let security = manifest.security.ok_or("missing runpack security context")?;
        if !security.dev_permissive {
            return Err("expected dev_permissive=true in runpack security context".into());
        }
        if security.namespace_authority != "dg_registry" {
            return Err("unexpected namespace_authority in runpack security context".into());
        }

        transcripts.extend(client.transcript());
        server.shutdown().await;
    }

    // Case 2: AssetCore authority emits security context metadata.
    {
        let authority = spawn_namespace_authority_stub(vec![1]).await?;
        let bind = allocate_bind_addr()?.to_string();
        let mut config = base_http_config(&bind);
        config.namespace.authority.mode = NamespaceAuthorityMode::AssetcoreHttp;
        config.namespace.authority.assetcore = Some(AssetCoreNamespaceAuthorityConfig {
            base_url: authority.base_url().to_string(),
            auth_token: None,
            connect_timeout_ms: 500,
            request_timeout_ms: 1_000,
        });

        let server = spawn_mcp_server(config).await?;
        let client = server.client(std::time::Duration::from_secs(5))?;
        wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

        let mut fixture = ScenarioFixture::time_after("runpack-security-assetcore", "run-1", 0);
        fixture.spec.default_tenant_id = Some(fixture.tenant_id);
        let define_request = ScenarioDefineRequest {
            spec: fixture.spec.clone(),
        };
        let define_output: ScenarioDefineResponse = client
            .call_tool_typed("scenario_define", serde_json::to_value(&define_request)?)
            .await?;

        let start_request = ScenarioStartRequest {
            scenario_id: define_output.scenario_id.clone(),
            run_config: fixture.run_config(),
            started_at: Timestamp::Logical(1),
            issue_entry_packets: false,
        };
        client
            .call_tool_typed::<decision_gate_core::RunState>(
                "scenario_start",
                serde_json::to_value(&start_request)?,
            )
            .await?;

        let trigger_request = ScenarioTriggerRequest {
            scenario_id: define_output.scenario_id.clone(),
            trigger: decision_gate_core::TriggerEvent {
                run_id: fixture.run_id.clone(),
                tenant_id: fixture.tenant_id,
                namespace_id: fixture.namespace_id,
                trigger_id: TriggerId::new("trigger-1"),
                kind: TriggerKind::ExternalEvent,
                time: Timestamp::Logical(2),
                source_id: "runpack".to_string(),
                payload: None,
                correlation_id: None,
            },
        };
        client
            .call_tool_typed::<decision_gate_core::runtime::TriggerResult>(
                "scenario_trigger",
                serde_json::to_value(&trigger_request)?,
            )
            .await?;

        let runpack_dir = reporter.artifacts().runpack_dir().join("assetcore");
        std::fs::create_dir_all(&runpack_dir)?;
        let export_request = RunpackExportRequest {
            scenario_id: define_output.scenario_id.clone(),
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            run_id: fixture.run_id.clone(),
            output_dir: Some(runpack_dir.to_string_lossy().to_string()),
            manifest_name: Some("manifest.json".to_string()),
            generated_at: Timestamp::Logical(3),
            include_verification: false,
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::RunpackExportResponse>(
                "runpack_export",
                serde_json::to_value(&export_request)?,
            )
            .await?;

        let manifest_bytes = std::fs::read(runpack_dir.join("manifest.json"))?;
        let manifest: RunpackManifest = serde_json::from_slice(&manifest_bytes)?;
        let security = manifest.security.ok_or("missing runpack security context")?;
        if security.dev_permissive {
            return Err("expected dev_permissive=false for assetcore runpack".into());
        }
        if security.namespace_authority != "assetcore_catalog" {
            return Err("unexpected namespace_authority for assetcore runpack".into());
        }

        transcripts.extend(client.transcript());
        server.shutdown().await;
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["runpack manifests include security context".to_string()],
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
