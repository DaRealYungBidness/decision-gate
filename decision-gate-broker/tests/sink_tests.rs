// decision-gate-broker/tests/sink_tests.rs
// ============================================================================
// Module: Sink Tests Entry Point
// Description: Entry point for nested sink test modules.
// ============================================================================

//! Sink unit tests.

#![allow(clippy::unwrap_used, reason = "Tests use unwrap on deterministic fixtures.")]
#![allow(clippy::expect_used, reason = "Tests use expect for explicit failure messages.")]
#![allow(dead_code, reason = "Common module may have unused helpers.")]

mod common;

#[path = "sinks/callback_tests.rs"]
mod callback_tests;

#[path = "sinks/channel_tests.rs"]
mod channel_tests;

#[path = "sinks/log_tests.rs"]
mod log_tests;
