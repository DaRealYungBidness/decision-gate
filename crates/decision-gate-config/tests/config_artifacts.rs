//! Config artifact validation tests for decision-gate-config.
// crates/decision-gate-config/tests/config_artifacts.rs
// ============================================================================
// Module: Config Artifact Validation Tests
// Description: Validate config schema, example, and docs generators.
// Purpose: Prevent drift between config model and generated artifacts.
// Dependencies: decision-gate-config, jsonschema, toml
// ============================================================================

use decision_gate_config::config_docs_markdown;
use decision_gate_config::config_schema;
use decision_gate_config::config_toml_example;
use jsonschema::Draft;
use serde_json::json;

type TestResult = Result<(), String>;

#[test]
fn config_schema_accepts_minimal_and_example_configs() -> TestResult {
    let schema = config_schema();
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .map_err(|err| err.to_string())?;

    let minimal = json!({});
    if !validator.is_valid(&minimal) {
        return Err("minimal config should be valid".to_string());
    }

    let toml_str = config_toml_example();
    let toml_value: toml::Value = toml::from_str(&toml_str).map_err(|err| err.to_string())?;
    let json_value = serde_json::to_value(toml_value).map_err(|err| err.to_string())?;
    if !validator.is_valid(&json_value) {
        return Err("example config should validate".to_string());
    }
    Ok(())
}

#[test]
fn config_docs_generate_without_error() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;
    if !docs.contains("# decision-gate.toml Configuration") {
        return Err("docs missing title header".to_string());
    }
    Ok(())
}
