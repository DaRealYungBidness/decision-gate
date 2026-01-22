// system-tests/tests/helpers/mod.rs
// ============================================================================
// Module: System Test Helpers
// Description: Shared helpers for Decision Gate system-tests.
// Purpose: Provide MCP harnesses, fixtures, and artifact utilities.
// Dependencies: system-tests, decision-gate-core, decision-gate-mcp
// ============================================================================

#![allow(dead_code, reason = "Shared helpers are reused across multiple test suites.")]

pub mod artifacts;
pub mod harness;
pub mod mcp_client;
pub mod provider_stub;
pub mod readiness;
pub mod scenarios;
