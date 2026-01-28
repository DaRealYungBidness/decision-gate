// enterprise-system-tests/tests/helpers/artifacts.rs
// ============================================================================
// Module: Enterprise Test Artifacts
// Description: Artifact helpers for enterprise system-tests.
// Purpose: Create per-test run roots and write deterministic summaries.
// Dependencies: serde, serde_jcs
// ============================================================================

use std::fmt::Write;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;

use serde::Serialize;
use serde_jcs;

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

fn default_run_root(test_name: &str) -> PathBuf {
    static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);
    let run_id = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    PathBuf::from("target/enterprise-system-tests").join(format!("run_{run_id}")).join(test_name)
}

fn resolve_run_root(test_name: &str) -> PathBuf {
    if let Ok(value) = std::env::var("DECISION_GATE_ENTERPRISE_SYSTEM_TEST_RUN_ROOT") {
        return PathBuf::from(value);
    }
    if let Ok(value) = std::env::var("DECISION_GATE_SYSTEM_TEST_RUN_ROOT") {
        return PathBuf::from(value);
    }
    default_run_root(test_name)
}

/// Artifact manager for a single enterprise system-test.
#[derive(Debug, Clone)]
pub struct TestArtifacts {
    root: PathBuf,
}

impl TestArtifacts {
    /// Creates the artifact root for a test.
    pub fn new(test_name: &str) -> io::Result<Self> {
        let root = resolve_run_root(test_name);
        fs::create_dir_all(&root)?;
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
}

impl TestReporter {
    /// Creates a reporter for the named test.
    pub fn new(test_name: &str) -> io::Result<Self> {
        Ok(Self {
            artifacts: TestArtifacts::new(test_name)?,
            test_name: test_name.to_string(),
            started_at: Instant::now(),
            finalized: false,
        })
    }

    /// Returns the artifact manager.
    pub const fn artifacts(&self) -> &TestArtifacts {
        &self.artifacts
    }

    /// Writes the final summary for the test.
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
    out.push_str("# Enterprise System-Test Summary\n\n");
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
