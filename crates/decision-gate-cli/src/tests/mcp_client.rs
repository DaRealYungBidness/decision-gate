// crates/decision-gate-cli/src/tests/mcp_client.rs
// ============================================================================
// Module: MCP Client Tests
// Description: Unit tests for SSE parsing and stdio framing helpers.
// Purpose: Ensure MCP client protocol handling stays deterministic and bounded.
// Dependencies: decision-gate-cli mcp_client helpers
// ============================================================================

//! ## Overview
//! Validates MCP SSE parsing, stdio framing, and stdio config environment setup.

use std::io::BufReader;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use hyper::HeaderMap;
use hyper::StatusCode;

use crate::mcp_client::MAX_MCP_RESPONSE_BYTES;
use crate::mcp_client::McpClient;
use crate::mcp_client::McpClientConfig;
use crate::mcp_client::McpClientError;
use crate::mcp_client::McpTransport;
use crate::mcp_client::parse_sse_body;
use crate::mcp_client::read_framed;
use crate::mcp_client::stdio_config_env;
use crate::mcp_client::write_framed;
use crate::tests::support::TestHttpServer;
use crate::tests::support::TestResponse;
use crate::tests::support::jsonrpc_error;
use crate::tests::support::jsonrpc_result;

#[test]
fn parse_sse_body_reads_first_event() {
    let body = b"data: {\"jsonrpc\":\"2.0\"}\n\n\
data: {\"jsonrpc\":\"2.0\",\"id\":1}\n\n";
    let parsed = parse_sse_body(body).expect("parse sse");
    assert_eq!(parsed, b"{\"jsonrpc\":\"2.0\"}");
}

#[test]
fn parse_sse_body_joins_multiline_data() {
    let body = b"data: line-1\ndata: line-2\n\n";
    let parsed = parse_sse_body(body).expect("parse sse");
    assert_eq!(parsed, b"line-1\nline-2");
}

#[test]
fn parse_sse_body_errors_without_data() {
    let err = parse_sse_body(b"event: message\n\n").expect_err("expected missing data error");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn parse_sse_body_errors_on_invalid_utf8() {
    let err = parse_sse_body(&[0xff, 0xfe]).expect_err("expected utf8 error");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn read_framed_rejects_oversized_payload() {
    let oversized = MAX_MCP_RESPONSE_BYTES + 1;
    let data = format!("Content-Length: {oversized}\r\n\r\n");
    let mut reader = BufReader::new(Cursor::new(data.into_bytes()));
    let err = read_framed(&mut reader).expect_err("expected size limit error");
    assert!(matches!(err, McpClientError::ResponseTooLarge { .. }));
}

#[test]
fn framed_roundtrip_preserves_payload() {
    let payload = br#"{"jsonrpc":"2.0"}"#;
    let mut buffer = Vec::new();
    write_framed(&mut buffer, payload).expect("write framed");

    let mut reader = BufReader::new(Cursor::new(buffer));
    let decoded = read_framed(&mut reader).expect("read framed");
    assert_eq!(decoded, payload);
}

#[test]
fn read_framed_errors_on_missing_length() {
    let data = b"\r\n".to_vec();
    let mut reader = BufReader::new(Cursor::new(data));
    let err = read_framed(&mut reader).expect_err("missing length");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn read_framed_errors_on_invalid_length() {
    let data = b"Content-Length: abc\r\n\r\n".to_vec();
    let mut reader = BufReader::new(Cursor::new(data));
    let err = read_framed(&mut reader).expect_err("invalid length");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn read_framed_rejects_duplicate_content_length_header() {
    let data = b"Content-Length: 1\r\nContent-Length: 1\r\n\r\nx".to_vec();
    let mut reader = BufReader::new(Cursor::new(data));
    let err = read_framed(&mut reader).expect_err("duplicate content length");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn read_framed_rejects_oversized_header_line() {
    let long_header = "a".repeat(1_200);
    let payload = format!("{long_header}\r\nContent-Length: 1\r\n\r\nx");
    let mut reader = BufReader::new(Cursor::new(payload.into_bytes()));
    let err = read_framed(&mut reader).expect_err("oversized header line");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn read_framed_rejects_too_many_headers() {
    let mut payload = String::new();
    for _ in 0 .. 80 {
        payload.push_str("X-Test: 1\r\n");
    }
    payload.push_str("Content-Length: 1\r\n\r\nx");
    let mut reader = BufReader::new(Cursor::new(payload.into_bytes()));
    let err = read_framed(&mut reader).expect_err("too many headers");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn read_framed_rejects_oversized_header_block() {
    let mut payload = String::new();
    for _ in 0 .. 50 {
        payload.push_str("X-Test: ");
        payload.push_str(&"a".repeat(170));
        payload.push_str("\r\n");
    }
    payload.push_str("Content-Length: 1\r\n\r\nx");
    let mut reader = BufReader::new(Cursor::new(payload.into_bytes()));
    let err = read_framed(&mut reader).expect_err("oversized headers");
    assert!(matches!(err, McpClientError::Protocol(_)));
}

#[test]
fn stdio_config_env_pairs_path() {
    let path = PathBuf::from("decision-gate.toml");
    let (key, value) = stdio_config_env(&path);
    assert_eq!(key, "DECISION_GATE_CONFIG");
    assert_eq!(value, "decision-gate.toml");
}

fn http_client_config(endpoint: String, timeout: Duration) -> McpClientConfig {
    McpClientConfig {
        transport: McpTransport::Http,
        endpoint: Some(endpoint),
        bearer_token: None,
        client_subject: None,
        timeout,
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    }
}

// ============================================================================
// SECTION: HTTP Response Handling Tests (Extensions)
// ============================================================================

#[tokio::test]
async fn http_400_error_includes_body_preview() {
    let server = TestHttpServer::start(|_| {
        TestResponse::raw(
            StatusCode::BAD_REQUEST,
            HeaderMap::new(),
            Bytes::from_static(b"bad request"),
        )
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected error");
    let message = err.to_string();
    assert!(message.contains("http status 400"));
    assert!(message.contains("bad request"));
    server.shutdown().await;
}

#[tokio::test]
async fn http_500_error_includes_status_code() {
    let server = TestHttpServer::start(|_| {
        TestResponse::raw(
            StatusCode::INTERNAL_SERVER_ERROR,
            HeaderMap::new(),
            Bytes::from_static(b"server error"),
        )
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected error");
    let message = err.to_string();
    assert!(message.contains("http status 500"));
    server.shutdown().await;
}

#[tokio::test]
async fn http_redirect_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert(
        hyper::header::LOCATION,
        hyper::header::HeaderValue::from_static("http://127.0.0.1:1"),
    );
    let server = TestHttpServer::start(move |_| {
        TestResponse::raw(StatusCode::FOUND, headers.clone(), Bytes::new())
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected redirect error");
    assert!(err.to_string().contains("http status 302"));
    server.shutdown().await;
}

#[tokio::test]
async fn http_timeout_fails_gracefully() {
    let server = TestHttpServer::start(|_| {
        std::thread::sleep(Duration::from_millis(200));
        TestResponse::json(&jsonrpc_result(&serde_json::json!({ "tools": [] })))
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(50));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected timeout");
    assert!(matches!(err, McpClientError::Transport(_)));
    server.shutdown().await;
}

#[tokio::test]
async fn http_connection_refused_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    let endpoint = format!("http://127.0.0.1:{port}");
    let config = http_client_config(endpoint, Duration::from_millis(500));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected connection refused");
    assert!(matches!(err, McpClientError::Transport(_)));
}

#[tokio::test]
async fn http_dns_resolution_failure_error() {
    let config =
        http_client_config("http://nonexistent.invalid".to_string(), Duration::from_millis(200));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected dns failure");
    assert!(matches!(err, McpClientError::Transport(_)));
}

#[tokio::test]
async fn http_response_without_content_type() {
    let server = TestHttpServer::start(|_| {
        let headers = HeaderMap::new();
        TestResponse::raw(
            StatusCode::OK,
            headers,
            Bytes::from(
                serde_json::to_vec(&jsonrpc_result(&serde_json::json!({ "tools": [] })))
                    .expect("json"),
            ),
        )
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let tools = client.list_tools().await.expect("list tools");
    assert!(tools.is_empty());
    server.shutdown().await;
}

#[tokio::test]
async fn http_response_without_content_length_handled() {
    let server = TestHttpServer::start(|_| {
        let mut headers = HeaderMap::new();
        headers.insert(
            hyper::header::TRANSFER_ENCODING,
            hyper::header::HeaderValue::from_static("chunked"),
        );
        TestResponse::raw_without_length(
            StatusCode::OK,
            headers,
            Bytes::from(
                serde_json::to_vec(&jsonrpc_result(&serde_json::json!({ "tools": [] })))
                    .expect("json"),
            ),
        )
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let tools = client.list_tools().await.expect("list tools");
    assert!(tools.is_empty());
    server.shutdown().await;
}

#[tokio::test]
async fn http_partial_read_reports_protocol_error() {
    let server = TestHttpServer::start(|_| {
        TestResponse::raw(StatusCode::OK, HeaderMap::new(), Bytes::from_static(b"{"))
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected parse error");
    assert!(matches!(err, McpClientError::Protocol(_)));
    server.shutdown().await;
}

// ============================================================================
// SECTION: JSON-RPC Error Handling Tests
// ============================================================================

#[tokio::test]
async fn jsonrpc_error_message_extracted() {
    let server =
        TestHttpServer::start(|_| TestResponse::json(&jsonrpc_error(-32601, "Method not found")))
            .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected json-rpc error");
    assert!(err.to_string().contains("Method not found"));
    server.shutdown().await;
}

#[tokio::test]
async fn jsonrpc_missing_result_and_error_rejected() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1
        }))
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected missing result error");
    assert!(err.to_string().contains("missing result"));
    server.shutdown().await;
}

#[tokio::test]
async fn jsonrpc_both_result_and_error_handled() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "tools": [] },
            "error": { "code": 0, "message": "broken" }
        }))
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let err = client.list_tools().await.expect_err("expected json-rpc error");
    assert!(err.to_string().contains("broken"));
    server.shutdown().await;
}

#[tokio::test]
async fn request_id_increments_across_requests() {
    let server = TestHttpServer::start(|request| {
        let _ = request;
        TestResponse::json(&jsonrpc_result(&serde_json::json!({ "tools": [] })))
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    let _ = client.list_tools().await.expect("list tools");
    let _ = client.list_tools().await.expect("list tools");
    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    let first: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("request json");
    let second: serde_json::Value =
        serde_json::from_slice(&requests[1].body).expect("request json");
    let first_id = first.get("id").and_then(serde_json::Value::as_u64).expect("id");
    let second_id = second.get("id").and_then(serde_json::Value::as_u64).expect("id");
    assert_eq!(second_id, first_id + 1);
    server.shutdown().await;
}

// ============================================================================
// SECTION: Stdio Process Management Tests
// ============================================================================

#[test]
fn stdio_spawn_failure_reported() {
    let config = McpClientConfig {
        transport: McpTransport::Stdio,
        endpoint: None,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(1),
        stdio_command: Some("/nonexistent/command".to_string()),
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let result = McpClient::new(config);
    assert!(result.is_err());
    let err = result.err().expect("spawn error");
    assert!(err.to_string().contains("spawn stdio failed"));
}

#[tokio::test]
async fn stdio_process_exit_detected() {
    let (command, args) = if cfg!(windows) {
        ("cmd".to_string(), vec!["/C".to_string(), "exit".to_string(), "0".to_string()])
    } else {
        ("sh".to_string(), vec!["-c".to_string(), "exit 0".to_string()])
    };
    let config = McpClientConfig {
        transport: McpTransport::Stdio,
        endpoint: None,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(1),
        stdio_command: Some(command),
        stdio_args: args,
        stdio_env: vec![],
    };
    let mut client = McpClient::new(config).expect("spawn stdio");
    let err = client.list_tools().await.expect_err("expected stdio failure");
    assert!(matches!(err, McpClientError::Transport(_)));
}

#[test]
fn stdio_extra_headers_ignored() {
    let data = b"Content-Length: 10\r\nX-Custom: value\r\n\r\n0123456789";
    let mut reader = BufReader::new(Cursor::new(data));
    let result = read_framed(&mut reader);
    // Should succeed, ignoring X-Custom header
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Request ID Handling Tests
// ============================================================================

#[tokio::test]
async fn request_id_overflow_errors_before_send() {
    let server = TestHttpServer::start(|_| {
        TestResponse::json(&jsonrpc_result(&serde_json::json!({ "tools": [] })))
    })
    .await;
    let config = http_client_config(server.url(), Duration::from_millis(2_000));
    let mut client = McpClient::new(config).expect("client");
    client.set_next_id_for_test(u64::MAX);
    let err = client.list_tools().await.expect_err("expected overflow error");
    assert!(matches!(err, McpClientError::Protocol(_)));
    let requests = server.requests().await;
    assert!(requests.is_empty(), "request should not be sent on overflow");
    server.shutdown().await;
}

// ============================================================================
// SECTION: Transport Configuration Tests
// ============================================================================

#[test]
fn http_missing_endpoint_config_error() {
    let config = McpClientConfig {
        transport: McpTransport::Http,
        endpoint: None,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(5),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let result = McpClient::new(config);
    assert!(result.is_err());
    let err = result.err().expect("config error");
    assert!(matches!(err, McpClientError::Config(_)));
}

#[test]
fn stdio_missing_command_config_error() {
    let config = McpClientConfig {
        transport: McpTransport::Stdio,
        endpoint: None,
        bearer_token: None,
        client_subject: None,
        timeout: Duration::from_secs(5),
        stdio_command: None,
        stdio_args: vec![],
        stdio_env: vec![],
    };
    let result = McpClient::new(config);
    assert!(result.is_err());
    let err = result.err().expect("config error");
    assert!(matches!(err, McpClientError::Config(_)));
}
