// system-tests/tests/helpers/cli.rs
// ============================================================================
// Module: CLI Helpers
// Description: Shared helpers for locating and invoking the decision-gate CLI.
// Purpose: Provide consistent CLI binary resolution across system-test suites.
// Dependencies: std::process, std::path
// ============================================================================

//! Helpers for invoking the decision-gate CLI in system-tests.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::OnceLock;

/// Locates the decision-gate CLI binary, building it if necessary.
pub fn cli_binary() -> Option<PathBuf> {
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

/// Runs the CLI with arguments and returns the process output.
pub fn run_cli(binary: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(binary)
        .args(args)
        .output()
        .map_err(|err| format!("run decision-gate failed: {err}"))
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
