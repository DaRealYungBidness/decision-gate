// system-tests/tests/agentic.rs
// ============================================================================
// Module: Agentic Harness Suite
// Description: End-to-end agentic flow harness tests.
// Purpose: Execute canonical agentic scenarios across projections.
// Dependencies: suites/agentic_harness.rs, helpers
// ============================================================================

//! Agentic harness suite entry point for system-tests.

mod helpers;

#[path = "suites/agentic_harness.rs"]
mod agentic_harness;
