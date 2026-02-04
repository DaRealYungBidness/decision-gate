// system-tests/tests/helpers/artifacts.rs
// ============================================================================
// Module: Test Artifacts
// Description: Artifact helpers for system-tests.
// Purpose: Create per-test run roots and write deterministic summaries.
// Dependencies: system-tests, serde, serde_jcs
// ============================================================================

//! ## Overview
//! Artifact helpers for system-tests.
//! Purpose: Create per-test run roots and write deterministic summaries.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::env;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;

use serde::Serialize;
use serde_jcs;
use system_tests::config::SystemTestConfig;

#[derive(Debug, Serialize)]
struct TestSummary {
    test_name: String,
    status: String,
    started_at_ms: u128,
    ended_at_ms: u128,
    duration_ms: u128,
    notes: Vec<String>,
    artifacts: Vec<String>,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), Path::to_path_buf)
}

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn default_run_root(test_name: &str) -> PathBuf {
    let base = workspace_root().join("target/system-tests");
    if let Ok(run_id) = env::var("NEXTEST_RUN_ID") {
        return base.join(run_id).join(test_name);
    }
    let run_id = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    base.join(format!("run_{pid}_{run_id}")).join(test_name)
}

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvLockGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
}

fn lock_env() -> io::Result<EnvLockGuard> {
    let guard = ENV_LOCK.lock().map_err(|_| io::Error::other("env lock poisoned"))?;
    Ok(EnvLockGuard {
        _guard: guard,
    })
}

/// Artifact manager for a single system-test.
#[derive(Debug)]
pub struct TestArtifacts {
    root: PathBuf,
}

impl TestArtifacts {
    /// Creates the artifact root for a test.
    pub fn new(test_name: &str) -> io::Result<Self> {
        let config = SystemTestConfig::load().map_err(io::Error::other)?;
        let root = config.run_root.unwrap_or_else(|| default_run_root(test_name));
        if root.exists() && !config.allow_overwrite {
            if !root.is_dir() {
                return Err(io::Error::other(format!(
                    "system-test run root exists and is not a directory: {}",
                    root.display()
                )));
            }
            let mut entries = fs::read_dir(&root)?;
            if entries.next().is_some() {
                return Err(io::Error::other(format!(
                    "system-test run root already exists: {} (set \
                     DECISION_GATE_SYSTEM_TEST_ALLOW_OVERWRITE=1 to reuse)",
                    root.display()
                )));
            }
        }
        fs::create_dir_all(&root)?;
        let marker = root.join(".system-test-run");
        if !marker.exists() {
            fs::write(&marker, b"decision-gate system-test run root\n")?;
        }
        Ok(Self {
            root,
        })
    }

    /// Returns the root directory for the test artifacts.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns a runpack directory path for this test.
    pub fn runpack_dir(&self) -> PathBuf {
        self.root.join("runpack")
    }

    /// Writes a JSON artifact using canonical JCS serialization.
    pub fn write_json<T: Serialize>(&self, name: &str, value: &T) -> io::Result<PathBuf> {
        let path = self.root.join(name);
        let bytes = serde_jcs::to_vec(value).map_err(|err| io::Error::other(err.to_string()))?;
        fs::write(&path, bytes)?;
        Ok(path)
    }

    /// Writes a text artifact with UTF-8 encoding.
    pub fn write_text(&self, name: &str, value: &str) -> io::Result<PathBuf> {
        let path = self.root.join(name);
        fs::write(&path, value.as_bytes())?;
        Ok(path)
    }
}

/// Helper that writes summaries even when a test panics.
pub struct TestReporter {
    artifacts: TestArtifacts,
    test_name: String,
    started_at: Instant,
    finalized: bool,
    _env_guard: EnvLockGuard,
}

impl TestReporter {
    /// Creates a reporter for the named test.
    pub fn new(test_name: &str) -> io::Result<Self> {
        let env_guard = lock_env()?;
        Ok(Self {
            artifacts: TestArtifacts::new(test_name)?,
            test_name: test_name.to_string(),
            started_at: Instant::now(),
            finalized: false,
            _env_guard: env_guard,
        })
    }

    /// Returns the artifact manager.
    pub const fn artifacts(&self) -> &TestArtifacts {
        &self.artifacts
    }

    /// Writes the final summary for the test.
    #[allow(clippy::print_stderr, reason = "Failure summaries are emitted for system-test triage.")]
    pub fn finish(
        &mut self,
        status: &str,
        notes: Vec<String>,
        artifacts: Vec<String>,
    ) -> io::Result<()> {
        let ended_at_ms = self.started_at.elapsed().as_millis();
        let summary = TestSummary {
            test_name: self.test_name.clone(),
            status: status.to_string(),
            started_at_ms: 0,
            ended_at_ms,
            duration_ms: ended_at_ms,
            notes,
            artifacts,
        };
        self.artifacts.write_json("summary.json", &summary)?;
        self.artifacts.write_text("summary.md", &summary_markdown(&summary))?;
        self.finalized = true;
        if status != "pass" {
            let notes_summary = summary.notes.join("; ");
            eprintln!(
                "[system-tests] {} status={} notes={} artifacts_root={}",
                self.test_name,
                status,
                notes_summary,
                self.artifacts.root().display()
            );
        }
        Ok(())
    }
}

impl Drop for TestReporter {
    fn drop(&mut self) {
        if self.finalized {
            return;
        }
        let status = if std::thread::panicking() { "panic" } else { "unknown" };
        let _ = self.finish(
            status,
            vec!["test terminated without explicit summary".to_string()],
            Vec::new(),
        );
    }
}

fn summary_markdown(summary: &TestSummary) -> String {
    let mut out = String::new();
    out.push_str("# System-Test Summary\n\n");
    out.push_str("## Status\n\n");
    let _ = writeln!(out, "- Test: {}", summary.test_name);
    let _ = writeln!(out, "- Status: {}", summary.status);
    let _ = writeln!(out, "- Duration (ms): {}", summary.duration_ms);
    out.push_str("\n## Notes\n\n");
    if summary.notes.is_empty() {
        out.push_str("- None\n");
    } else {
        for note in &summary.notes {
            let _ = writeln!(out, "- {note}");
        }
    }
    out.push_str("\n## Artifacts\n\n");
    if summary.artifacts.is_empty() {
        out.push_str("- None\n");
    } else {
        for artifact in &summary.artifacts {
            let _ = writeln!(out, "- {artifact}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use system_tests::config::SystemTestEnv;
    use tempfile::TempDir;

    use super::*;
    use crate::helpers::env;

    struct EnvGuard {
        entries: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new(names: &[&'static str]) -> Self {
            let entries = names.iter().map(|name| (*name, std::env::var(*name).ok())).collect();
            Self {
                entries,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in self.entries.drain(..) {
                match value {
                    Some(value) => env::set_var(name, &value),
                    None => env::remove_var(name),
                }
            }
        }
    }

    #[test]
    fn run_root_fails_closed_when_marker_exists() -> Result<(), Box<dyn std::error::Error>> {
        let _lock = super::lock_env()?;
        let _guard = EnvGuard::new(&[
            SystemTestEnv::RunRoot.as_str(),
            SystemTestEnv::AllowOverwrite.as_str(),
        ]);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().join("run_root");
        fs::create_dir_all(&root)?;
        fs::write(root.join(".system-test-run"), "existing\n")?;

        env::set_var(SystemTestEnv::RunRoot.as_str(), root.to_string_lossy().as_ref());
        env::remove_var(SystemTestEnv::AllowOverwrite.as_str());

        match TestArtifacts::new("fail_closed") {
            Err(err) => {
                let message = err.to_string();
                if !message.contains("system-test run root already exists") {
                    return Err(io::Error::other(format!(
                        "expected fail-closed error, got {message}"
                    ))
                    .into());
                }
            }
            Ok(_) => {
                return Err(io::Error::other("expected fail-closed error").into());
            }
        }
        Ok(())
    }

    #[test]
    fn run_root_fails_closed_when_directory_not_empty() -> Result<(), Box<dyn std::error::Error>> {
        let _lock = super::lock_env()?;
        let _guard = EnvGuard::new(&[
            SystemTestEnv::RunRoot.as_str(),
            SystemTestEnv::AllowOverwrite.as_str(),
        ]);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().join("run_root");
        fs::create_dir_all(&root)?;
        fs::write(root.join("orphan.txt"), "leftover\n")?;

        env::set_var(SystemTestEnv::RunRoot.as_str(), root.to_string_lossy().as_ref());
        env::remove_var(SystemTestEnv::AllowOverwrite.as_str());

        match TestArtifacts::new("fail_closed_non_empty") {
            Err(err) => {
                let message = err.to_string();
                if !message.contains("system-test run root already exists") {
                    return Err(io::Error::other(format!(
                        "expected fail-closed error, got {message}"
                    ))
                    .into());
                }
            }
            Ok(_) => {
                return Err(io::Error::other("expected fail-closed error").into());
            }
        }
        Ok(())
    }

    #[test]
    fn run_root_allows_overwrite_when_flag_set() -> Result<(), Box<dyn std::error::Error>> {
        let _lock = super::lock_env()?;
        let _guard = EnvGuard::new(&[
            SystemTestEnv::RunRoot.as_str(),
            SystemTestEnv::AllowOverwrite.as_str(),
        ]);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().join("run_root");
        fs::create_dir_all(&root)?;
        fs::write(root.join(".system-test-run"), "existing\n")?;

        env::set_var(SystemTestEnv::RunRoot.as_str(), root.to_string_lossy().as_ref());
        env::set_var(SystemTestEnv::AllowOverwrite.as_str(), "1");

        let artifacts = TestArtifacts::new("allow_overwrite")?;
        if artifacts.root() != root.as_path() {
            return Err(io::Error::other("run root mismatch after overwrite").into());
        }
        Ok(())
    }
}
