// system-tests/tests/helpers/readiness.rs
// ============================================================================
// Module: Readiness Helpers
// Description: Readiness probes for MCP servers.
// Purpose: Ensure servers are ready without arbitrary sleeps.
// Dependencies: tokio
// ============================================================================

use std::future::Future;
use std::time::Duration;
use std::time::Instant;

use tokio::time::sleep;

use super::mcp_client::McpHttpClient;
use super::stdio_client::StdioMcpClient;
use super::timeouts;

/// Polls a readiness probe until it succeeds or timeout expires.
pub async fn wait_for_ready<F, Fut>(
    mut probe: F,
    timeout: Duration,
    label: &str,
) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let start = Instant::now();
    let mut attempts = 0u32;
    loop {
        attempts = attempts.saturating_add(1);
        match probe().await {
            Ok(()) => return Ok(()),
            Err(err) => {
                if start.elapsed() > timeout {
                    return Err(format!(
                        "{label} readiness timeout after {attempts} attempts: {err}"
                    ));
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

/// Polls tools/list until the HTTP server responds or timeout expires.
pub async fn wait_for_server_ready(
    client: &McpHttpClient,
    timeout: Duration,
) -> Result<(), String> {
    let timeout = timeouts::resolve_timeout(timeout);
    wait_for_ready(|| async { client.list_tools().await.map(|_| ()) }, timeout, "server").await
}

/// Polls tools/list until the stdio server responds or timeout expires.
pub async fn wait_for_stdio_ready(
    client: &StdioMcpClient,
    timeout: Duration,
) -> Result<(), String> {
    let timeout = timeouts::resolve_timeout(timeout);
    wait_for_ready(|| async { client.list_tools().await.map(|_| ()) }, timeout, "stdio server")
        .await
}
