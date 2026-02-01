// decision-gate-cli/src/tests.rs
// ============================================================================
// Module: CLI Test Lint Configuration
// Description: Shared test-only lint relaxations for CLI unit tests.
// Purpose: Allow panic-based assertions and debug output in tests.
// Dependencies: decision-gate-cli
// ============================================================================

//! ## Overview
//! Provides test-only lint relaxations for CLI unit tests.

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

// ============================================================================
// SECTION: Modules
// ============================================================================

mod auth;
mod i18n;
mod interop;
mod mcp_client;
mod protocol;
mod resource_limits;
mod serve_policy;
mod support;
mod timing;
