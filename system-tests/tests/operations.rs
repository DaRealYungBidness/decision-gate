// system-tests/tests/operations.rs
// ============================================================================
// Module: Operations Suite
// Description: Aggregates operational posture and hardening system tests.
// Purpose: Reduce binaries while keeping operations coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates operational posture and hardening system tests.
//! Purpose: Reduce binaries while keeping operations coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/broker_integration.rs"]
mod broker_integration;
#[path = "suites/cli_golden_outputs.rs"]
mod cli_golden_outputs;
#[path = "suites/cli_limits.rs"]
mod cli_limits;
#[path = "suites/cli_workflows.rs"]
mod cli_workflows;
#[path = "suites/contract_cli.rs"]
mod contract_cli;
#[path = "suites/docs_config.rs"]
mod docs_config;
#[path = "suites/mcp_hardening.rs"]
mod mcp_hardening;
#[path = "suites/operations.rs"]
mod operations;
#[path = "suites/presets.rs"]
mod presets;
#[path = "suites/sdk_gen_cli.rs"]
mod sdk_gen_cli;
