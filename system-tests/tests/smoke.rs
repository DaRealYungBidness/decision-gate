// system-tests/tests/smoke.rs
// ============================================================================
// Module: Smoke Suite
// Description: Aggregates smoke system tests into one binary.
// Purpose: Reduce binaries while keeping smoke coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Smoke suite entry point for system-tests.

mod helpers;

#[path = "suites/smoke.rs"]
mod smoke;
