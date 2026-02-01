// decision-gate-contract/src/contract/tests.rs
// ============================================================================
// Module: Contract Builder Unit Tests
// Description: Unit coverage for contract output safety checks.
// Purpose: Ensure verification fails closed on unsafe output layouts.
// Dependencies: decision-gate-contract, tempfile, std
// ============================================================================

//! ## Overview
//! Tests defensive behaviors around contract artifact verification, including
//! size mismatch detection and symlink rejection.
//!
//! Security posture: tests validate fail-closed handling of untrusted artifact
//! directories; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;

use crate::ContractBuilder;
use crate::ContractError;

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn verify_output_rejects_size_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let output_dir = temp.path().join("out");
    let builder = ContractBuilder::new(output_dir.clone());
    builder.write()?;

    let bundle = builder.build()?;
    let artifact = bundle.artifacts.first().ok_or("expected at least one artifact")?;
    let artifact_path = output_dir.join(&artifact.path);
    let mut bytes = fs::read(&artifact_path)?;
    bytes.extend_from_slice(b"extra");
    fs::write(&artifact_path, &bytes)?;

    let Err(err) = builder.verify_output(&output_dir) else {
        return Err("expected size mismatch to be rejected".into());
    };
    if !matches!(err, ContractError::Generation(_)) {
        return Err("expected generation error for size mismatch".into());
    }

    Ok(())
}

#[cfg(unix)]
#[test]
fn verify_output_rejects_symlinked_output_dir() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let output_dir = temp.path().join("out");
    let builder = ContractBuilder::new(output_dir.clone());
    builder.write()?;

    let link = temp.path().join("out-link");
    symlink(&output_dir, &link)?;

    let Err(err) = builder.verify_output(&link) else {
        return Err("expected symlinked output dir to be rejected".into());
    };
    if !matches!(err, ContractError::OutputPath(_)) {
        return Err("expected OutputPath error for symlinked output dir".into());
    }

    Ok(())
}
