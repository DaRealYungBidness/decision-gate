// system-tests/tests/suites/sdk_examples.rs
// ============================================================================
// Module: SDK Example Tests
// Description: Executes repository examples against a live MCP server.
// Purpose: Ensure examples are runnable, deterministic, and auditable.
// Dependencies: system-tests helpers, decision-gate-core, tokio
// ============================================================================

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "Test suite helpers keep documentation concise."
)]

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use helpers::sdk_runner;

use crate::helpers;

const fn tenant_id_one() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn namespace_id_one() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

const PYTHON_BASIC: &str = "examples/python/basic_lifecycle.py";
const PYTHON_AGENT: &str = "examples/python/agent_loop.py";
const PYTHON_CI: &str = "examples/python/ci_gate.py";
const PYTHON_PRECHECK: &str = "examples/python/precheck.py";

const TYPESCRIPT_BASIC: &str = "examples/typescript/basic_lifecycle.ts";
const TYPESCRIPT_AGENT: &str = "examples/typescript/agent_loop.ts";
const TYPESCRIPT_CI: &str = "examples/typescript/ci_gate.ts";
const TYPESCRIPT_PRECHECK: &str = "examples/typescript/precheck.ts";

#[tokio::test(flavor = "multi_thread")]
async fn python_examples_runnable() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("python_examples_runnable")?;
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

    let examples = vec![
        ExampleCase::new(
            "basic",
            PYTHON_BASIC,
            &ScenarioFixture::time_after("py-basic", "run-py-basic", 0),
        ),
        ExampleCase::new(
            "agent",
            PYTHON_AGENT,
            &ScenarioFixture::time_after("py-agent", "run-py-agent", 0),
        )
        .with_agent("agent-alpha"),
        ExampleCase::new("ci", PYTHON_CI, &ScenarioFixture::time_after("py-ci", "run-py-ci", 0))
            .with_ci_gate(),
        ExampleCase::precheck("precheck", PYTHON_PRECHECK, precheck_spec("py-precheck"))?,
    ];

    run_examples(&mut reporter, runtime.path.as_path(), &bind, examples).await?;

    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn typescript_examples_runnable() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("typescript_examples_runnable")?;
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

    let examples = vec![
        ExampleCase::new(
            "basic",
            TYPESCRIPT_BASIC,
            &ScenarioFixture::time_after("ts-basic", "run-ts-basic", 0),
        ),
        ExampleCase::new(
            "agent",
            TYPESCRIPT_AGENT,
            &ScenarioFixture::time_after("ts-agent", "run-ts-agent", 0),
        )
        .with_agent("agent-alpha"),
        ExampleCase::new(
            "ci",
            TYPESCRIPT_CI,
            &ScenarioFixture::time_after("ts-ci", "run-ts-ci", 0),
        )
        .with_ci_gate(),
        ExampleCase::precheck("precheck", TYPESCRIPT_PRECHECK, precheck_spec("ts-precheck"))?,
    ];

    run_examples(&mut reporter, runtime.path.as_path(), &bind, examples).await?;

    drop(reporter);
    server.shutdown().await;
    Ok(())
}

#[derive(Debug, Clone)]
struct ExampleCase {
    name: String,
    script: &'static str,
    spec: ScenarioSpec,
    run_config: RunConfig,
    extra_envs: HashMap<String, String>,
    expects: ExampleExpectation,
}

impl ExampleCase {
    fn new(name: &str, script: &'static str, fixture: &ScenarioFixture) -> Self {
        let mut spec = fixture.spec.clone();
        spec.default_tenant_id = Some(fixture.tenant_id);
        let run_config = fixture.run_config();
        Self {
            name: name.to_string(),
            script,
            spec,
            run_config,
            extra_envs: HashMap::new(),
            expects: ExampleExpectation::Lifecycle,
        }
    }

    fn precheck(
        name: &str,
        script: &'static str,
        spec: ScenarioSpec,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let run_config = RunConfig {
            tenant_id: tenant_id_one(),
            namespace_id: namespace_id_one(),
            run_id: decision_gate_core::RunId::new("precheck-run"),
            scenario_id: spec.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        };
        let mut case = Self {
            name: name.to_string(),
            script,
            spec,
            run_config,
            extra_envs: HashMap::new(),
            expects: ExampleExpectation::Precheck,
        };
        let schema_json = serde_json::to_string(&schema_record_json())
            .map_err(|error| std::io::Error::other(format!("schema json failed: {error}")))?;
        case.extra_envs.insert("DG_SCHEMA_RECORD".to_string(), schema_json);
        Ok(case)
    }

    fn with_agent(mut self, agent_id: &str) -> Self {
        self.run_config.dispatch_targets.push(DispatchTarget::Agent {
            agent_id: agent_id.to_string(),
        });
        self.extra_envs.insert("DG_AGENT_ID".to_string(), agent_id.to_string());
        self.expects = ExampleExpectation::AgentLoop;
        self
    }

    const fn with_ci_gate(mut self) -> Self {
        self.expects = ExampleExpectation::CiGate;
        self
    }
}

#[derive(Debug, Clone, Copy)]
enum ExampleExpectation {
    Lifecycle,
    AgentLoop,
    CiGate,
    Precheck,
}

#[allow(
    clippy::future_not_send,
    reason = "TestReporter holds a mutex guard; examples are not spawned across threads."
)]
async fn run_examples(
    reporter: &mut TestReporter,
    runtime: &Path,
    bind: &str,
    examples: Vec<ExampleCase>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut artifacts = vec!["summary.json".to_string(), "summary.md".to_string()];
    for case in &examples {
        artifacts.push(format!("example.{}.stdout.log", case.name));
        artifacts.push(format!("example.{}.stderr.log", case.name));
    }

    for case in examples {
        let output = run_example_script(
            runtime,
            &example_path(case.script),
            bind,
            None,
            &case.spec,
            &case.run_config,
            &case.extra_envs,
        )
        .await?;

        reporter
            .artifacts()
            .write_text(&format!("example.{}.stdout.log", case.name), &output.stdout)?;
        reporter
            .artifacts()
            .write_text(&format!("example.{}.stderr.log", case.name), &output.stderr)?;

        if !output.status.success() {
            reporter.finish(
                "fail",
                vec![format!("example {} failed: {}", case.name, output.status)],
                artifacts.clone(),
            )?;
            return Err(format!("example {} failed", case.name).into());
        }

        let payload: serde_json::Value = serde_json::from_str(output.stdout.trim())?;
        assert_example_payload(case.expects, &payload)
            .map_err(|err| format!("example {} output invalid: {err}", case.name))?;
    }

    reporter.finish("pass", vec!["all examples executed successfully".to_string()], artifacts)?;
    Ok(())
}

fn assert_example_payload(
    expectation: ExampleExpectation,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let get_obj = |key: &str| {
        payload
            .get(key)
            .and_then(|value| value.as_object())
            .ok_or_else(|| format!("missing {key} output"))
    };

    match expectation {
        ExampleExpectation::Lifecycle => {
            let define = get_obj("define")?;
            if define.get("scenario_id").is_none() {
                return Err("define missing scenario_id".to_string());
            }
            let start = get_obj("start")?;
            if start.get("run_id").is_none() {
                return Err("start missing run_id".to_string());
            }
            let _status = get_obj("status")?;
        }
        ExampleExpectation::AgentLoop => {
            let _define = get_obj("define")?;
            let _start = get_obj("start")?;
            let next = get_obj("next")?;
            if next.get("decision").is_none() && next.get("status").is_none() {
                return Err("next missing decision or status".to_string());
            }
        }
        ExampleExpectation::CiGate => {
            let _define = get_obj("define")?;
            let _start = get_obj("start")?;
            let _trigger = get_obj("trigger")?;
            let runpack = get_obj("runpack")?;
            if runpack.get("manifest").is_none() {
                return Err("runpack missing manifest".to_string());
            }
        }
        ExampleExpectation::Precheck => {
            let _define = get_obj("define")?;
            let _schema = get_obj("schema")?;
            let precheck = get_obj("precheck")?;
            if precheck.get("decision").is_none() {
                return Err("precheck missing decision".to_string());
            }
        }
    }
    Ok(())
}

async fn run_example_script(
    interpreter: &Path,
    script: &Path,
    bind: &str,
    token: Option<&str>,
    spec: &ScenarioSpec,
    run_config: &RunConfig,
    extra_envs: &HashMap<String, String>,
) -> Result<sdk_runner::ScriptOutput, Box<dyn std::error::Error>> {
    let mut envs = HashMap::new();
    envs.insert("DG_ENDPOINT".to_string(), format!("http://{bind}/rpc"));
    if let Some(token) = token {
        envs.insert("DG_TOKEN".to_string(), token.to_string());
    }
    envs.insert("DG_SCENARIO_SPEC".to_string(), serde_json::to_string(spec)?);
    envs.insert("DG_RUN_CONFIG".to_string(), serde_json::to_string(run_config)?);
    envs.insert("DG_STARTED_AT".to_string(), serde_json::to_string(&Timestamp::Logical(1))?);
    for (key, value) in extra_envs {
        envs.insert(key.clone(), value.clone());
    }

    let script = script
        .canonicalize()
        .map_err(|err| format!("example path missing: {} ({err})", script.display()))?;
    if script.extension().and_then(|ext| ext.to_str()) == Some("ts") {
        let args = vec!["--experimental-strip-types".to_string(), script.display().to_string()];
        let node_options = match std::env::var("NODE_OPTIONS") {
            Ok(existing) if !existing.is_empty() => {
                format!("{existing} --unhandled-rejections=strict")
            }
            _ => "--unhandled-rejections=strict".to_string(),
        };
        envs.insert("NODE_OPTIONS".to_string(), node_options);
        return Ok(
            sdk_runner::run_script(interpreter, &args, &envs, Duration::from_secs(25)).await?
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
    Ok(sdk_runner::run_script(interpreter, &args, &envs, Duration::from_secs(25)).await?)
}

fn example_path(relative: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.parent().map_or_else(|| root.join(relative), |parent| parent.join(relative))
}

fn precheck_spec(scenario_id: &str) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new(scenario_id),
        namespace_id: namespace_id_one(),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("main"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-env"),
                requirement: ret_logic::Requirement::condition(ConditionId::new("deploy_env")),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id: ConditionId::new("deploy_env"),
            query: EvidenceQuery {
                provider_id: ProviderId::new("env"),
                check_id: "get".to_string(),
                params: Some(serde_json::json!({"key": "DEPLOY_ENV"})),
            },
            comparator: Comparator::Equals,
            expected: Some(serde_json::json!("production")),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(tenant_id_one()),
    }
}

fn schema_record_json() -> serde_json::Value {
    serde_json::json!({
        "schema_id": "asserted_payload",
        "version": "v1",
        "description": "Asserted payload schema.",
        "tenant_id": 1,
        "namespace_id": 1,
        "created_at": { "kind": "logical", "value": 1 },
        "schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": { "deploy_env": { "type": "string" } },
            "required": ["deploy_env"]
        }
    })
}
