// system-tests/tests/functional.rs
// ============================================================================
// Module: Functional Suite
// Description: Aggregates strict validation functional tests.
// Purpose: Reduce binaries while keeping functional coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Functional suite entry point for system-tests.

mod helpers;

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
