// system-tests/tests/operations.rs
// ============================================================================
// Module: Operations Suite
// Description: Aggregates operational posture and hardening system tests.
// Purpose: Reduce binaries while keeping operations coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Operations suite entry point for system-tests.

mod helpers;

#[path = "suites/cli_workflows.rs"]
mod cli_workflows;
#[path = "suites/mcp_hardening.rs"]
mod mcp_hardening;
#[path = "suites/operations.rs"]
mod operations;
#[path = "suites/sdk_gen_cli.rs"]
mod sdk_gen_cli;
