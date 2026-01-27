// enterprise/enterprise-system-tests/tests/runpack.rs
// ============================================================================
// Module: Runpack Suite
// Description: Aggregates enterprise runpack hardening system tests.
// Purpose: Reduce binaries while keeping runpack coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Runpack suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/runpack_hardening.rs"]
mod runpack_hardening;
