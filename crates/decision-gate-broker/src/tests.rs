// crates/decision-gate-broker/src/tests.rs
// ============================================================================
// Module: Broker Test Lint Configuration
// Description: Shared test-only lint relaxations for broker unit tests.
// Purpose: Allow panic-based assertions and debug output in tests.
// Dependencies: decision-gate-broker
// ============================================================================

//! ## Overview
//! Provides test-only lint relaxations for Decision Gate broker unit tests.

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
