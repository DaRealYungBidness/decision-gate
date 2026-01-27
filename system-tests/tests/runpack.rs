// system-tests/tests/runpack.rs
// ============================================================================
// Module: Runpack Suite
// Description: Aggregates runpack export/verify system tests.
// Purpose: Reduce binaries while keeping runpack coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Runpack suite entry point for system-tests.

mod helpers;

#[path = "suites/runpack.rs"]
mod runpack;
