// system-tests/tests/mcp_transport.rs
// ============================================================================
// Module: MCP Transport Suite
// Description: Aggregates MCP transport and parity system tests.
// Purpose: Reduce binaries while keeping transport coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates MCP transport and parity system tests.
//! Purpose: Reduce binaries while keeping transport coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/cli_transport.rs"]
mod cli_transport;
#[path = "suites/mcp_transport.rs"]
mod mcp_transport;
#[path = "suites/sse_transport.rs"]
mod sse_transport;
#[path = "suites/transport_parity.rs"]
mod transport_parity;
