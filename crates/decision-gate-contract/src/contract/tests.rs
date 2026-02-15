// crates/decision-gate-contract/src/contract/tests.rs
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
use std::io;
use std::path::Path;

use crate::ContractBuilder;
use crate::ContractError;

/// Creates a symlink to a file target.
#[cfg(unix)]
fn create_file_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

/// Creates a symlink to a file target.
#[cfg(windows)]
fn create_file_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(src, dst)
}

/// Creates a symlink to a directory target.
#[cfg(unix)]
fn create_dir_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

/// Creates a symlink to a directory target.
#[cfg(windows)]
fn create_dir_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(src, dst)
}

/// Returns true when symlink creation failures should be treated as skip.
fn symlink_error_is_skip(err: &io::Error) -> bool {
    matches!(err.kind(), io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported)
}

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

#[test]
fn write_rejects_symlinked_output_dir() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let output_dir = temp.path().join("out");
    fs::create_dir_all(&output_dir)?;

    let link = temp.path().join("out-link");
    if let Err(err) = create_dir_symlink(&output_dir, &link) {
        if symlink_error_is_skip(&err) {
            return Ok(());
        }
        return Err(err.into());
    }

    let builder = ContractBuilder::new(link.clone());
    let Err(err) = builder.write_to(&link) else {
        return Err("expected symlinked output dir to be rejected".into());
    };
    if !matches!(err, ContractError::OutputPath(_)) {
        return Err("expected OutputPath error for symlinked output dir".into());
    }

    Ok(())
}

#[test]
fn verify_output_rejects_symlinked_output_dir() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let output_dir = temp.path().join("out");
    let builder = ContractBuilder::new(output_dir.clone());
    builder.write()?;

    let link = temp.path().join("out-link");
    if let Err(err) = create_dir_symlink(&output_dir, &link) {
        if symlink_error_is_skip(&err) {
            return Ok(());
        }
        return Err(err.into());
    }

    let Err(err) = builder.verify_output(&link) else {
        return Err("expected symlinked output dir to be rejected".into());
    };
    if !matches!(err, ContractError::OutputPath(_)) {
        return Err("expected OutputPath error for symlinked output dir".into());
    }

    Ok(())
}

#[test]
fn verify_output_rejects_symlinked_artifact_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let output_dir = temp.path().join("out");
    let builder = ContractBuilder::new(output_dir.clone());
    builder.write()?;

    let bundle = builder.build()?;
    let artifact = bundle.artifacts.first().ok_or("expected at least one artifact")?;
    let artifact_path = output_dir.join(&artifact.path);
    let replacement = temp.path().join("replacement.txt");
    fs::write(&replacement, b"replacement")?;
    fs::remove_file(&artifact_path)?;

    if let Err(err) = create_file_symlink(&replacement, &artifact_path) {
        if symlink_error_is_skip(&err) {
            return Ok(());
        }
        return Err(err.into());
    }

    let Err(err) = builder.verify_output(&output_dir) else {
        return Err("expected symlinked artifact file to be rejected".into());
    };
    if !matches!(err, ContractError::OutputPath(_)) {
        return Err("expected OutputPath error for symlinked artifact".into());
    }

    Ok(())
}
