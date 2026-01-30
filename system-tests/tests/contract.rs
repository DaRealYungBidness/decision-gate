// system-tests/tests/contract.rs
// ============================================================================
// Module: Contract Suite
// Description: Aggregates contract conformance and discovery system tests.
// Purpose: Reduce binaries while keeping contract coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Contract suite entry point for system-tests.

mod helpers;

#[path = "suites/config_artifacts.rs"]
mod config_artifacts;
#[path = "suites/contract.rs"]
mod contract;
#[path = "suites/provider_discovery.rs"]
mod provider_discovery;
