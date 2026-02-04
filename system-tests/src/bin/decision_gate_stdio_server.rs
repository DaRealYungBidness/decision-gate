// system-tests/src/bin/decision_gate_stdio_server.rs
// ============================================================================
// Module: Decision Gate Stdio Server
// Description: MCP stdio server runner for system-tests.
// Purpose: Provide a dedicated stdio server binary for end-to-end tests.
// Dependencies: decision-gate-mcp, tokio
// ============================================================================

//! ## Overview
//! MCP stdio server runner for system-tests.
//! Purpose: Provide a dedicated stdio server binary for end-to-end tests.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::io::Write;

use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::McpServer;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = match DecisionGateConfig::load(None) {
        Ok(config) => config,
        Err(err) => {
            write_stderr_line(&format!("decision-gate-stdio-server: config load failed: {err}"));
            std::process::exit(1);
        }
    };

    let server = match tokio::task::spawn_blocking(move || McpServer::from_config(config)).await {
        Ok(result) => match result {
            Ok(server) => server,
            Err(err) => {
                write_stderr_line(&format!("decision-gate-stdio-server: init failed: {err}"));
                std::process::exit(1);
            }
        },
        Err(err) => {
            write_stderr_line(&format!("decision-gate-stdio-server: init join failed: {err}"));
            std::process::exit(1);
        }
    };

    if let Err(err) = server.serve().await {
        write_stderr_line(&format!("decision-gate-stdio-server: server failed: {err}"));
        std::process::exit(1);
    }
}

/// Writes a single line to stderr without panicking.
fn write_stderr_line(message: &str) {
    let mut stderr = std::io::stderr();
    let _ = writeln!(&mut stderr, "{message}");
}
