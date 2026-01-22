// decision-gate-broker/tests/sink_tests.rs
// ============================================================================
// Module: Sink Tests Entry Point
// Description: Entry point for nested sink test modules.
// ============================================================================

//! Sink unit tests.

#![allow(dead_code, reason = "Common module may have unused helpers.")]
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

mod common;

#[path = "sinks/callback_tests.rs"]
mod callback_tests;

#[path = "sinks/channel_tests.rs"]
mod channel_tests;

#[path = "sinks/log_tests.rs"]
mod log_tests;
