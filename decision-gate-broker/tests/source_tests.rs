// decision-gate-broker/tests/source_tests.rs
// ============================================================================
// Module: Source Tests Entry Point
// Description: Entry point for nested source test modules.
// ============================================================================

//! Source unit tests.

#![allow(clippy::unwrap_used, reason = "Tests use unwrap on deterministic fixtures.")]
#![allow(clippy::expect_used, reason = "Tests use expect for explicit failure messages.")]
#![allow(dead_code, reason = "Common module may have unused helpers.")]

mod common;

#[path = "sources/file_tests.rs"]
mod file_tests;

#[path = "sources/http_tests.rs"]
mod http_tests;

#[path = "sources/inline_tests.rs"]
mod inline_tests;
