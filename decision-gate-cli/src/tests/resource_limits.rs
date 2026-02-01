// decision-gate-cli/src/tests/resource_limits.rs
// ============================================================================
// Module: Resource Limit Enforcement Tests
// Description: Unit tests for CLI size limits and response bounding.
// Purpose: Ensure large inputs/responses are rejected before parsing.
// Dependencies: decision-gate-cli mcp_client, test support server
// ============================================================================

//! ## Overview
//! Verifies hard size limits are enforced for MCP HTTP/SSE responses.

use std::time::Duration;

use bytes::Bytes;
use hyper::HeaderMap;
use hyper::StatusCode;

use crate::mcp_client::MAX_MCP_RESPONSE_BYTES;
use crate::mcp_client::McpClient;
use crate::mcp_client::McpClientConfig;
use crate::mcp_client::McpClientError;
use crate::mcp_client::McpTransport;
use crate::tests::support::TestHttpServer;
use crate::tests::support::TestResponse;
use crate::tests::support::jsonrpc_result;

fn http_config(endpoint: String) -> McpClientConfig {
    McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some(endpoint),
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    }
}

fn sse_config(endpoint: String) -> McpClientConfig {
    McpClientConfig {
        transport: McpTransport::Sse,
        endpoint: Some(endpoint),
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    }
}

fn jsonrpc_body_with_total_len(target_len: usize) -> Vec<u8> {
    let base = serde_json::to_vec(&jsonrpc_result(&serde_json::json!({
        "tools": [],
        "padding": ""
    })))
    .expect("serialize json")
    .len();
    assert!(target_len >= base, "target length too small");
    let padding_len = target_len - base;
    let payload = jsonrpc_result(&serde_json::json!({
        "tools": [],
        "padding": "X".repeat(padding_len)
    }));
    let bytes = serde_json::to_vec(&payload).expect("serialize padded json");
    assert_eq!(bytes.len(), target_len);
    bytes
}

#[tokio::test]
async fn http_response_rejects_oversized_body() {
    let oversized = vec![b'a'; MAX_MCP_RESPONSE_BYTES + 1];
    let server = TestHttpServer::start(move |_| {
        TestResponse::raw(StatusCode::OK, HeaderMap::new(), Bytes::from(oversized.clone()))
    })
    .await;
    let config = http_config(server.url());
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected size limit error");
    assert!(matches!(err, McpClientError::ResponseTooLarge { .. }));
    server.shutdown().await;
}

#[tokio::test]
async fn http_response_accepts_body_at_limit() {
    let body = jsonrpc_body_with_total_len(MAX_MCP_RESPONSE_BYTES);
    let server = TestHttpServer::start(move |_| {
        TestResponse::raw(StatusCode::OK, HeaderMap::new(), Bytes::from(body.clone()))
    })
    .await;
    let config = http_config(server.url());
    let mut client = McpClient::new(config).expect("client");
    let tools = client.list_tools().await.expect("list tools");
    assert!(tools.is_empty());
    server.shutdown().await;
}

#[tokio::test]
async fn sse_response_rejects_oversized_body() {
    let oversized = "a".repeat(MAX_MCP_RESPONSE_BYTES);
    let body = format!("data: {oversized}\n\n");
    let mut headers = HeaderMap::new();
    headers.insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/event-stream"),
    );
    let server = TestHttpServer::start(move |_| {
        TestResponse::raw(StatusCode::OK, headers.clone(), Bytes::from(body.clone()))
    })
    .await;
    let config = sse_config(server.url());
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected size limit error");
    assert!(matches!(err, McpClientError::ResponseTooLarge { .. }));
    server.shutdown().await;
}
