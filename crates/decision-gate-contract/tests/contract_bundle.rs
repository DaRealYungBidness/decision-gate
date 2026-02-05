// crates/decision-gate-contract/tests/contract_bundle.rs
// ============================================================================
// Module: Contract Bundle Tests
// Description: Tests for deterministic contract bundle generation.
// Purpose: Validate stable outputs and verification workflow.
// Dependencies: decision-gate-contract, tempfile
// ============================================================================

//! ## Overview
//! These tests ensure contract generation is deterministic and that the
//! verification routine succeeds against freshly generated artifacts.
//! Security posture: tests validate deterministic contract artifacts; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_contract::ContractBuilder;

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Ensures contract bundle generation is deterministic.
#[test]
fn contract_bundle_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    let builder = ContractBuilder::default();
    let first = builder.build()?;
    let second = builder.build()?;
    if first != second {
        return Err("contract bundle is not deterministic".into());
    }
    Ok(())
}

/// Ensures generated artifacts can be verified in place.
#[test]
fn contract_bundle_verifies() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let builder = ContractBuilder::new(temp.path().to_path_buf());
    builder.write()?;
    builder.verify_output(temp.path())?;
    Ok(())
}
