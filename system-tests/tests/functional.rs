// system-tests/tests/functional.rs
// ============================================================================
// Module: Functional Suite
// Description: Aggregates strict validation functional tests.
// Purpose: Reduce binaries while keeping functional coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates strict validation functional tests.
//! Purpose: Reduce binaries while keeping functional coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/cli_conformance.rs"]
mod cli_conformance;
#[path = "suites/docs_search.rs"]
mod docs_search;
#[path = "suites/json_evidence.rs"]
mod json_evidence;
#[path = "suites/precheck.rs"]
mod precheck;
#[path = "suites/ret_logic_authoring.rs"]
mod ret_logic_authoring;
#[path = "suites/validation.rs"]
mod validation;
