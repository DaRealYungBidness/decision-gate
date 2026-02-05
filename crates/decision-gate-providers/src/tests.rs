// crates/decision-gate-providers/src/tests.rs
// ============================================================================
// Module: Providers Test Lint Configuration
// Description: Shared test-only lint relaxations for provider unit tests.
// Purpose: Allow panic-based assertions and debug output in tests.
// Dependencies: decision-gate-providers
// ============================================================================

//! ## Overview
//! Provides test-only lint relaxations for provider unit tests.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

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
