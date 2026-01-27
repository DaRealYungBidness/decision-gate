//! Enterprise config limits system tests.
// enterprise-system-tests/tests/config_limits.rs
// ============================================================================
// Module: Enterprise Config Limits Tests
// Description: Validate enterprise config path and size limits.
// Purpose: Ensure config ingestion rejects oversized inputs.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::path::PathBuf;

use decision_gate_enterprise::config::EnterpriseConfig;
use helpers::artifacts::TestReporter;

#[test]
fn enterprise_config_path_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_config_path_limits")?;

    let long_path = "a".repeat(5000);
    helpers::env::set_var("DECISION_GATE_ENTERPRISE_CONFIG", &long_path);
    let err = EnterpriseConfig::load(None).expect_err("expected long path rejection");
    let message = err.to_string();
    if !message.contains("path exceeds max length") {
        return Err(format!("unexpected error for long path: {message}").into());
    }
    helpers::env::remove_var("DECISION_GATE_ENTERPRISE_CONFIG");

    let long_component = "b".repeat(300);
    let path = PathBuf::from(long_component);
    let err = EnterpriseConfig::load(Some(&path)).expect_err("expected component length rejection");
    let message = err.to_string();
    if !message.contains("path component too long") {
        return Err(format!("unexpected error for long component: {message}").into());
    }

    let temp_dir = tempfile::TempDir::new()?;
    let config_path = temp_dir.path().join("oversize.toml");
    std::fs::write(&config_path, vec![b'x'; 600 * 1024])?;
    let err = EnterpriseConfig::load(Some(&config_path)).expect_err("expected file size rejection");
    let message = err.to_string();
    if !message.contains("file exceeds size limit") {
        return Err(format!("unexpected error for size limit: {message}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["enterprise config limits enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
