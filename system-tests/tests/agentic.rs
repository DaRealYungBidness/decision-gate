// system-tests/tests/agentic.rs
// ============================================================================
// Module: Agentic Harness Suite
// Description: End-to-end agentic flow harness tests.
// Purpose: Execute canonical agentic scenarios across projections.
// Dependencies: suites/agentic_harness.rs, helpers
// ============================================================================

//! ## Overview
//! End-to-end agentic flow harness tests.
//! Purpose: Execute canonical agentic scenarios across projections.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/agentic_harness.rs"]
mod agentic_harness;
