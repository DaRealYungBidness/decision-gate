// system-tests/tests/functional.rs
// ============================================================================
// Module: Functional Suite
// Description: Aggregates strict validation functional tests.
// Purpose: Reduce binaries while keeping functional coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Functional suite entry point for system-tests.

mod helpers;

#[path = "suites/json_evidence.rs"]
mod json_evidence;
#[path = "suites/validation.rs"]
mod validation;
