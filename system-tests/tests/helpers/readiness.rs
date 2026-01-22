// system-tests/tests/helpers/readiness.rs
// ============================================================================
// Module: Readiness Helpers
// Description: Readiness probes for MCP servers.
// Purpose: Ensure servers are ready without arbitrary sleeps.
// Dependencies: tokio
// ============================================================================

use std::time::Duration;
use std::time::Instant;

use tokio::time::sleep;

use super::mcp_client::McpHttpClient;

/// Polls tools/list until the server responds or timeout expires.
pub async fn wait_for_server_ready(
    client: &McpHttpClient,
    timeout: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    let mut attempts = 0u32;
    loop {
        attempts = attempts.saturating_add(1);
        match client.list_tools().await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if start.elapsed() > timeout {
                    return Err(format!(
                        "server readiness timeout after {attempts} attempts: {err}"
                    ));
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
}
