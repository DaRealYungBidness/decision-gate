// system-tests/tests/contract.rs
// ============================================================================
// Module: Contract Suite
// Description: Aggregates contract conformance and discovery system tests.
// Purpose: Reduce binaries while keeping contract coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates contract conformance and discovery system tests.
//! Purpose: Reduce binaries while keeping contract coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/config_artifacts.rs"]
mod config_artifacts;
#[path = "suites/contract.rs"]
mod contract;
#[path = "suites/provider_discovery.rs"]
mod provider_discovery;
