// ret-logic/src/tests.rs
// ============================================================================
// Module: Requirement Logic Test Lint Configuration
// Description: Shared test-only lint relaxations for ret-logic unit tests.
// Purpose: Allow panic-based assertions and debug output in tests.
// Dependencies: ret-logic
// ============================================================================

//! ## Overview
//! Provides test-only lint relaxations for requirement logic unit tests.

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
