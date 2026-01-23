// system-tests/src/bin/decision_gate_stdio_server.rs
// ============================================================================
// Module: Decision Gate Stdio Server
// Description: MCP stdio server runner for system-tests.
// Purpose: Provide a dedicated stdio server binary for end-to-end tests.
// Dependencies: decision-gate-mcp, tokio
// ============================================================================

//! Stdio MCP server binary for system-tests.

use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::McpServer;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = match DecisionGateConfig::load(None) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("decision-gate-stdio-server: config load failed: {err}");
            std::process::exit(1);
        }
    };

    let server = match tokio::task::spawn_blocking(move || McpServer::from_config(config)).await {
        Ok(result) => match result {
            Ok(server) => server,
            Err(err) => {
                eprintln!("decision-gate-stdio-server: init failed: {err}");
                std::process::exit(1);
            }
        },
        Err(err) => {
            eprintln!("decision-gate-stdio-server: init join failed: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = server.serve().await {
        eprintln!("decision-gate-stdio-server: server failed: {err}");
        std::process::exit(1);
    }
}
