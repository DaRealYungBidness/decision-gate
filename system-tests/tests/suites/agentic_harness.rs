// system-tests/tests/suites/agentic_harness.rs
// ============================================================================
// Module: Agentic Flow Harness
// Description: Deterministic agentic scenario execution across projections.
// Purpose: Validate cross-driver invariance and audit artifacts.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Agentic flow harness for deterministic scenario execution.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::core::hashing::hash_bytes;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::policy::PolicyEffect;
use decision_gate_mcp::policy::PolicyEngine;
use decision_gate_mcp::policy::PolicyRule;
use decision_gate_mcp::policy::StaticPolicyConfig;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;
use helpers::sdk_runner;
use helpers::sdk_runner::node_runtime_for_typescript;
use helpers::sdk_runner::python_runtime;
use helpers::sdk_runner::run_script;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::oneshot;
use toml::Value as TomlValue;
use toml::value::Table;

use crate::helpers;

const FIXTURE_ROOT: &str = "tests/fixtures/agentic";
const REGISTRY_PATH: &str = "tests/fixtures/agentic/scenario_registry.toml";
const EXPECTED_HASH_FILE: &str = "expected/runpack_root_hash.txt";
const EXPECTED_HASH_PREFIX: &str = "expected/runpack_root_hash.";
const HTTP_PLACEHOLDER: &str = "{{HTTP_BASE_URL}}";

type DynError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Deserialize)]
struct ScenarioRegistry {
    scenarios: Vec<ScenarioEntry>,
}

#[derive(Debug, Deserialize)]
struct ScenarioEntry {
    id: String,
    #[allow(
        dead_code,
        reason = "Description is captured for registry context but unused in tests."
    )]
    description: Option<String>,
    providers: Vec<String>,
    drivers: Vec<String>,
    modes: Vec<String>,
    expected_status: String,
    expected_outcome: Option<String>,
    policy_mode: Option<String>,
}

#[derive(Debug, Serialize)]
struct DriverResult {
    driver: String,
    status: String,
    outcome: Option<String>,
    runpack_root_hash: Option<String>,
    runpack_hash_algorithm: Option<String>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ScenarioResult {
    scenario_id: String,
    expected_status: String,
    expected_outcome: Option<String>,
    drivers: Vec<DriverResult>,
    baseline_root_hash: Option<String>,
}

#[derive(Debug)]
struct ScenarioPack {
    id: String,
    root: PathBuf,
    fixtures_dir: PathBuf,
    spec: Value,
    run_config: Value,
    trigger: Value,
    env_overrides: BTreeMap<String, String>,
    expected_hash_path: PathBuf,
}

#[derive(Debug)]
struct HttpStubHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl HttpStubHandle {
    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for HttpStubHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn agentic_flow_harness_deterministic() -> Result<(), DynError> {
    let mut reporter = TestReporter::new("agentic_flow_harness_deterministic")?;
    let registry = load_registry()?;
    let scenario_filter = parse_filter("DECISION_GATE_AGENTIC_SCENARIOS");
    let driver_filter = parse_filter("DECISION_GATE_AGENTIC_DRIVERS");
    let update_expected = env::var("UPDATE_AGENTIC_EXPECTED").is_ok();

    let mut results = Vec::new();
    let mut notes = Vec::new();

    for scenario in registry.scenarios {
        if !scenario_filter.is_empty() && !scenario_filter.contains(&scenario.id) {
            continue;
        }
        if !scenario.modes.iter().any(|mode| mode == "deterministic") {
            continue;
        }

        let pack = load_scenario_pack(&scenario.id)?;
        let http_stub = if scenario.providers.iter().any(|p| p == "http") {
            Some(spawn_http_stub(&scenario.id).await?)
        } else {
            None
        };
        let http_base_url = http_stub.as_ref().map(|handle| handle.base_url().to_string());

        let mut scenario_results = ScenarioResult {
            scenario_id: scenario.id.clone(),
            expected_status: scenario.expected_status.clone(),
            expected_outcome: scenario.expected_outcome.clone(),
            drivers: Vec::new(),
            baseline_root_hash: None,
        };

        let raw_result =
            run_raw_mcp_driver(&reporter, &scenario, &pack, http_base_url.as_deref()).await?;
        scenario_results.baseline_root_hash = raw_result.runpack_root_hash.clone();
        scenario_results.drivers.push(raw_result);

        for driver in &scenario.drivers {
            if driver == "raw_mcp" {
                continue;
            }
            if !driver_filter.is_empty() && !driver_filter.contains(driver) {
                continue;
            }
            let result =
                run_script_driver(&reporter, driver, &scenario, &pack, http_base_url.as_deref())
                    .await?;
            scenario_results.drivers.push(result);
        }

        enforce_expectations(&scenario, &pack, &scenario_results, update_expected)?;
        notes.push(format!("scenario {}: {} drivers", scenario.id, scenario_results.drivers.len()));
        results.push(scenario_results);
        drop(http_stub);
    }

    reporter.artifacts().write_json("agentic_results.json", &results)?;

    reporter.finish(
        "pass",
        notes,
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "agentic_results.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

fn load_registry() -> Result<ScenarioRegistry, DynError> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(REGISTRY_PATH);
    let contents = fs::read_to_string(root)?;
    Ok(toml::from_str(&contents)?)
}

fn parse_filter(var: &str) -> HashSet<String> {
    env::var(var)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn load_scenario_pack(id: &str) -> Result<ScenarioPack, DynError> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ROOT).join(id);
    let fixtures_dir = root.join("fixtures");
    let spec = load_json(root.join("spec.json"))?;
    let run_config = load_json(root.join("run_config.json"))?;
    let trigger = load_json(root.join("trigger.json"))?;
    let env_overrides = load_env_overrides(&fixtures_dir.join("env.json"))?;
    let expected_hash_path = resolve_expected_hash_path(&root);

    Ok(ScenarioPack {
        id: id.to_string(),
        root,
        fixtures_dir,
        spec,
        run_config,
        trigger,
        env_overrides,
        expected_hash_path,
    })
}

fn resolve_expected_hash_path(root: &Path) -> PathBuf {
    let os = std::env::consts::OS;
    let candidate = root.join(format!("{EXPECTED_HASH_PREFIX}{os}.txt"));
    if candidate.exists() {
        return candidate;
    }
    root.join(EXPECTED_HASH_FILE)
}

fn load_json(path: PathBuf) -> Result<Value, DynError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn load_env_overrides(path: &Path) -> Result<BTreeMap<String, String>, DynError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let bytes = fs::read(path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let mut map = BTreeMap::new();
    if let Value::Object(obj) = value {
        for (key, value) in obj {
            if let Value::String(raw) = value {
                map.insert(key, raw);
            }
        }
    }
    Ok(map)
}

async fn spawn_http_stub(scenario_id: &str) -> Result<HttpStubHandle, DynError> {
    async fn artifact_handler() -> impl IntoResponse {
        (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            r#"{"artifact_id":"build-123","sha256":"sha256:deadbeef"}"#,
        )
    }

    async fn fetch_handler() -> impl IntoResponse {
        (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            r#"{"value":"ok"}"#,
        )
    }

    let app = Router::new()
        .route("/artifact.json", get(artifact_handler))
        .route("/fetch.json", get(fetch_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], http_stub_port(scenario_id)?));
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|err| {
        format!("failed to bind deterministic http stub for {scenario_id} on {addr}: {err}")
    })?;
    let base_url = format!("http://{addr}");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
    });
    Ok(HttpStubHandle {
        base_url,
        shutdown: Some(shutdown_tx),
    })
}

fn http_stub_port(scenario_id: &str) -> Result<u16, DynError> {
    if let Ok(value) = env::var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT") {
        let port: u16 = value.parse().map_err(|_| {
            std::io::Error::other("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT must be a valid u16")
        })?;
        if port == 0 {
            return Err(std::io::Error::other(
                "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT must be nonzero",
            )
            .into());
        }
        return Ok(port);
    }

    let base: u16 = env::var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(20000);
    let range: u16 = env::var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(20000);
    if base == 0 || range == 0 {
        return Err(std::io::Error::other(
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE and RANGE must be nonzero",
        )
        .into());
    }
    let max = u32::from(base) + u32::from(range);
    if max >= u32::from(u16::MAX) {
        return Err(
            std::io::Error::other("http stub port range exceeds available port space").into()
        );
    }

    let digest = hash_bytes(HashAlgorithm::Sha256, scenario_id.as_bytes());
    let seed = u32::from_str_radix(&digest.value[.. 8], 16).unwrap_or(0);
    let offset = seed % u32::from(range);
    let port = u32::from(base) + offset;
    let port = u16::try_from(port).map_err(|_| std::io::Error::other("http stub port overflow"))?;
    Ok(port)
}

async fn run_raw_mcp_driver(
    reporter: &TestReporter,
    scenario: &ScenarioEntry,
    pack: &ScenarioPack,
    http_base_url: Option<&str>,
) -> Result<DriverResult, DynError> {
    let (server, client) =
        spawn_scenario_server(pack, http_base_url, scenario.policy_mode.as_deref()).await?;
    let result = async {
        let spec = replace_http_placeholder(&pack.spec, http_base_url);
        let run_config = pack.run_config.clone();
        let trigger = pack.trigger.clone();

        let scenario_id = read_string(&run_config, "scenario_id")?;

        client.call_tool("scenario_define", serde_json::json!({"spec": spec})).await?;

        client
            .call_tool(
                "scenario_start",
                serde_json::json!({
                    "scenario_id": scenario_id,
                    "run_config": run_config,
                    "started_at": {"kind": "logical", "value": 1},
                    "issue_entry_packets": false,
                }),
            )
            .await?;

        client
            .call_tool(
                "scenario_trigger",
                serde_json::json!({"scenario_id": scenario_id, "trigger": trigger}),
            )
            .await?;

        let status = client
            .call_tool(
                "scenario_status",
                serde_json::json!({
                    "scenario_id": scenario_id,
                    "request": {
                        "tenant_id": read_u64(&run_config, "tenant_id")?,
                        "namespace_id": read_u64(&run_config, "namespace_id")?,
                        "run_id": read_string(&run_config, "run_id")?,
                        "requested_at": {"kind": "logical", "value": 3},
                        "correlation_id": null
                    }
                }),
            )
            .await?;

        let runpack_dir = allocate_runpack_dir(reporter, &pack.id, "raw_mcp")?;
        let export = client
            .call_tool(
                "runpack_export",
                serde_json::json!({
                    "scenario_id": scenario_id,
                    "run_id": read_string(&run_config, "run_id")?,
                    "tenant_id": read_u64(&run_config, "tenant_id")?,
                    "namespace_id": read_u64(&run_config, "namespace_id")?,
                    "output_dir": runpack_dir.to_string_lossy().to_string(),
                    "manifest_name": "manifest.json",
                    "generated_at": {"kind": "logical", "value": 10},
                    "include_verification": false,
                }),
            )
            .await?;

        reporter.artifacts().write_json(&format!("status_{}_raw_mcp.json", pack.id), &status)?;
        reporter.artifacts().write_json(
            &format!("tool_transcript_{}_raw_mcp.json", pack.id),
            &client.transcript(),
        )?;

        Ok(DriverResult {
            driver: "raw_mcp".to_string(),
            status: read_string(&status, "status")?,
            outcome: extract_outcome(&status),
            runpack_root_hash: read_root_hash(&export),
            runpack_hash_algorithm: read_root_hash_algorithm(&export),
            notes: Vec::new(),
        })
    }
    .await;

    server.shutdown().await;
    result
}

#[allow(clippy::too_many_lines, reason = "Driver orchestration is clearer as a single flow.")]
async fn run_script_driver(
    reporter: &TestReporter,
    driver: &str,
    scenario: &ScenarioEntry,
    pack: &ScenarioPack,
    http_base_url: Option<&str>,
) -> Result<DriverResult, DynError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = match driver {
        "python_sdk" => manifest_dir.join("tests/fixtures/agentic/drivers/sdk_python_driver.py"),
        "typescript_sdk" => {
            manifest_dir.join("tests/fixtures/agentic/drivers/sdk_typescript_driver.ts")
        }
        "langchain" => manifest_dir.join("tests/fixtures/agentic/drivers/adapter_langchain.py"),
        "crewai" => manifest_dir.join("tests/fixtures/agentic/drivers/adapter_crewai.py"),
        "autogen" => manifest_dir.join("tests/fixtures/agentic/drivers/adapter_autogen.py"),
        "openai_agents" => {
            manifest_dir.join("tests/fixtures/agentic/drivers/adapter_openai_agents.py")
        }
        _ => {
            return Ok(DriverResult {
                driver: driver.to_string(),
                status: "skipped".to_string(),
                outcome: None,
                runpack_root_hash: None,
                runpack_hash_algorithm: None,
                notes: vec!["unknown driver".to_string()],
            });
        }
    };

    let (server, _client) =
        spawn_scenario_server(pack, http_base_url, scenario.policy_mode.as_deref()).await?;
    let endpoint = server.base_url().to_string();
    let runpack_dir = allocate_runpack_dir(reporter, &pack.id, driver)?;

    let mut envs = HashMap::new();
    envs.insert("DG_ENDPOINT".to_string(), endpoint);
    envs.insert("DG_SCENARIO_PACK".to_string(), pack.root.to_string_lossy().to_string());
    envs.insert("DG_RUNPACK_DIR".to_string(), runpack_dir.to_string_lossy().to_string());
    if let Some(http_base) = http_base_url {
        envs.insert("DG_HTTP_BASE_URL".to_string(), http_base.to_string());
    }

    let (interpreter, args) = if driver == "typescript_sdk" {
        let runtime = match node_runtime_for_typescript() {
            Ok(runtime) => runtime,
            Err(err) => {
                return Ok(DriverResult {
                    driver: driver.to_string(),
                    status: "skipped".to_string(),
                    outcome: None,
                    runpack_root_hash: None,
                    runpack_hash_algorithm: None,
                    notes: vec![err],
                });
            }
        };
        let args =
            vec!["--experimental-strip-types".to_string(), script.to_string_lossy().to_string()];
        let loader_path = manifest_dir.join("tests/fixtures/ts_loader.mjs");
        let loader_path = loader_path.canonicalize().ok();
        let node_options = sdk_runner::node_options_with_loader(
            env::var("NODE_OPTIONS").ok(),
            loader_path.as_deref(),
        );
        envs.insert("NODE_OPTIONS".to_string(), node_options);
        (runtime.path, args)
    } else {
        let runtime = match python_runtime() {
            Ok(runtime) => runtime,
            Err(err) => {
                return Ok(DriverResult {
                    driver: driver.to_string(),
                    status: "skipped".to_string(),
                    outcome: None,
                    runpack_root_hash: None,
                    runpack_hash_algorithm: None,
                    notes: vec![err],
                });
            }
        };
        let args = vec![script.to_string_lossy().to_string()];
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| std::io::Error::other("missing workspace root"))?
            .to_path_buf();
        let mut paths = vec![
            workspace_root.join("sdks/python"),
            workspace_root.join("adapters/langchain/src"),
            workspace_root.join("adapters/crewai/src"),
            workspace_root.join("adapters/autogen/src"),
            workspace_root.join("adapters/openai_agents/src"),
        ];
        if let Some(existing) = env::var_os("PYTHONPATH") {
            paths.extend(env::split_paths(&existing));
        }
        let joined = env::join_paths(paths)
            .map_err(|err| std::io::Error::other(format!("pythonpath join failed: {err}")))?;
        envs.insert("PYTHONPATH".to_string(), joined.to_string_lossy().to_string());
        (runtime.path, args)
    };

    let output = match run_script(&interpreter, &args, &envs, Duration::from_secs(60)).await {
        Ok(output) => output,
        Err(err) => {
            server.shutdown().await;
            return Err(err.into());
        }
    };
    reporter
        .artifacts()
        .write_text(&format!("{}_{}_stdout.log", pack.id, driver), &output.stdout)?;
    reporter
        .artifacts()
        .write_text(&format!("{}_{}_stderr.log", pack.id, driver), &output.stderr)?;

    let payload = match parse_driver_output(&output.stdout) {
        Ok(payload) => payload,
        Err(err) => {
            server.shutdown().await;
            return Err(err);
        }
    };
    if payload.get("status").and_then(|val| val.as_str()) == Some("skipped") {
        server.shutdown().await;
        return Ok(DriverResult {
            driver: driver.to_string(),
            status: "skipped".to_string(),
            outcome: None,
            runpack_root_hash: None,
            runpack_hash_algorithm: None,
            notes: vec![
                payload
                    .get("reason")
                    .and_then(|val| val.as_str())
                    .unwrap_or("dependency missing")
                    .to_string(),
            ],
        });
    }
    if payload.get("status").and_then(Value::as_str) == Some("fatal_error") {
        let error = payload.get("error").and_then(Value::as_str).unwrap_or("fatal_error");
        let optional_adapter =
            matches!(driver, "langchain" | "crewai" | "autogen" | "openai_agents");
        let strict = env::var("DECISION_GATE_STRICT_AGENTIC_ADAPTERS").is_ok();
        if optional_adapter && !strict {
            server.shutdown().await;
            return Ok(DriverResult {
                driver: driver.to_string(),
                status: "skipped".to_string(),
                outcome: None,
                runpack_root_hash: None,
                runpack_hash_algorithm: None,
                notes: vec![format!("adapter failed: {error}")],
            });
        }
    }
    server.shutdown().await;

    Ok(DriverResult {
        driver: driver.to_string(),
        status: read_string(&payload, "status")?,
        outcome: payload.get("outcome").and_then(Value::as_str).map(ToString::to_string),
        runpack_root_hash: payload
            .get("runpack_root_hash")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        runpack_hash_algorithm: payload
            .get("runpack_hash_algorithm")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        notes: Vec::new(),
    })
}

async fn spawn_scenario_server(
    pack: &ScenarioPack,
    http_base_url: Option<&str>,
    policy_mode: Option<&str>,
) -> Result<(helpers::harness::McpServerHandle, McpHttpClient), DynError> {
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    apply_provider_config(&mut config, pack, http_base_url)?;
    apply_policy_config(&mut config, policy_mode);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(10))?;
    wait_for_server_ready(&client, Duration::from_secs(10)).await?;
    Ok((server, client))
}

fn parse_driver_output(stdout: &str) -> Result<Value, DynError> {
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| std::io::Error::other("driver produced no output"))?;
    Ok(serde_json::from_str(line)?)
}

fn apply_provider_config(
    config: &mut DecisionGateConfig,
    pack: &ScenarioPack,
    http_base_url: Option<&str>,
) -> Result<(), DynError> {
    let json_config = json_provider_config(&pack.fixtures_dir, &pack.id);
    set_provider_config(config, "json", json_config)?;

    let env_config = env_provider_config(&pack.env_overrides);
    set_provider_config(config, "env", env_config)?;

    let time_config = time_provider_config();
    set_provider_config(config, "time", time_config)?;

    if http_base_url.is_some() {
        let http_config = http_provider_config();
        set_provider_config(config, "http", http_config)?;
    }

    Ok(())
}

fn apply_policy_config(config: &mut DecisionGateConfig, policy_mode: Option<&str>) {
    if policy_mode != Some("deny_restricted") {
        return;
    }
    let rule = PolicyRule {
        effect: PolicyEffect::Deny,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: vec!["restricted".to_string()],
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let static_policy = StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![rule],
    };
    config.policy = PolicyConfig {
        engine: PolicyEngine::Static,
        static_policy: Some(static_policy),
    };
}

fn env_provider_config(overrides: &BTreeMap<String, String>) -> TomlValue {
    let mut table = Table::new();
    table.insert("denylist".to_string(), TomlValue::Array(Vec::new()));
    table.insert("max_value_bytes".to_string(), TomlValue::Integer(65536));
    table.insert("max_key_bytes".to_string(), TomlValue::Integer(255));
    if !overrides.is_empty() {
        let allowlist = overrides.keys().cloned().map(TomlValue::String).collect();
        table.insert("allowlist".to_string(), TomlValue::Array(allowlist));
        let mut override_table = Table::new();
        for (key, value) in overrides {
            override_table.insert(key.clone(), TomlValue::String(value.clone()));
        }
        table.insert("overrides".to_string(), TomlValue::Table(override_table));
    }
    TomlValue::Table(table)
}

fn json_provider_config(root: &Path, scenario_id: &str) -> TomlValue {
    let mut table = Table::new();
    table.insert("root".to_string(), TomlValue::String(root.to_string_lossy().to_string()));
    table.insert("root_id".to_string(), TomlValue::String(format!("agentic_{scenario_id}")));
    table.insert("allow_yaml".to_string(), TomlValue::Boolean(false));
    table.insert("max_bytes".to_string(), TomlValue::Integer(1024 * 1024));
    TomlValue::Table(table)
}

fn http_provider_config() -> TomlValue {
    let mut table = Table::new();
    table.insert("allow_http".to_string(), TomlValue::Boolean(true));
    table.insert(
        "allowed_hosts".to_string(),
        TomlValue::Array(vec![TomlValue::String("127.0.0.1".to_string())]),
    );
    table.insert("timeout_ms".to_string(), TomlValue::Integer(2000));
    table.insert("max_response_bytes".to_string(), TomlValue::Integer(1024 * 1024));
    table.insert("hash_algorithm".to_string(), TomlValue::String("sha256".to_string()));
    table.insert("user_agent".to_string(), TomlValue::String("dg-agentic".to_string()));
    TomlValue::Table(table)
}

fn time_provider_config() -> TomlValue {
    let mut table = Table::new();
    table.insert("allow_logical".to_string(), TomlValue::Boolean(true));
    TomlValue::Table(table)
}

fn set_provider_config(
    config: &mut DecisionGateConfig,
    name: &str,
    value: TomlValue,
) -> Result<(), DynError> {
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.name == name)
        .ok_or_else(|| std::io::Error::other(format!("missing provider {name}")))?;
    provider.config = Some(value);
    Ok(())
}

fn replace_http_placeholder(spec: &Value, http_base_url: Option<&str>) -> Value {
    if let Some(base) = http_base_url {
        return replace_placeholder_value(spec, HTTP_PLACEHOLDER, base);
    }
    spec.clone()
}

fn replace_placeholder_value(value: &Value, token: &str, replacement: &str) -> Value {
    match value {
        Value::String(value) => {
            if value.contains(token) {
                Value::String(value.replace(token, replacement))
            } else {
                Value::String(value.clone())
            }
        }
        Value::Array(items) => Value::Array(
            items.iter().map(|item| replace_placeholder_value(item, token, replacement)).collect(),
        ),
        Value::Object(map) => {
            let mut replaced = serde_json::Map::new();
            for (key, value) in map {
                replaced.insert(key.clone(), replace_placeholder_value(value, token, replacement));
            }
            Value::Object(replaced)
        }
        _ => value.clone(),
    }
}

fn read_string(value: &Value, field: &str) -> Result<String, DynError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| std::io::Error::other(format!("missing {field}")).into())
}

fn read_u64(value: &Value, field: &str) -> Result<u64, DynError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| std::io::Error::other(format!("missing {field}")).into())
}

fn extract_outcome(value: &Value) -> Option<String> {
    let decision = value.get("last_decision")?;
    let outcome = decision.get("outcome")?;
    if let Some(kind) = outcome.get("kind").and_then(|val| val.as_str()) {
        return Some(kind.to_lowercase());
    }
    let map = outcome.as_object()?;
    if map.len() != 1 {
        return None;
    }
    map.keys().next().map(|key| key.to_lowercase())
}

fn read_root_hash(export: &Value) -> Option<String> {
    export
        .get("manifest")
        .and_then(|value| value.get("integrity"))
        .and_then(|value| value.get("root_hash"))
        .and_then(|value| value.get("value"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn read_root_hash_algorithm(export: &Value) -> Option<String> {
    export
        .get("manifest")
        .and_then(|value| value.get("integrity"))
        .and_then(|value| value.get("root_hash"))
        .and_then(|value| value.get("algorithm"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn allocate_runpack_dir(
    reporter: &TestReporter,
    scenario_id: &str,
    driver: &str,
) -> Result<PathBuf, DynError> {
    let root =
        reporter.artifacts().root().join("agentic").join(sanitize_name(scenario_id)).join(driver);
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn sanitize_name(value: &str) -> String {
    value.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' { ch } else { '_' }).collect()
}

fn enforce_expectations(
    scenario: &ScenarioEntry,
    pack: &ScenarioPack,
    result: &ScenarioResult,
    update_expected: bool,
) -> Result<(), DynError> {
    let baseline = result
        .baseline_root_hash
        .clone()
        .ok_or_else(|| std::io::Error::other("missing baseline root hash"))?;

    let expected_hash = read_expected_hash(&pack.expected_hash_path)?;
    if update_expected {
        if let Some(parent) = pack.expected_hash_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&pack.expected_hash_path, format!("{baseline}\n"))?;
    } else if let Some(expected) = expected_hash {
        if expected != baseline {
            return Err(std::io::Error::other(format!(
                "runpack hash mismatch for {} (expected {}, got {})",
                scenario.id, expected, baseline
            ))
            .into());
        }
    } else {
        return Err(std::io::Error::other(format!(
            "missing expected runpack hash for {} (set UPDATE_AGENTIC_EXPECTED=1)",
            scenario.id
        ))
        .into());
    }

    for driver in &result.drivers {
        if driver.status == "skipped" {
            continue;
        }
        if driver.status != scenario.expected_status {
            return Err(std::io::Error::other(format!(
                "status mismatch for {} via {} (expected {}, got {})",
                scenario.id, driver.driver, scenario.expected_status, driver.status
            ))
            .into());
        }
        if let Some(expected_outcome) = &scenario.expected_outcome
            && driver.outcome.as_ref() != Some(expected_outcome)
        {
            return Err(std::io::Error::other(format!(
                "outcome mismatch for {} via {} (expected {:?}, got {:?})",
                scenario.id, driver.driver, scenario.expected_outcome, driver.outcome
            ))
            .into());
        }
        if let Some(hash) = &driver.runpack_root_hash
            && hash != &baseline
        {
            return Err(std::io::Error::other(format!(
                "runpack hash mismatch for {} via {} (expected {}, got {})",
                scenario.id, driver.driver, baseline, hash
            ))
            .into());
        }
    }

    Ok(())
}

fn read_expected_hash(path: &Path) -> Result<Option<String>, DynError> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)?;
    let value = contents.trim();
    if value.is_empty() || value == "PENDING" {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::helpers::env as test_env;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        entries: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new(names: &[&'static str]) -> Self {
            let entries = names.iter().map(|name| (*name, env::var(*name).ok())).collect();
            Self {
                entries,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in self.entries.drain(..) {
                match value {
                    Some(value) => test_env::set_var(name, &value),
                    None => test_env::remove_var(name),
                }
            }
        }
    }

    #[test]
    fn http_stub_port_respects_explicit_override() -> Result<(), DynError> {
        let _lock = ENV_LOCK.lock().map_err(|_| std::io::Error::other("env lock poisoned"))?;
        let _guard = EnvGuard::new(&[
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE",
        ]);
        test_env::set_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT", "32123");
        let port = http_stub_port("agentic-ci-gate")?;
        if port != 32123 {
            return Err(format!("expected port 32123, got {port}").into());
        }
        Ok(())
    }

    #[test]
    fn http_stub_port_rejects_zero_override() -> Result<(), DynError> {
        let _lock = ENV_LOCK.lock().map_err(|_| std::io::Error::other("env lock poisoned"))?;
        let _guard = EnvGuard::new(&[
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE",
        ]);
        test_env::set_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT", "0");
        if http_stub_port("agentic-ci-gate").is_ok() {
            return Err("expected zero override to fail".into());
        }
        Ok(())
    }

    #[test]
    fn http_stub_port_is_deterministic_and_bounded() -> Result<(), DynError> {
        let _lock = ENV_LOCK.lock().map_err(|_| std::io::Error::other("env lock poisoned"))?;
        let _guard = EnvGuard::new(&[
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE",
        ]);
        test_env::remove_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT");
        test_env::set_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE", "30000");
        test_env::set_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE", "100");
        let port_a = http_stub_port("agentic-ci-gate")?;
        let port_b = http_stub_port("agentic-ci-gate")?;
        if port_a != port_b {
            return Err(format!("expected deterministic port, got {port_a} vs {port_b}").into());
        }
        if !(30000 .. 30100).contains(&port_a) {
            return Err(format!("expected port in range, got {port_a}").into());
        }
        Ok(())
    }

    #[test]
    fn http_stub_port_rejects_overflow_range() -> Result<(), DynError> {
        let _lock = ENV_LOCK.lock().map_err(|_| std::io::Error::other("env lock poisoned"))?;
        let _guard = EnvGuard::new(&[
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE",
            "DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE",
        ]);
        test_env::remove_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT");
        test_env::set_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_BASE", "65530");
        test_env::set_var("DECISION_GATE_SYSTEM_TEST_HTTP_STUB_PORT_RANGE", "10");
        if http_stub_port("agentic-ci-gate").is_ok() {
            return Err("expected overflow range to fail".into());
        }
        Ok(())
    }

    #[test]
    fn replace_http_placeholder_updates_nested_values() {
        let spec = json!({
            "url": "{{HTTP_BASE_URL}}/artifact.json",
            "nested": {
                "list": ["keep", "{{HTTP_BASE_URL}}/fetch.json"]
            },
            "count": 3
        });
        let replaced = replace_http_placeholder(&spec, Some("http://127.0.0.1:1234"));
        assert_eq!(replaced["url"], "http://127.0.0.1:1234/artifact.json");
        assert_eq!(replaced["nested"]["list"][1], "http://127.0.0.1:1234/fetch.json");
        assert_eq!(replaced["count"], 3);
    }

    #[test]
    fn replace_http_placeholder_noop_when_missing() {
        let spec = json!({"url": "http://example.com", "list": ["a"]});
        let replaced = replace_http_placeholder(&spec, None);
        assert_eq!(replaced, spec);
    }

    #[test]
    fn enforce_expectations_overwrites_expected_hash() -> Result<(), DynError> {
        let dir = TempDir::new()?;
        let expected_path = dir.path().join("expected/runpack_root_hash.txt");
        let Some(parent) = expected_path.parent() else {
            return Err(std::io::Error::other("expected path missing parent").into());
        };
        fs::create_dir_all(parent)?;
        fs::write(&expected_path, "oldhash\n")?;

        let scenario = ScenarioEntry {
            id: "agentic-ci-gate".to_string(),
            description: None,
            providers: Vec::new(),
            drivers: Vec::new(),
            modes: Vec::new(),
            expected_status: "completed".to_string(),
            expected_outcome: None,
            policy_mode: None,
        };
        let pack = ScenarioPack {
            id: "agentic-ci-gate".to_string(),
            root: dir.path().to_path_buf(),
            fixtures_dir: dir.path().join("fixtures"),
            spec: json!({}),
            run_config: json!({}),
            trigger: json!({}),
            env_overrides: BTreeMap::new(),
            expected_hash_path: expected_path.clone(),
        };
        let result = ScenarioResult {
            scenario_id: "agentic-ci-gate".to_string(),
            expected_status: "completed".to_string(),
            expected_outcome: None,
            drivers: Vec::new(),
            baseline_root_hash: Some("newhash".to_string()),
        };

        enforce_expectations(&scenario, &pack, &result, true)?;
        let updated = fs::read_to_string(expected_path)?;
        if updated != "newhash\n" {
            return Err(format!("expected updated hash, got {updated:?}").into());
        }
        Ok(())
    }

    #[test]
    fn enforce_expectations_rejects_hash_mismatch_without_update() -> Result<(), DynError> {
        let dir = TempDir::new()?;
        let expected_path = dir.path().join("expected/runpack_root_hash.txt");
        let Some(parent) = expected_path.parent() else {
            return Err(std::io::Error::other("expected path missing parent").into());
        };
        fs::create_dir_all(parent)?;
        fs::write(&expected_path, "oldhash\n")?;

        let scenario = ScenarioEntry {
            id: "agentic-ci-gate".to_string(),
            description: None,
            providers: Vec::new(),
            drivers: Vec::new(),
            modes: Vec::new(),
            expected_status: "completed".to_string(),
            expected_outcome: None,
            policy_mode: None,
        };
        let pack = ScenarioPack {
            id: "agentic-ci-gate".to_string(),
            root: dir.path().to_path_buf(),
            fixtures_dir: dir.path().join("fixtures"),
            spec: json!({}),
            run_config: json!({}),
            trigger: json!({}),
            env_overrides: BTreeMap::new(),
            expected_hash_path: expected_path.clone(),
        };
        let result = ScenarioResult {
            scenario_id: "agentic-ci-gate".to_string(),
            expected_status: "completed".to_string(),
            expected_outcome: None,
            drivers: Vec::new(),
            baseline_root_hash: Some("newhash".to_string()),
        };

        let Err(err) = enforce_expectations(&scenario, &pack, &result, false) else {
            return Err("expected hash mismatch error".into());
        };
        if !err.to_string().contains("runpack hash mismatch") {
            return Err(format!("unexpected error: {err}").into());
        }
        Ok(())
    }
}
