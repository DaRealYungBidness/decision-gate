// system-tests/tests/smoke.rs
// ============================================================================
// Module: Smoke Suite
// Description: Aggregates smoke system tests into one binary.
// Purpose: Reduce binaries while keeping smoke coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates smoke system tests into one binary.
//! Purpose: Reduce binaries while keeping smoke coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/smoke.rs"]
mod smoke;
