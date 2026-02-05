//! Config load validation tests for decision-gate-config.
// crates/decision-gate-config/tests/load_validation.rs
// =============================================================================
// Module: Config Load Validation Tests
// Description: Validate config loading guards (path, size, encoding).
// Purpose: Ensure config input handling is strict and fail-closed.
// =============================================================================

use std::io::Write;
use std::path::Path;

use decision_gate_config::ConfigError;
use decision_gate_config::DecisionGateConfig;
use tempfile::NamedTempFile;

type TestResult = Result<(), String>;

fn assert_invalid(result: Result<DecisionGateConfig, ConfigError>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            let message = error.to_string();
            if message.contains(needle) {
                Ok(())
            } else {
                Err(format!("error {message} did not contain {needle}"))
            }
        }
        Ok(_) => Err("expected invalid config load".to_string()),
    }
}

#[test]
fn load_rejects_path_too_long() -> TestResult {
    let long_path = "a".repeat(5_000);
    let path = Path::new(&long_path);
    assert_invalid(DecisionGateConfig::load(Some(path)), "config path exceeds max length")?;
    Ok(())
}

#[test]
fn load_rejects_path_component_too_long() -> TestResult {
    let long_component = "a".repeat(300);
    let path = Path::new(&long_component);
    assert_invalid(DecisionGateConfig::load(Some(path)), "config path component too long")?;
    Ok(())
}

#[test]
fn load_rejects_oversized_file() -> TestResult {
    let mut file = NamedTempFile::new().map_err(|err| err.to_string())?;
    let payload = vec![b'a'; 1_048_577];
    file.write_all(&payload).map_err(|err| err.to_string())?;
    assert_invalid(DecisionGateConfig::load(Some(file.path())), "config file exceeds size limit")?;
    Ok(())
}

#[test]
fn load_rejects_non_utf8_file() -> TestResult {
    let mut file = NamedTempFile::new().map_err(|err| err.to_string())?;
    file.write_all(&[0xFF, 0xFE, 0xFF]).map_err(|err| err.to_string())?;
    assert_invalid(DecisionGateConfig::load(Some(file.path())), "config file must be utf-8")?;
    Ok(())
}
