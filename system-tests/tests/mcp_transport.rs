// system-tests/tests/mcp_transport.rs
// ============================================================================
// Module: MCP Transport Suite
// Description: Aggregates MCP transport and parity system tests.
// Purpose: Reduce binaries while keeping transport coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! MCP transport suite entry point for system-tests.

mod helpers;

#[path = "suites/mcp_transport.rs"]
mod mcp_transport;
#[path = "suites/sse_transport.rs"]
mod sse_transport;
#[path = "suites/transport_parity.rs"]
mod transport_parity;
