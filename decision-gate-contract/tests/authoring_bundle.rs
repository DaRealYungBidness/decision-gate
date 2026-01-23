// decision-gate-contract/tests/authoring_bundle.rs
// ============================================================================
// Module: Authoring Bundle Tests
// Description: Validate generated authoring artifacts normalize correctly.
// Purpose: Ensure scenario.ron normalizes to canonical scenario.json.
// Dependencies: decision-gate-contract, decision-gate-core, serde_json
// ============================================================================

//! Contract bundle authoring tests.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    clippy::missing_docs_in_private_items,
    reason = "Test-only authoring validation uses panic-based assertions."
)]

use decision_gate_contract::AuthoringFormat;
use decision_gate_contract::ContractBuilder;
use decision_gate_contract::authoring::normalize_scenario;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::hashing::canonical_json_bytes;

fn artifact_bytes(
    bundle: &decision_gate_contract::ContractBundle,
    path: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    for artifact in &bundle.artifacts {
        if artifact.path == path {
            return Ok(artifact.bytes.clone());
        }
    }
    Err(format!("artifact not found: {path}").into())
}

#[test]
fn ron_example_normalizes_to_canonical_json_example() -> Result<(), Box<dyn std::error::Error>> {
    let bundle = ContractBuilder::default().build()?;
    let ron_bytes = artifact_bytes(&bundle, "examples/scenario.ron")?;
    let json_bytes = artifact_bytes(&bundle, "examples/scenario.json")?;

    let ron_input = std::str::from_utf8(&ron_bytes)?;
    let normalized = normalize_scenario(ron_input, AuthoringFormat::Ron)?;

    let spec: ScenarioSpec = serde_json::from_slice(&json_bytes)?;
    let expected = canonical_json_bytes(&spec)?;
    assert_eq!(normalized.canonical_json, expected);
    Ok(())
}
