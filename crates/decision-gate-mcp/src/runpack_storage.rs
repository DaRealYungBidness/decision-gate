// crates/decision-gate-mcp/src/runpack_storage.rs
// ============================================================================
// Module: Runpack Storage
// Description: Optional runpack storage backend integration for managed cloud.
// Purpose: Allow MCP to export runpacks to object storage via pluggable sinks.
// Dependencies: decision-gate-core, thiserror
// ============================================================================

//! ## Overview
//! Runpack storage integration for MCP exports.
//!
//! This module provides a pluggable interface for exporting runpacks to
//! managed storage backends (for example, S3) without coupling MCP to
//! enterprise storage crates.
//!
//! Security posture: storage backends are untrusted; runpack integrity must be
//! verified on read and export errors must fail closed. See
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::path::Path;

use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::TenantId;
use thiserror::Error;

/// Runpack storage key (tenant + namespace + run).
///
/// # Invariants
/// - Identifiers scope a single runpack export operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpackStorageKey {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
}

/// Runpack storage errors.
///
/// # Invariants
/// - Variants are stable for error classification.
#[derive(Debug, Error)]
pub enum RunpackStorageError {
    /// Storage I/O error.
    #[error("runpack storage io error: {0}")]
    Io(String),
    /// Storage request invalid.
    #[error("runpack storage invalid request: {0}")]
    Invalid(String),
    /// Storage backend failed.
    #[error("runpack storage backend error: {0}")]
    Backend(String),
}

/// Runpack storage backend.
pub trait RunpackStorage: Send + Sync {
    /// Stores a runpack directory and returns an optional storage URI.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackStorageError`] when validation or storage fails.
    fn store_runpack(
        &self,
        key: &RunpackStorageKey,
        source_dir: &Path,
    ) -> Result<Option<String>, RunpackStorageError>;
}
