// crates/decision-gate-mcp/src/tests.rs
// ============================================================================
// Module: MCP Test Lint Configuration
// Description: Shared test-only lint relaxations for the MCP crate.
// Purpose: Allow panic-based assertions and debug output in unit tests.
// Dependencies: decision-gate-mcp
// ============================================================================

//! ## Overview
//! Provides test-only lint relaxations for MCP unit tests that live in the
//! crate root module tree.

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
