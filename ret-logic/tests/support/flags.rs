// ret-logic/tests/support/flags.rs
// ============================================================================
// Module: Flag Constants
// Description: Shared flag constants for requirement tests.
// ============================================================================
//! ## Overview
//! Flag constants shared by requirement integration tests.

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

/// Flag representing capability A for tests.
pub const FLAG_A: u64 = 0b0001;
/// Flag representing capability B for tests.
pub const FLAG_B: u64 = 0b0010;
/// Flag representing capability C for tests.
pub const FLAG_C: u64 = 0b0100;
/// Combination of `FLAG_A` and `FLAG_B`.
pub const FLAG_AB: u64 = FLAG_A | FLAG_B;
