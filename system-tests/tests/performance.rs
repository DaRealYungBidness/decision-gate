// system-tests/tests/performance.rs
// ============================================================================
// Module: Performance Suite
// Description: Aggregates performance smoke system tests.
// Purpose: Reduce binaries while keeping performance coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Performance suite entry point for system-tests.

mod helpers;

#[path = "suites/performance.rs"]
mod performance;
