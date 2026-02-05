// crates/decision-gate-broker/tests/source_tests.rs
// ============================================================================
// Module: Source Tests Entry Point
// Description: Entry point for nested source test modules.
// Purpose: Wire source test modules and shared helpers.
// Dependencies: decision-gate-broker
// ============================================================================

//! ## Overview
//! Aggregates source-focused test modules for the broker crate.

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

#[path = "sources/file_tests.rs"]
mod file_tests;

#[path = "sources/http_tests.rs"]
mod http_tests;

#[path = "sources/inline_tests.rs"]
mod inline_tests;
