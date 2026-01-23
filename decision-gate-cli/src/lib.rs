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

#[cfg(test)]
mod tests {
    //! Test-only lint relaxations for panic-based assertions and debug output.
    #![allow(
        clippy::panic,
        clippy::print_stdout,
        clippy::print_stderr,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::panic_in_result_fn,
        clippy::unwrap_in_result,
        reason = "Test-only output and panic-based assertions are permitted."
    )]
}
