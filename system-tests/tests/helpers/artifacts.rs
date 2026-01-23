// system-tests/tests/helpers/artifacts.rs
// ============================================================================
// Module: Test Artifacts
// Description: Artifact helpers for system-tests.
// Purpose: Create per-test run roots and write deterministic summaries.
// Dependencies: system-tests, serde, serde_jcs
// ============================================================================

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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

fn now_millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

fn default_run_root(test_name: &str) -> PathBuf {
    let stamp = now_millis();
    PathBuf::from("target/system-tests").join(format!("run_{stamp}")).join(test_name)
}

/// Artifact manager for a single system-test.
#[derive(Debug, Clone)]
pub struct TestArtifacts {
    root: PathBuf,
}

impl TestArtifacts {
    /// Creates the artifact root for a test.
    pub fn new(test_name: &str) -> io::Result<Self> {
        let config =
            SystemTestConfig::load().map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let root = config.run_root.unwrap_or_else(|| default_run_root(test_name));
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
        let bytes = serde_jcs::to_vec(value)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
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
    started_at_ms: u128,
    finalized: bool,
}

impl TestReporter {
    /// Creates a reporter for the named test.
    pub fn new(test_name: &str) -> io::Result<Self> {
        Ok(Self {
            artifacts: TestArtifacts::new(test_name)?,
            test_name: test_name.to_string(),
            started_at_ms: now_millis(),
            finalized: false,
        })
    }

    /// Returns the artifact manager.
    pub fn artifacts(&self) -> &TestArtifacts {
        &self.artifacts
    }

    /// Writes the final summary for the test.
    pub fn finish(
        &mut self,
        status: &str,
        notes: Vec<String>,
        artifacts: Vec<String>,
    ) -> io::Result<()> {
        let ended_at_ms = now_millis();
        let summary = TestSummary {
            test_name: self.test_name.clone(),
            status: status.to_string(),
            started_at_ms: self.started_at_ms,
            ended_at_ms,
            duration_ms: ended_at_ms.saturating_sub(self.started_at_ms),
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
    out.push_str("# System-Test Summary\n\n");
    out.push_str("## Status\n\n");
    out.push_str(&format!("- Test: {}\n", summary.test_name));
    out.push_str(&format!("- Status: {}\n", summary.status));
    out.push_str(&format!("- Duration (ms): {}\n", summary.duration_ms));
    out.push_str("\n## Notes\n\n");
    if summary.notes.is_empty() {
        out.push_str("- None\n");
    } else {
        for note in &summary.notes {
            out.push_str(&format!("- {}\n", note));
        }
    }
    out.push_str("\n## Artifacts\n\n");
    if summary.artifacts.is_empty() {
        out.push_str("- None\n");
    } else {
        for artifact in &summary.artifacts {
            out.push_str(&format!("- {}\n", artifact));
        }
    }
    out
}
