// system-tests/tests/operations.rs
// ============================================================================
// Module: Operations Suite
// Description: Aggregates operational posture and hardening system tests.
// Purpose: Reduce binaries while keeping operations coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Operations suite entry point for system-tests.

mod helpers;

#[path = "suites/mcp_hardening.rs"]
mod mcp_hardening;
#[path = "suites/operations.rs"]
mod operations;
