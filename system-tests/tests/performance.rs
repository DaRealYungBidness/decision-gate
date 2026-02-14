// system-tests/tests/performance.rs
// ============================================================================
// Module: Performance Suite
// Description: Aggregates performance throughput SLO system tests.
// Purpose: Reduce binaries while keeping performance gate coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates performance throughput SLO system tests.
//! Purpose: reduce binaries while keeping performance gate coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/performance.rs"]
mod performance;

#[path = "suites/perf_sqlite_store.rs"]
mod perf_sqlite_store;
