// enterprise/enterprise-system-tests/tests/config.rs
// ============================================================================
// Module: Config Suite
// Description: Aggregates enterprise config wiring and limits tests.
// Purpose: Reduce binaries while keeping config coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Config suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/config_limits.rs"]
mod config_limits;
#[path = "suites/config_wiring.rs"]
mod config_wiring;
