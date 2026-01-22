// ret-logic/tests/support/mod.rs
// ============================================================================
// Module: Test Support
// Description: Shared result helpers for requirement integration tests.
// ============================================================================
//! ## Overview
//! Shared test helpers for consistent Result-based assertions.

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

use std::error::Error;
use std::fmt;

// ========================================================================
// Test Result Helpers
// ========================================================================

/// Standard result type used across requirement integration tests.
pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Lightweight error type for test assertions.
#[derive(Debug)]
struct TestError {
    /// Human-readable failure message.
    message: String,
}

impl TestError {
    /// Creates a new test error with the provided message.
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for TestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for TestError {}

/// Returns an error when a test condition fails.
///
/// # Errors
/// Returns a `TestError` when the condition is false.
pub fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition { Ok(()) } else { Err(Box::new(TestError::new(message))) }
}
