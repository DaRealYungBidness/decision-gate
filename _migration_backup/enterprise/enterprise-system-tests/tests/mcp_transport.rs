// enterprise/enterprise-system-tests/tests/mcp_transport.rs
// ============================================================================
// Module: MCP Transport Suite
// Description: Aggregates enterprise transport parity and TLS tests.
// Purpose: Reduce binaries while keeping transport coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! MCP transport suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/transport_parity.rs"]
mod transport_parity;
#[path = "suites/transport_tls.rs"]
mod transport_tls;
