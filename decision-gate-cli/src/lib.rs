// decision-gate-cli/src/lib.rs
// ============================================================================
// Module: Decision Gate CLI Library
// Description: Shared helpers for the Decision Gate command-line interface.
// Purpose: Provide reusable components (i18n) for the CLI binary and tests.
// Dependencies: Standard library.
// ============================================================================

//! ## Overview
//! This library module houses shared CLI utilities, including the internationalized
//! message catalog. The binary entry point (`src/main.rs`) imports these helpers
//! to keep all user-facing output consistent.
//!
//! Security posture: CLI inputs are untrusted and must be validated; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

/// Internationalization helpers and message catalog.
pub mod i18n;

#[allow(dead_code, reason = "MCP client helpers are shared by CLI and interop tests.")]
pub(crate) mod mcp_client;

#[allow(dead_code, reason = "Interop helpers are used by the CLI binary and tests.")]
pub mod interop;

/// Security utilities (constant-time comparisons).
pub mod security;

/// CLI server bind policy helpers.
pub mod serve_policy;

#[cfg(test)]
mod tests;
