// system-tests/tests/suites/sdk_client.rs
// ============================================================================
// Module: SDK Client Tests
// Description: End-to-end SDK validation for Python and TypeScript clients.
// Purpose: Ensure SDK transports can execute core scenario lifecycle calls.
// Dependencies: system-tests helpers, decision-gate-core, tokio
// ============================================================================

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "Test suite helpers keep documentation concise."
)]

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::Timestamp;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::base_http_config_with_bearer;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::sdk_runner;

use crate::helpers;

const PYTHON_SCRIPT: &str = "tests/fixtures/sdk_client_python.py";
const TYPESCRIPT_SCRIPT: &str = "tests/fixtures/sdk_client_typescript.ts";

#[tokio::test(flavor = "multi_thread")]
async fn python_sdk_http_scenario_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("python_sdk_http_scenario_lifecycle")?;
    let runtime = match sdk_runner::python_runtime() {
        Ok(runtime) => runtime,
        Err(reason) => {
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };

    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("python-sdk-scenario", "run-python-1", 0);
    let mut spec = fixture.spec.clone();
    spec.default_tenant_id = Some(fixture.tenant_id);
    let run_config = fixture.run_config();

    let output = run_sdk_script(
        &runtime.path,
        &fixture_path(PYTHON_SCRIPT),
        &bind,
        None,
        &spec,
        &run_config,
        false,
    )
    .await?;

    reporter.artifacts().write_text("sdk.stdout.log", &output.stdout)?;
    reporter.artifacts().write_text("sdk.stderr.log", &output.stderr)?;

    if !output.status.success() {
        reporter.finish(
            "fail",
            vec![format!("python sdk script failed: {}", output.status)],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "sdk.stdout.log".to_string(),
                "sdk.stderr.log".to_string(),
            ],
        )?;
        return Err("python sdk script failed".into());
    }

    let payload: serde_json::Value = serde_json::from_str(output.stdout.trim())?;
    let define =
        payload.get("define").and_then(|value| value.as_object()).ok_or("missing define output")?;
    if define.get("scenario_id").is_none() {
        return Err("define response missing scenario_id".into());
    }

    reporter.finish(
        "pass",
        vec!["python sdk scenario lifecycle succeeded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "sdk.stdout.log".to_string(),
            "sdk.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn python_sdk_bearer_auth_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("python_sdk_bearer_auth_enforced")?;
    let runtime = match sdk_runner::python_runtime() {
        Ok(runtime) => runtime,
        Err(reason) => {
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };

    let bind = allocate_bind_addr()?.to_string();
    let token = "sdk-token-1";
    let config = base_http_config_with_bearer(&bind, token);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.to_string());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("python-sdk-auth", "run-python-2", 0);
    let mut spec = fixture.spec.clone();
    spec.default_tenant_id = Some(fixture.tenant_id);
    let run_config = fixture.run_config();

    let success = run_sdk_script(
        &runtime.path,
        &fixture_path(PYTHON_SCRIPT),
        &bind,
        Some(token),
        &spec,
        &run_config,
        false,
    )
    .await?;

    let failure = run_sdk_script(
        &runtime.path,
        &fixture_path(PYTHON_SCRIPT),
        &bind,
        None,
        &spec,
        &run_config,
        true,
    )
    .await?;

    reporter.artifacts().write_text("sdk.success.stdout.log", &success.stdout)?;
    reporter.artifacts().write_text("sdk.success.stderr.log", &success.stderr)?;
    reporter.artifacts().write_text("sdk.failure.stdout.log", &failure.stdout)?;
    reporter.artifacts().write_text("sdk.failure.stderr.log", &failure.stderr)?;

    if !success.status.success() {
        reporter.finish(
            "fail",
            vec![format!("python sdk success path failed: {}", success.status)],
            artifact_list_auth(),
        )?;
        return Err("python sdk success path failed".into());
    }
    if !failure.status.success() {
        reporter.finish(
            "fail",
            vec![format!("python sdk failure path did not exit cleanly: {}", failure.status)],
            artifact_list_auth(),
        )?;
        return Err("python sdk failure path failed".into());
    }

    reporter.finish(
        "pass",
        vec!["python sdk bearer auth enforced".to_string()],
        artifact_list_auth(),
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn typescript_sdk_http_scenario_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("typescript_sdk_http_scenario_lifecycle")?;
    let runtime = match sdk_runner::node_runtime_for_typescript() {
        Ok(runtime) => runtime,
        Err(reason) => {
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };

    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("ts-sdk-scenario", "run-ts-1", 0);
    let mut spec = fixture.spec.clone();
    spec.default_tenant_id = Some(fixture.tenant_id);
    let run_config = fixture.run_config();

    let output = run_sdk_script(
        &runtime.path,
        &fixture_path(TYPESCRIPT_SCRIPT),
        &bind,
        None,
        &spec,
        &run_config,
        false,
    )
    .await?;

    reporter.artifacts().write_text("sdk.stdout.log", &output.stdout)?;
    reporter.artifacts().write_text("sdk.stderr.log", &output.stderr)?;

    if !output.status.success() {
        reporter.finish(
            "fail",
            vec![format!("typescript sdk script failed: {}", output.status)],
            vec![
                "summary.json".to_string(),
                "summary.md".to_string(),
                "sdk.stdout.log".to_string(),
                "sdk.stderr.log".to_string(),
            ],
        )?;
        return Err("typescript sdk script failed".into());
    }

    let payload: serde_json::Value = serde_json::from_str(output.stdout.trim())?;
    let define =
        payload.get("define").and_then(|value| value.as_object()).ok_or("missing define output")?;
    if define.get("scenario_id").is_none() {
        return Err("define response missing scenario_id".into());
    }

    reporter.finish(
        "pass",
        vec!["typescript sdk scenario lifecycle succeeded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "sdk.stdout.log".to_string(),
            "sdk.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn typescript_sdk_bearer_auth_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("typescript_sdk_bearer_auth_enforced")?;
    let runtime = match sdk_runner::node_runtime_for_typescript() {
        Ok(runtime) => runtime,
        Err(reason) => {
            reporter.finish(
                "skip",
                vec![reason],
                vec!["summary.json".to_string(), "summary.md".to_string()],
            )?;
            drop(reporter);
            return Ok(());
        }
    };

    let bind = allocate_bind_addr()?.to_string();
    let token = "sdk-token-2";
    let config = base_http_config_with_bearer(&bind, token);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?.with_bearer_token(token.to_string());
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let fixture = ScenarioFixture::time_after("ts-sdk-auth", "run-ts-2", 0);
    let mut spec = fixture.spec.clone();
    spec.default_tenant_id = Some(fixture.tenant_id);
    let run_config = fixture.run_config();

    let success = run_sdk_script(
        &runtime.path,
        &fixture_path(TYPESCRIPT_SCRIPT),
        &bind,
        Some(token),
        &spec,
        &run_config,
        false,
    )
    .await?;

    let failure = run_sdk_script(
        &runtime.path,
        &fixture_path(TYPESCRIPT_SCRIPT),
        &bind,
        None,
        &spec,
        &run_config,
        true,
    )
    .await?;

    reporter.artifacts().write_text("sdk.success.stdout.log", &success.stdout)?;
    reporter.artifacts().write_text("sdk.success.stderr.log", &success.stderr)?;
    reporter.artifacts().write_text("sdk.failure.stdout.log", &failure.stdout)?;
    reporter.artifacts().write_text("sdk.failure.stderr.log", &failure.stderr)?;

    if !success.status.success() {
        reporter.finish(
            "fail",
            vec![format!("typescript sdk success path failed: {}", success.status)],
            artifact_list_auth(),
        )?;
        return Err("typescript sdk success path failed".into());
    }
    if !failure.status.success() {
        reporter.finish(
            "fail",
            vec![format!("typescript sdk failure path did not exit cleanly: {}", failure.status)],
            artifact_list_auth(),
        )?;
        return Err("typescript sdk failure path failed".into());
    }

    reporter.finish(
        "pass",
        vec!["typescript sdk bearer auth enforced".to_string()],
        artifact_list_auth(),
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}

async fn run_sdk_script(
    interpreter: &Path,
    script: &Path,
    bind: &str,
    token: Option<&str>,
    spec: &decision_gate_core::ScenarioSpec,
    run_config: &decision_gate_core::RunConfig,
    expect_failure: bool,
) -> Result<sdk_runner::ScriptOutput, Box<dyn std::error::Error>> {
    let mut envs = HashMap::new();
    envs.insert("DG_ENDPOINT".to_string(), format!("http://{bind}/rpc"));
    if let Some(token) = token {
        envs.insert("DG_TOKEN".to_string(), token.to_string());
    }
    if expect_failure {
        envs.insert("DG_EXPECT_FAILURE".to_string(), "1".to_string());
    }
    envs.insert("DG_SCENARIO_SPEC".to_string(), serde_json::to_string(spec)?);
    envs.insert("DG_RUN_CONFIG".to_string(), serde_json::to_string(run_config)?);
    envs.insert("DG_STARTED_AT".to_string(), serde_json::to_string(&Timestamp::Logical(1))?);
    let script = script
        .canonicalize()
        .map_err(|err| format!("fixture path missing: {} ({err})", script.display()))?;
    if script.extension().and_then(|ext| ext.to_str()) == Some("ts") {
        let args = vec!["--experimental-strip-types".to_string(), script.display().to_string()];
        let loader_path = fixture_path("tests/fixtures/ts_loader.mjs");
        let loader_path = loader_path.canonicalize().ok();
        let node_options = sdk_runner::node_options_with_loader(
            std::env::var("NODE_OPTIONS").ok(),
            loader_path.as_deref(),
        );
        envs.insert("NODE_OPTIONS".to_string(), node_options);
        return Ok(
            sdk_runner::run_script(interpreter, &args, &envs, Duration::from_secs(20)).await?
        );
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("missing workspace root")?
        .to_path_buf();
    let mut paths = Vec::new();
    paths.push(workspace_root.join("sdks/python"));
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let joined =
        std::env::join_paths(paths).map_err(|err| format!("pythonpath join failed: {err}"))?;
    envs.insert("PYTHONPATH".to_string(), joined.to_string_lossy().to_string());
    let args = vec![script.display().to_string()];
    Ok(sdk_runner::run_script(interpreter, &args, &envs, Duration::from_secs(20)).await?)
}

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn artifact_list_auth() -> Vec<String> {
    vec![
        "summary.json".to_string(),
        "summary.md".to_string(),
        "sdk.success.stdout.log".to_string(),
        "sdk.success.stderr.log".to_string(),
        "sdk.failure.stdout.log".to_string(),
        "sdk.failure.stderr.log".to_string(),
    ]
}
