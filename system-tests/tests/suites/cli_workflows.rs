// system-tests/tests/suites/cli_workflows.rs
// ============================================================================
// Module: CLI Workflow Tests
// Description: End-to-end Decision Gate CLI command coverage.
// Purpose: Validate serve, runpack, authoring, config, provider, and interop flows.
// Dependencies: system-tests helpers, decision-gate-core
// ============================================================================

//! CLI workflow coverage for Decision Gate system-tests.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::OnceLock;
use std::time::Duration;

use decision_gate_core::Timestamp;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::Value;
use tempfile::TempDir;

use crate::helpers;

struct CliServer {
    child: std::process::Child,
}

impl CliServer {
    fn spawn(
        binary: &Path,
        config_path: &Path,
        stdout: &Path,
        stderr: &Path,
    ) -> Result<Self, String> {
        let out = fs::File::create(stdout).map_err(|err| format!("stdout file: {err}"))?;
        let err = fs::File::create(stderr).map_err(|err| format!("stderr file: {err}"))?;
        let child = Command::new(binary)
            .args(["serve", "--config"])
            .arg(config_path)
            .stdout(out)
            .stderr(err)
            .spawn()
            .map_err(|err| format!("spawn decision-gate serve failed: {err}"))?;
        Ok(Self {
            child,
        })
    }

    fn shutdown(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn cli_binary() -> Option<PathBuf> {
    if let Some(path) = option_env!("CARGO_BIN_EXE_decision_gate") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_decision_gate") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    build_cli_binary().map_or_else(|_| resolve_cli_from_current_exe(), Some)
}

fn resolve_cli_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let profile_dir = exe.parent()?.parent()?;
    let candidate = profile_dir.join(format!("decision-gate{}", exe_suffix()));
    if candidate.exists() { Some(candidate) } else { None }
}

fn target_dir_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let profile_dir = exe.parent()?.parent()?;
    profile_dir.parent().map(PathBuf::from)
}

fn build_cli_binary() -> Result<PathBuf, String> {
    static BUILD_RESULT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let result = BUILD_RESULT.get_or_init(|| {
        let Some(target_dir) = target_dir_from_current_exe() else {
            return Err("unable to resolve target dir from current exe".to_string());
        };
        let output = Command::new("cargo")
            .args(["build", "-p", "decision-gate-cli", "--bin", "decision-gate", "--target-dir"])
            .arg(&target_dir)
            .output()
            .map_err(|err| format!("spawn cargo build failed: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "cargo build decision-gate-cli failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        resolve_cli_from_target_dir(&target_dir)
            .ok_or_else(|| "decision-gate binary not found after build".to_string())
    });
    result.clone()
}

fn resolve_cli_from_target_dir(target_dir: &Path) -> Option<PathBuf> {
    let profile_dir = target_dir.join("debug");
    let candidate = profile_dir.join(format!("decision-gate{}", exe_suffix()));
    if candidate.exists() { Some(candidate) } else { None }
}

const fn exe_suffix() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}

fn write_cli_config(path: &Path, bind: &str) -> Result<(), String> {
    let contents = format!(
        r#"[server]
transport = "http"
mode = "strict"
bind = "{bind}"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"
"#
    );
    fs::write(path, contents).map_err(|err| format!("write config: {err}"))
}

fn run_cli(binary: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(binary)
        .args(args)
        .output()
        .map_err(|err| format!("run decision-gate failed: {err}"))
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "End-to-end CLI workflow stays linear for auditability.")]
async fn cli_workflows_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_workflows_end_to_end")?;
    let Some(cli) = cli_binary() else {
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec!["summary.json".to_string(), "summary.md".to_string()],
        )?;
        drop(reporter);
        return Ok(());
    };
    let temp_dir = TempDir::new()?;
    let bind = allocate_bind_addr()?.to_string();
    helpers::harness::release_bind_addr(&bind);
    let config_path = temp_dir.path().join("decision-gate.toml");
    write_cli_config(&config_path, &bind)?;

    let stdout_path = reporter.artifacts().root().join("cli.serve.stdout.log");
    let stderr_path = reporter.artifacts().root().join("cli.serve.stderr.log");
    let server = CliServer::spawn(&cli, &config_path, &stdout_path, &stderr_path)?;

    let base_url = format!("http://{bind}/rpc");
    let client = McpHttpClient::new(base_url.clone(), Duration::from_millis(750))?;
    wait_for_server_ready(&client, Duration::from_secs(15)).await?;

    let fixture = ScenarioFixture::time_after("cli-runpack", "run-1", 0);
    let mut spec = fixture.spec.clone();
    spec.default_tenant_id = Some(fixture.tenant_id);
    let define_request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec: spec.clone(),
    };
    client
        .call_tool_typed::<decision_gate_mcp::tools::ScenarioDefineResponse>(
            "scenario_define",
            serde_json::to_value(&define_request)?,
        )
        .await?;
    let start_request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: spec.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let run_state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", serde_json::to_value(&start_request)?).await?;

    let spec_path = temp_dir.path().join("spec.json");
    let state_path = temp_dir.path().join("run_state.json");
    fs::write(&spec_path, serde_json::to_vec(&spec)?)?;
    fs::write(&state_path, serde_json::to_vec(&run_state)?)?;

    let runpack_dir = temp_dir.path().join("runpack");
    let output = run_cli(
        &cli,
        &[
            "runpack",
            "export",
            "--spec",
            spec_path.to_str().unwrap_or_default(),
            "--state",
            state_path.to_str().unwrap_or_default(),
            "--output-dir",
            runpack_dir.to_str().unwrap_or_default(),
            "--with-verification",
            "--generated-at-unix-ms",
            "1",
        ],
    )?;
    reporter
        .artifacts()
        .write_text("cli.runpack.export.stdout.log", &String::from_utf8_lossy(&output.stdout))?;
    reporter
        .artifacts()
        .write_text("cli.runpack.export.stderr.log", &String::from_utf8_lossy(&output.stderr))?;
    if !output.status.success() {
        return Err("runpack export CLI failed".into());
    }
    let manifest_path = runpack_dir.join("runpack.json");
    if !manifest_path.exists() {
        return Err("runpack manifest missing after CLI export".into());
    }

    let verify = run_cli(
        &cli,
        &["runpack", "verify", "--manifest", manifest_path.to_str().unwrap_or_default()],
    )?;
    reporter
        .artifacts()
        .write_text("cli.runpack.verify.stdout.log", &String::from_utf8_lossy(&verify.stdout))?;
    reporter
        .artifacts()
        .write_text("cli.runpack.verify.stderr.log", &String::from_utf8_lossy(&verify.stderr))?;
    if !verify.status.success() {
        return Err("runpack verify CLI failed".into());
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root =
        manifest_dir.parent().ok_or_else(|| "failed to resolve repo root".to_string())?;
    let ron_path = repo_root.join("Docs/generated/decision-gate/examples/scenario.ron");
    let normalized_path = temp_dir.path().join("scenario.json");
    let validate = run_cli(
        &cli,
        &[
            "authoring",
            "validate",
            "--input",
            ron_path.to_str().unwrap_or_default(),
            "--format",
            "ron",
        ],
    )?;
    reporter.artifacts().write_text(
        "cli.authoring.validate.stdout.log",
        &String::from_utf8_lossy(&validate.stdout),
    )?;
    reporter.artifacts().write_text(
        "cli.authoring.validate.stderr.log",
        &String::from_utf8_lossy(&validate.stderr),
    )?;
    if !validate.status.success() {
        return Err("authoring validate CLI failed".into());
    }
    let normalize = run_cli(
        &cli,
        &[
            "authoring",
            "normalize",
            "--input",
            ron_path.to_str().unwrap_or_default(),
            "--format",
            "ron",
            "--output",
            normalized_path.to_str().unwrap_or_default(),
        ],
    )?;
    reporter.artifacts().write_text(
        "cli.authoring.normalize.stdout.log",
        &String::from_utf8_lossy(&normalize.stdout),
    )?;
    reporter.artifacts().write_text(
        "cli.authoring.normalize.stderr.log",
        &String::from_utf8_lossy(&normalize.stderr),
    )?;
    if !normalize.status.success() {
        return Err("authoring normalize CLI failed".into());
    }
    let normalized: Value = serde_json::from_slice(&fs::read(&normalized_path)?)?;
    if normalized.get("scenario_id").is_none() {
        return Err("normalized scenario missing scenario_id".into());
    }

    let config_validate = run_cli(
        &cli,
        &["config", "validate", "--config", config_path.to_str().unwrap_or_default()],
    )?;
    reporter.artifacts().write_text(
        "cli.config.validate.stdout.log",
        &String::from_utf8_lossy(&config_validate.stdout),
    )?;
    reporter.artifacts().write_text(
        "cli.config.validate.stderr.log",
        &String::from_utf8_lossy(&config_validate.stderr),
    )?;
    if !config_validate.status.success() {
        return Err("config validate CLI failed".into());
    }

    let provider_contract = run_cli(
        &cli,
        &[
            "provider",
            "contract",
            "get",
            "--provider",
            "time",
            "--config",
            config_path.to_str().unwrap_or_default(),
        ],
    )?;
    if !provider_contract.status.success() {
        return Err("provider contract get CLI failed".into());
    }
    let contract_json: Value = serde_json::from_slice(&provider_contract.stdout)?;
    if contract_json.get("provider_id").is_none() {
        return Err("provider contract response missing provider_id".into());
    }

    let provider_schema = run_cli(
        &cli,
        &[
            "provider",
            "schema",
            "get",
            "--provider",
            "time",
            "--predicate",
            "after",
            "--config",
            config_path.to_str().unwrap_or_default(),
        ],
    )?;
    if !provider_schema.status.success() {
        return Err("provider schema get CLI failed".into());
    }
    let schema_json: Value = serde_json::from_slice(&provider_schema.stdout)?;
    if schema_json.get("predicate").is_none() {
        return Err("provider schema response missing predicate".into());
    }

    let interop_fixture = ScenarioFixture::time_after("cli-interop", "run-interop", 0);
    let mut interop_spec = interop_fixture.spec.clone();
    interop_spec.default_tenant_id = Some(interop_fixture.tenant_id);
    let interop_spec_path = temp_dir.path().join("interop_spec.json");
    let interop_run_config_path = temp_dir.path().join("interop_run_config.json");
    let interop_trigger_path = temp_dir.path().join("interop_trigger.json");
    fs::write(&interop_spec_path, serde_json::to_vec(&interop_spec)?)?;
    fs::write(&interop_run_config_path, serde_json::to_vec(&interop_fixture.run_config())?)?;
    fs::write(
        &interop_trigger_path,
        serde_json::to_vec(&interop_fixture.trigger_event("trigger-1", Timestamp::Logical(2)))?,
    )?;

    let interop_report_path = temp_dir.path().join("interop_report.json");
    let interop = run_cli(
        &cli,
        &[
            "interop",
            "eval",
            "--mcp-url",
            &base_url,
            "--spec",
            interop_spec_path.to_str().unwrap_or_default(),
            "--run-config",
            interop_run_config_path.to_str().unwrap_or_default(),
            "--trigger",
            interop_trigger_path.to_str().unwrap_or_default(),
            "--started-at-logical",
            "1",
            "--status-requested-at-logical",
            "3",
            "--output",
            interop_report_path.to_str().unwrap_or_default(),
        ],
    )?;
    reporter
        .artifacts()
        .write_text("cli.interop.stdout.log", &String::from_utf8_lossy(&interop.stdout))?;
    reporter
        .artifacts()
        .write_text("cli.interop.stderr.log", &String::from_utf8_lossy(&interop.stderr))?;
    if !interop.status.success() {
        return Err("interop eval CLI failed".into());
    }
    let report: Value = serde_json::from_slice(&fs::read(&interop_report_path)?)?;
    if report.get("trigger_result").is_none() {
        return Err("interop report missing trigger_result".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec![
            "CLI serve, runpack, authoring, config, provider, and interop workflows passed"
                .to_string(),
        ],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "cli.serve.stdout.log".to_string(),
            "cli.serve.stderr.log".to_string(),
            "cli.runpack.export.stdout.log".to_string(),
            "cli.runpack.export.stderr.log".to_string(),
            "cli.runpack.verify.stdout.log".to_string(),
            "cli.runpack.verify.stderr.log".to_string(),
            "cli.authoring.validate.stdout.log".to_string(),
            "cli.authoring.validate.stderr.log".to_string(),
            "cli.authoring.normalize.stdout.log".to_string(),
            "cli.authoring.normalize.stderr.log".to_string(),
            "cli.config.validate.stdout.log".to_string(),
            "cli.config.validate.stderr.log".to_string(),
            "cli.interop.stdout.log".to_string(),
            "cli.interop.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown();
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_rejects_non_loopback_bind() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("cli_rejects_non_loopback_bind")?;
    let Some(cli) = cli_binary() else {
        reporter.finish(
            "skip",
            vec!["decision-gate CLI binary unavailable".to_string()],
            vec!["summary.json".to_string(), "summary.md".to_string()],
        )?;
        drop(reporter);
        return Ok(());
    };
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("decision-gate.toml");
    let contents = r#"[server]
transport = "http"
mode = "strict"
bind = "0.0.0.0:8088"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[[providers]]
name = "time"
type = "builtin"
"#;
    fs::write(&config_path, contents)?;

    let output = run_cli(&cli, &["serve", "--config", config_path.to_str().unwrap_or_default()])?;
    reporter
        .artifacts()
        .write_text("cli.non_loopback.stdout.log", &String::from_utf8_lossy(&output.stdout))?;
    reporter
        .artifacts()
        .write_text("cli.non_loopback.stderr.log", &String::from_utf8_lossy(&output.stderr))?;
    if output.status.success() {
        return Err("expected serve to reject non-loopback bind".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["CLI rejects non-loopback binds without explicit allow".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "cli.non_loopback.stdout.log".to_string(),
            "cli.non_loopback.stderr.log".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
