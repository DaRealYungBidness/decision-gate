// decision-gate-cli/src/tests/auth.rs
// ============================================================================
// Module: Authentication & Credential Handling Tests
// Description: Unit tests for credential injection, validation, and redaction.
// Purpose: Ensure credentials are handled safely and injected correctly.
// Dependencies: decision-gate-cli mcp_client, test support server
// ============================================================================

//! ## Overview
//! Validates authentication and credential handling to ensure:
//! - Bearer tokens are injected into Authorization headers.
//! - mTLS client subjects are injected into custom headers.
//! - Invalid header values fail closed.
//! - Debug output redacts bearer tokens.

use std::time::Duration;

use hyper::header::ACCEPT;
use hyper::header::AUTHORIZATION;

use crate::mcp_client::McpClient;
use crate::mcp_client::McpClientConfig;
use crate::mcp_client::McpClientError;
use crate::mcp_client::McpTransport;
use crate::tests::support::TestHttpServer;
use crate::tests::support::TestResponse;
use crate::tests::support::jsonrpc_result;

#[tokio::test]
async fn bearer_token_injected_in_authorization_header() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&jsonrpc_result(serde_json::json!({ "tools": [] })))
    })
    .await;
    let token = "test-bearer-token-12345";
    let config = McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some(server.url()),
        bearer_token: Some(token.to_string()),
        client_subject: None,
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let mut client = McpClient::new(config).expect("client");
    let tools = client.list_tools().await.expect("list tools");
    assert!(tools.is_empty());

    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    let header = requests[0].headers.get(AUTHORIZATION).expect("authorization header");
    assert_eq!(header.to_str().expect("auth header utf8"), format!("Bearer {token}"));

    server.shutdown().await;
}

#[tokio::test]
async fn client_subject_injected_in_custom_header() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&jsonrpc_result(serde_json::json!({ "tools": [] })))
    })
    .await;
    let subject = "CN=test-client";
    let config = McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some(server.url()),
        bearer_token: None,
        client_subject: Some(subject.to_string()),
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let mut client = McpClient::new(config).expect("client");
    let _ = client.list_tools().await.expect("list tools");

    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    let header =
        requests[0].headers.get("x-decision-gate-client-subject").expect("client subject header");
    assert_eq!(header.to_str().expect("subject header utf8"), subject);

    server.shutdown().await;
}

#[tokio::test]
async fn sse_transport_sets_accept_header() {
    let server = TestHttpServer::start(|_| {
        TestResponse::sse_json(&jsonrpc_result(serde_json::json!({ "tools": [] })))
    })
    .await;
    let config = McpClientConfig {
        transport: McpTransport::Sse,
        endpoint: Some(server.url()),
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let mut client = McpClient::new(config).expect("client");
    let _ = client.list_tools().await.expect("list tools");

    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    let header = requests[0].headers.get(ACCEPT).expect("accept header");
    assert_eq!(header.to_str().expect("accept header utf8"), "text/event-stream");

    server.shutdown().await;
}

#[tokio::test]
async fn invalid_bearer_token_rejected() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&jsonrpc_result(serde_json::json!({ "tools": [] })))
    })
    .await;
    let token = "invalid\nvalue";
    let config = McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some(server.url()),
        bearer_token: Some(token.to_string()),
        client_subject: None,
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected invalid header");
    assert!(matches!(err, McpClientError::Config(_)));

    server.shutdown().await;
}

#[tokio::test]
async fn invalid_client_subject_rejected() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&jsonrpc_result(serde_json::json!({ "tools": [] })))
    })
    .await;
    let subject = "CN=bad\nvalue";
    let config = McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some(server.url()),
        bearer_token: None,
        client_subject: Some(subject.to_string()),
        timeout: Duration::from_millis(2_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected invalid header");
    assert!(matches!(err, McpClientError::Config(_)));

    server.shutdown().await;
}

#[test]
fn bearer_token_redacted_in_debug() {
    let secret = "super-secret-token";
    let config = McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some("http://127.0.0.1:8080/rpc".to_string()),
        bearer_token: Some(secret.to_string()),
        client_subject: None,
        timeout: Duration::from_millis(5_000),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let debug_output = format!("{config:?}");
    assert!(!debug_output.contains(secret), "bearer token leaked in debug output: {debug_output}");
}
