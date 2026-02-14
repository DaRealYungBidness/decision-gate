// system-tests/tests/performance.rs
// ============================================================================
// Module: Performance Suite
// Description: Aggregates performance smoke system tests.
// Purpose: Reduce binaries while keeping performance coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates performance smoke system tests.
//! Purpose: Reduce binaries while keeping performance coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/performance.rs"]
mod performance;
