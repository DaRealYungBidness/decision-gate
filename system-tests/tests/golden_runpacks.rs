// system-tests/tests/golden_runpacks.rs
// ============================================================================
// Module: Golden Runpack Suite
// Description: Aggregates golden runpack determinism tests.
// Purpose: Enforce cross-OS deterministic runpack exports.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates golden runpack determinism tests.
//! Purpose: Enforce cross-OS deterministic runpack exports.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/golden_runpacks.rs"]
mod golden_runpacks;
