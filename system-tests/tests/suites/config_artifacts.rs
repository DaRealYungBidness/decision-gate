// system-tests/tests/suites/config_artifacts.rs
// =============================================================================
// Module: Config Artifact Tests
// Description: Validate generated config docs/schema/examples match outputs.
// Purpose: Prevent drift between canonical config and committed artifacts.
// Dependencies: decision-gate-config, serde_jcs
// =============================================================================

//! ## Overview
//! Validate generated config docs/schema/examples match outputs.
//! Purpose: Prevent drift between canonical config and committed artifacts.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use decision_gate_config::config_docs_markdown;
use decision_gate_config::config_schema;
use decision_gate_config::config_toml_example;
use serde_json::Value;

type TestResult = Result<(), String>;

fn repo_root() -> Result<PathBuf, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.parent().map(Path::to_path_buf).ok_or_else(|| "repo root not found".to_string())
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n")
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let canonical =
        serde_jcs::to_vec(value).map_err(|error| format!("canonical json failed: {error}"))?;
    let canonical_value: Value = serde_json::from_slice(&canonical)
        .map_err(|error| format!("canonical value failed: {error}"))?;
    let mut bytes = serde_json::to_vec_pretty(&canonical_value)
        .map_err(|error| format!("pretty json failed: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[test]
fn config_docs_match_generated_output() -> TestResult {
    let root = repo_root()?;
    let doc_path = root.join("Docs/configuration/decision-gate.toml.md");
    let actual =
        fs::read_to_string(doc_path).map_err(|error| format!("read doc failed: {error}"))?;
    let expected =
        config_docs_markdown().map_err(|error| format!("generate doc failed: {error}"))?;
    if normalize_newlines(&actual) != normalize_newlines(&expected) {
        return Err("config docs drifted from generated output".to_string());
    }
    Ok(())
}

#[test]
fn config_schema_matches_generated_output() -> TestResult {
    let root = repo_root()?;
    let schema_path = root.join("Docs/generated/decision-gate/schemas/config.schema.json");
    let actual = fs::read(schema_path).map_err(|error| format!("read schema failed: {error}"))?;
    let expected = canonical_json_bytes(&config_schema())?;
    if actual != expected {
        return Err("config schema drifted from generated output".to_string());
    }
    Ok(())
}

#[test]
fn config_example_matches_generated_output() -> TestResult {
    let root = repo_root()?;
    let example_path = root.join("Docs/generated/decision-gate/examples/decision-gate.toml");
    let actual = fs::read_to_string(example_path)
        .map_err(|error| format!("read example failed: {error}"))?;
    let expected = config_toml_example();
    if normalize_newlines(&actual) != normalize_newlines(&expected) {
        return Err("config example drifted from generated output".to_string());
    }
    Ok(())
}
