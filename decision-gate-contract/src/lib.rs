// decision-gate-contract/src/lib.rs
// ============================================================================
// Module: Decision Gate Contract Library
// Description: Canonical contract definitions and generators for Decision Gate.
// Purpose: Provide the invariant contract used to generate docs and tooling.
// Dependencies: decision-gate-core, serde, thiserror
// ============================================================================

//! ## Overview
//! The contract library defines the canonical, machine-readable contract for
//! Decision Gate. It is the single source of truth for tooling docs, schemas,
//! and provider capability metadata, following the invariance doctrine.
//!
//! Security posture: contract artifacts are inputs to external tooling and must
//! remain deterministic; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod authoring;
pub mod contract;
pub mod examples;
pub mod providers;
pub mod schemas;
pub mod tooling;
pub mod tooltips;
pub mod types;

// ============================================================================
// SECTION: Errors
// ============================================================================

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised when generating contract artifacts.
///
/// # Invariants
/// - Variants carry human-readable context for diagnostics.
/// - [`ContractError::OutputPath`] always includes the offending path.
#[derive(Debug, Error)]
pub enum ContractError {
    /// IO failure while writing artifacts.
    #[error("io error: {0}")]
    Io(String),
    /// Serialization failure while rendering artifacts.
    #[error("serialization error: {0}")]
    Serialization(String),
    /// Contract generation failed.
    #[error("contract generation error: {0}")]
    Generation(String),
    /// Output path invalid or inaccessible.
    #[error("invalid output path: {0}")]
    OutputPath(PathBuf),
}

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use authoring::AuthoringError;
pub use authoring::AuthoringFormat;
pub use authoring::NormalizedScenario;
pub use contract::ContractBuilder;
pub use types::ContractArtifact;
pub use types::ContractBundle;
pub use types::ContractManifest;
pub use types::ManifestArtifact;
pub use types::ToolName;
