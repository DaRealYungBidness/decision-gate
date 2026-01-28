// system-tests/tests/golden_runpacks.rs
// ============================================================================
// Module: Golden Runpack Suite
// Description: Aggregates golden runpack determinism tests.
// Purpose: Enforce cross-OS deterministic runpack exports.
// Dependencies: suites/*, helpers
// ============================================================================

//! Golden runpack suite entry point for system-tests.

mod helpers;

#[path = "suites/golden_runpacks.rs"]
mod golden_runpacks;
