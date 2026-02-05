// system-tests/tests/helpers/sdk_runner.rs
// ============================================================================
// Module: SDK Test Runner
// Description: Helpers for executing SDK scripts from system-tests.
// Purpose: Run Python/TypeScript SDK scripts with deterministic environment.
// Dependencies: tokio, stdlib
// ============================================================================

//! ## Overview
//! Helpers for executing SDK scripts from system-tests.
//! Purpose: Run Python/TypeScript SDK scripts with deterministic environment.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "Test helper module keeps docs focused on external behavior."
)]

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;
use url::Url;

pub struct ScriptOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

pub struct RuntimeCheck {
    pub path: PathBuf,
    pub notes: Vec<String>,
}

pub fn node_options_with_loader(existing: Option<String>, loader_path: Option<&Path>) -> String {
    let mut options = match existing {
        Some(value) if !value.is_empty() => format!("{value} --unhandled-rejections=strict"),
        _ => "--unhandled-rejections=strict".to_string(),
    };
    if let Some(path) = loader_path {
        let loader_arg = Url::from_file_path(path)
            .map_or_else(|()| path.display().to_string(), |url| url.to_string());
        options = format!("{options} --experimental-loader={loader_arg}");
    }
    options
}

pub fn python_runtime() -> Result<RuntimeCheck, String> {
    resolve_runtime(&["python3", "python"], &["--version"])
}

pub fn node_runtime_for_typescript() -> Result<RuntimeCheck, String> {
    let runtime = resolve_runtime(&["node"], &["--version"])?;
    if !supports_node_flag(&runtime.path, "--experimental-strip-types")? {
        return Err("node lacks --experimental-strip-types support".to_string());
    }
    if !node_has_fetch(&runtime.path)? {
        return Err("node fetch API unavailable; Node 18+ required".to_string());
    }
    Ok(runtime)
}

pub async fn run_script(
    interpreter: &Path,
    args: &[String],
    envs: &HashMap<String, String>,
    timeout_duration: Duration,
) -> Result<ScriptOutput, String> {
    let mut command = Command::new(interpreter);
    command.args(args);
    command.envs(envs);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = command.spawn().map_err(|err| format!("spawn failed: {err}"))?;
    let output = timeout(timeout_duration, child.wait_with_output())
        .await
        .map_err(|_| "script timed out".to_string())?
        .map_err(|err| format!("script failed: {err}"))?;

    let stdout =
        String::from_utf8(output.stdout).map_err(|err| format!("stdout decode failed: {err}"))?;
    let stderr =
        String::from_utf8(output.stderr).map_err(|err| format!("stderr decode failed: {err}"))?;

    Ok(ScriptOutput {
        status: output.status,
        stdout,
        stderr,
    })
}

fn resolve_runtime(candidates: &[&str], args: &[&str]) -> Result<RuntimeCheck, String> {
    let mut notes = Vec::new();
    for candidate in candidates {
        match std::process::Command::new(candidate).args(args).output() {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{stdout}{stderr}");
                if combined.contains("Microsoft Store")
                    || combined.contains("App execution aliases")
                    || combined.contains("Python was not found")
                {
                    notes.push(format!("{candidate} resolved to a placeholder stub"));
                    continue;
                }
                return Ok(RuntimeCheck {
                    path: PathBuf::from(candidate),
                    notes,
                });
            }
            Ok(output) => {
                let message =
                    format!("{candidate} returned {:?}", output.status.code().unwrap_or(-1));
                notes.push(message);
            }
            Err(err) => {
                notes.push(format!("{candidate} unavailable: {err}"));
            }
        }
    }
    Err(format!("runtime not available ({})", notes.join("; ")))
}

fn supports_node_flag(node: &Path, flag: &str) -> Result<bool, String> {
    let output = std::process::Command::new(node)
        .args([flag, "-e", "console.log('ok')"])
        .output()
        .map_err(|err| format!("node probe failed: {err}"))?;
    Ok(output.status.success())
}

fn node_has_fetch(node: &Path) -> Result<bool, String> {
    let output = std::process::Command::new(node)
        .args([
            "--experimental-strip-types",
            "-e",
            "process.exit(typeof fetch === 'function' ? 0 : 2)",
        ])
        .output()
        .map_err(|err| format!("node fetch probe failed: {err}"))?;
    Ok(output.status.success())
}
