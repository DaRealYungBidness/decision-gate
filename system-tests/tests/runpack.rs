// system-tests/tests/runpack.rs
// ============================================================================
// Module: Runpack Suite
// Description: Aggregates runpack export/verify system tests.
// Purpose: Reduce binaries while keeping runpack coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates runpack export/verify system tests.
//! Purpose: Reduce binaries while keeping runpack coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/runpack.rs"]
mod runpack;
