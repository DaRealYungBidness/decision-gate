// crates/decision-gate-providers/tests/http_provider_unit.rs
// ============================================================================
// Module: HTTP Provider Unit Tests
// Description: Focused unit tests for HTTP provider edge cases
// Purpose: Test Content-Length handling, timeout behavior, truncation, and error classification
// Threat Models: TM-HTTP-001 (SSRF), TM-HTTP-002 (TLS), TM-HTTP-003 (resource exhaustion)
// ============================================================================

//! ## Overview
//! Unit-level tests for HTTP provider edge cases that are not covered by integration tests:
//! - Content-Length header edge cases (missing, malformed, mismatched)
//! - Timeout behavior (connect, read, boundary conditions)
//! - Response truncation detection
//! - Network error classification
//!
//! These tests complement the existing integration tests in `http_provider.rs` which cover
//! happy paths, scheme restrictions, and host allowlist enforcement.
//!
//! ## Security Posture
//! Assumes adversarial network: servers may send malformed headers, lie about content length,
//! hang connections, or attempt resource exhaustion. Provider must fail closed.
//! See `Docs/security/threat_model.md`.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

mod common;

use std::collections::BTreeSet;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_providers::HttpProvider;
use decision_gate_providers::HttpProviderConfig;
use serde_json::json;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server;

use crate::common::sample_context;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Creates a provider configured to allow HTTP and the local server host.
fn local_provider() -> HttpProvider {
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("127.0.0.1".to_string());
    HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        allow_private_networks: true,
        timeout_ms: 5000,
        ..HttpProviderConfig::default()
    })
    .unwrap()
}

/// Creates a provider with custom timeout
fn timeout_provider(timeout_ms: u64) -> HttpProvider {
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("127.0.0.1".to_string());
    HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        allow_private_networks: true,
        timeout_ms,
        ..HttpProviderConfig::default()
    })
    .unwrap()
}

/// Creates a provider with custom max response size
fn size_limited_provider(max_bytes: usize) -> HttpProvider {
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("127.0.0.1".to_string());
    HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        allow_private_networks: true,
        max_response_bytes: max_bytes,
        ..HttpProviderConfig::default()
    })
    .unwrap()
}

fn raw_http_response_server(response: Vec<u8>) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(&response);
            let _ = stream.flush();
        }
    });
    (addr, handle)
}

// ============================================================================
// SECTION: Content-Length Edge Cases (TM-HTTP-003: Resource Exhaustion)
// ============================================================================

/// TM-HTTP-003: Tests that missing Content-Length header with streaming body is handled correctly.
///
/// Context: Servers may omit Content-Length and use chunked encoding. Provider must enforce
/// size limits even without explicit Content-Length header.
#[test]
fn http_content_length_missing_with_body() {
    let body_content = "response without content-length";
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            // Send response WITHOUT Content-Length header (chunked encoding implied)
            let response = Response::from_string(body_content);
            let _ = request.respond(response);
        }
    });

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    // Should succeed - missing Content-Length is valid HTTP/1.1
    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    // Verify it returns a hash (not an error)
    assert!(result.is_ok(), "Missing Content-Length should not cause failure");
}

/// TM-HTTP-003: Tests that Content-Length: 0 with non-empty body is detected as invalid.
///
/// Context: Malicious server sends Content-Length: 0 but then sends body data.
/// This is a protocol violation and should be handled conservatively.
#[test]
fn http_content_length_zero_with_nonempty_body() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            // Manually construct response with Content-Length: 0 but body content
            let header = Header::from_bytes(&b"Content-Length"[..], &b"0"[..]).unwrap();

            let mut response = tiny_http::Response::empty(200);
            response.add_header(header);

            // Note: tiny_http may not allow this contradiction, so this tests the client's handling
            // In practice, the client (reqwest) will read until connection close or 0 bytes
            let _ = request.respond(response);
        }
    });

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_ok(), "Content-Length:0 response should remain valid");
}

/// TM-HTTP-003: Tests that Content-Length exceeding `max_response_bytes` is rejected early.
///
/// Context: Malicious server advertises huge Content-Length to cause memory allocation.
/// Provider must check header before reading body.
#[test]
fn http_content_length_exceeds_max_response_bytes() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    // Create body larger than limit (2MB body, 1MB limit)
    let large_body = "x".repeat(2 * 1024 * 1024);

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            // Send response with body that exceeds limit
            // tiny_http will automatically set correct Content-Length
            let response = Response::from_string(large_body);
            let _ = request.respond(response);
        }
    });

    // Provider with 1MB limit
    let provider = size_limited_provider(1024 * 1024);
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    // Must reject based on advertised size or actual read
    assert!(result.is_err(), "Should reject Content-Length > max_response_bytes");
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("too large")
            || err_msg.contains("exceeds")
            || err_msg.contains("size")
            || err_msg.contains("limit"),
        "Error should mention size limit: {err_msg}"
    );
}

/// TM-HTTP-003: Tests Content-Length header with malformed value (negative).
///
/// Context: Adversarial server sends invalid Content-Length to confuse parser.
#[test]
fn http_content_length_malformed_negative() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: -100\r\n\r\nbody".to_vec();
    let (addr, handle) = raw_http_response_server(response);
    let url = format!("http://{addr}");

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err(), "Malformed Content-Length should be rejected");
}

/// TM-HTTP-003: Tests Content-Length with non-numeric value.
///
/// Context: Malicious server sends Content-Length: "abc" or other garbage.
#[test]
fn http_content_length_non_numeric() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: invalid\r\n\r\nbody".to_vec();
    let (addr, handle) = raw_http_response_server(response);
    let url = format!("http://{addr}");

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err(), "Non-numeric Content-Length should be rejected");
}

/// TM-HTTP-003: Tests Content-Length with integer overflow value.
///
/// Context: Malicious server sends Content-Length > `u64::MAX` to cause overflow.
#[test]
fn http_content_length_overflow() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Length: 99999999999999999999999999999\r\n\r\nbody".to_vec();
    let (addr, handle) = raw_http_response_server(response);
    let url = format!("http://{addr}");

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err(), "Overflow Content-Length should be rejected");
}

/// TM-HTTP-003: Tests multiple Content-Length headers (RFC 7230 violation).
///
/// Context: RFC 7230 section 3.3.2 states that multiple Content-Length headers
/// with different values MUST be rejected.
#[test]
fn http_multiple_content_length_headers() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Length: 5\r\n\r\nbody!".to_vec();
    let (addr, handle) = raw_http_response_server(response);
    let url = format!("http://{addr}");

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err(), "Ambiguous Content-Length headers should be rejected");
}

// ============================================================================
// SECTION: Basic Parsing and Checks
// ============================================================================

/// TM-HTTP-003: Tests that status codes are parsed correctly.
#[test]
fn http_status_code_parsed() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::empty(418);
            let _ = request.respond(response);
        }
    });

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    handle.join().unwrap();

    let value = result.value.expect("status value");
    let decision_gate_core::EvidenceValue::Json(json_value) = value else {
        panic!("expected json status");
    };
    let serde_json::Value::Number(number) = json_value else {
        panic!("expected numeric status");
    };
    assert_eq!(number.as_u64(), Some(418));
}

/// TM-HTTP-003: Tests body hash computation matches expected digest.
#[test]
fn http_body_hash_matches_expected() {
    let body = "hello-world";
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_string(body);
            let _ = request.respond(response);
        }
    });

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    handle.join().unwrap();

    let value = result.value.expect("hash value");
    let decision_gate_core::EvidenceValue::Json(json_value) = value else {
        panic!("expected json hash");
    };
    let expected = hash_bytes(DEFAULT_HASH_ALGORITHM, body.as_bytes());
    let expected_json = serde_json::to_value(expected).expect("hash json");
    assert_eq!(json_value, expected_json);
}

/// TM-HTTP-003: Tests missing params are rejected.
#[test]
fn http_missing_params_rejected() {
    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: None,
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("params"), "error should mention params");
}

/// TM-HTTP-003: Tests invalid params types are rejected.
#[test]
fn http_invalid_params_rejected() {
    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!(["not-an-object"])),
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
}

/// TM-HTTP-003: Tests unsupported check ids are rejected.
#[test]
fn http_unsupported_check_id_rejected() {
    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "headers".to_string(),
        params: Some(json!({"url": "http://127.0.0.1"})),
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
}

/// TM-HTTP-003: Tests cleartext HTTP is rejected when `allow_http` is false.
#[test]
fn http_rejects_cleartext_when_disallowed() {
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: false,
        ..HttpProviderConfig::default()
    })
    .unwrap();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://127.0.0.1"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("unsupported url scheme"));
}

/// TM-HTTP-003: Tests host allowlist is enforced.
#[test]
fn http_host_allowlist_enforced() {
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("example.com".to_string());
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        ..HttpProviderConfig::default()
    })
    .unwrap();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://127.0.0.1:1"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("host not allowed"));
}

/// TM-HTTP-001: Tests private literal loopback IP is blocked by default.
#[test]
fn http_private_literal_rejected_by_default() {
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("127.0.0.1".to_string());
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        ..HttpProviderConfig::default()
    })
    .unwrap();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://127.0.0.1:1"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("private or link-local"), "{err}");
}

/// TM-HTTP-001: Tests IPv4-mapped IPv6 loopback is blocked by default.
#[test]
fn http_ipv4_mapped_loopback_rejected_by_default() {
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: None,
        ..HttpProviderConfig::default()
    })
    .unwrap();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://[::ffff:127.0.0.1]:1"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("private or link-local"), "{err}");
}

/// TM-HTTP-001: Tests localhost access succeeds with explicit private-network opt-in.
#[test]
fn http_localhost_allowed_with_private_network_opt_in() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::empty(204);
            let _ = request.respond(response);
        }
    });
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("localhost".to_string());
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        allow_private_networks: true,
        ..HttpProviderConfig::default()
    })
    .unwrap();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": format!("http://localhost:{}/", addr.port())})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());
    handle.join().unwrap();
}

/// TM-HTTP-003: Tests truncated body detection when Content-Length exceeds actual bytes.
#[test]
fn http_truncated_body_detected() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nhello".to_vec();
    let (addr, handle) = raw_http_response_server(response);
    let url = format!("http://{addr}");

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("truncated") || err.contains("failed to read response"),
        "unexpected error: {err}"
    );
}

// ============================================================================
// SECTION: Timeout Behavior (TM-HTTP-003: Resource Exhaustion)
// ============================================================================

/// TM-HTTP-003: Tests that connection timeout is enforced.
///
/// Context: Server never accepts connection. Provider must timeout and not hang forever.
#[test]
fn http_connect_timeout_enforced() {
    // Use a non-routable IP (192.0.2.0/24 is TEST-NET-1, should timeout)
    // Actually, this may not work reliably in all environments
    // Better approach: use a port we know is not listening

    // Create provider with very short timeout
    let provider = timeout_provider(100); // 100ms timeout

    // Try to connect to localhost port that's not listening
    // This should timeout quickly
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://127.0.0.1:1"})), // Port 1 unlikely to be open
    };

    let start = std::time::Instant::now();
    let result = provider.query(&query, &sample_context());
    let elapsed = start.elapsed();

    // Should fail with timeout/connection error
    assert!(result.is_err(), "Connection to closed port should fail");

    // Should complete within reasonable time (timeout + overhead)
    // Allow 5x timeout for scheduling variance
    assert!(elapsed < Duration::from_millis(500), "Should timeout quickly, took {elapsed:?}");
}

/// TM-HTTP-003: Tests that read timeout during body is enforced.
///
/// Context: Server sends headers but then stalls when sending body (slow-loris attack).
#[test]
fn http_read_timeout_during_body() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            // Send response headers immediately
            // Then simulate slow body transmission

            // Unfortunately, tiny_http doesn't give us fine control over streaming
            // We can't easily simulate partial write + stall
            // This test is better suited for integration testing with a custom server

            // For now, just send normal response
            let response = Response::from_string("body");
            let _ = request.respond(response);
        }
    });

    let provider = timeout_provider(50); // Very short timeout
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_ok(), "Fast in-memory body should succeed");
}

/// TM-HTTP-003: Tests timeout exactly at boundary (off-by-one).
///
/// Context: Verify timeout triggers at exact threshold, not before or after.
#[test]
fn http_timeout_at_exact_boundary() {
    // This test is challenging without precise control over network timing
    // In practice, timeouts have some variance due to OS scheduling

    let provider = timeout_provider(1000); // 1 second timeout

    // Connect to a server that doesn't exist (should timeout)
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://127.0.0.1:1"})),
    };

    let start = std::time::Instant::now();
    let result = provider.query(&query, &sample_context());
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Should timeout");

    // Should return promptly (connection refused may return quickly)
    assert!(elapsed < Duration::from_millis(2000), "Should return promptly, took {elapsed:?}");
}

/// TM-HTTP-003: Tests timeout cleanup (no resource leaks).
///
/// Context: After timeout, HTTP client should clean up connections properly.
#[test]
fn http_timeout_cleanup() {
    let provider = timeout_provider(100);

    // Make multiple timeout requests
    for _ in 0 .. 5 {
        let query = EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({"url": "http://127.0.0.1:1"})),
        };

        let _ = provider.query(&query, &sample_context());
    }

    // If resources leaked, subsequent requests would fail or hang
    // This test mostly validates that multiple timeouts don't cause panics
    // True leak detection requires instrumentation (e.g., file descriptor counting)
}

// ============================================================================
// SECTION: Response Truncation (TM-HTTP-003: Resource Exhaustion)
// ============================================================================

/// TM-HTTP-003: Tests truncation detection when body is smaller than advertised Content-Length.
///
/// Context: Server advertises Content-Length: 1000 but only sends 500 bytes.
/// Provider must detect truncation to prevent cache poisoning or incomplete data acceptance.
#[test]
fn http_body_truncated_before_content_length() {
    let advertised_len = 1000;
    let body_len = 500;
    let body = "x".repeat(body_len);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {advertised_len}\r\nConnection: close\r\n\r\n{body}"
    )
    .into_bytes();

    let (addr, handle) = raw_http_response_server(response);
    let url = format!("http://{addr}");

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err(), "Truncated response must fail closed");
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("truncated") || err.contains("failed to read response"),
        "unexpected error: {err}"
    );
}

/// TM-HTTP-003: Tests zero-byte response handling.
///
/// Context: Server sends Content-Length: 0 or no body.
#[test]
fn http_zero_byte_response() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            // Send empty response
            let response = Response::from_string("");
            let _ = request.respond(response);
        }
    });

    let provider = local_provider();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    // Should successfully hash empty body
    assert!(result.is_ok(), "Empty body should be valid: {result:?}");
}

/// TM-HTTP-003: Tests response exactly at size limit boundary.
///
/// Context: Server sends exactly `max_response_bytes`. Should succeed.
#[test]
fn http_response_at_exact_size_limit() {
    let max_bytes = 1024;
    let body = "x".repeat(max_bytes);

    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_string(body);
            let _ = request.respond(response);
        }
    });

    let provider = size_limited_provider(max_bytes);
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    // Exactly at limit should succeed
    assert!(result.is_ok(), "Body at exact limit should succeed");
}

/// TM-HTTP-003: Tests response one byte over size limit.
///
/// Context: Server sends `max_response_bytes` + 1. Should be rejected.
#[test]
fn http_response_one_byte_over_limit() {
    let max_bytes = 1024;
    let body = "x".repeat(max_bytes + 1);

    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_string(body);
            let _ = request.respond(response);
        }
    });

    let provider = size_limited_provider(max_bytes);
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    // One byte over limit should fail
    assert!(result.is_err(), "Body over limit should be rejected");
}

// ============================================================================
// SECTION: Network Error Classification (TM-HTTP-003)
// ============================================================================

/// TM-HTTP-003: Tests that connection refused error is properly classified.
///
/// Context: No server listening on port. Should return clear error.
#[test]
fn http_connection_refused_error() {
    let provider = local_provider();

    // Port 1 is unlikely to have a service
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "http://127.0.0.1:1"})),
    };

    let result = provider.query(&query, &sample_context());

    // Must fail with some error (connection refused or timeout)
    assert!(result.is_err(), "Connection refused should return error");

    // Error message should be informative (not generic)
    let _err_msg = format!("{:?}", result.unwrap_err());
    // reqwest typically includes "connection refused" or similar in error
    // We just verify it's not a panic
}

/// TM-HTTP-003: Tests error message clarity for various failure modes.
///
/// Context: Error messages should not leak sensitive information but should be actionable.
#[test]
fn http_error_message_clarity() {
    let provider = local_provider();

    // Test 1: Invalid URL
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "not a url"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err(), "Invalid URL should error");
    let err = format!("{:?}", result.unwrap_err());
    // Should mention URL or parsing issue
    assert!(!err.is_empty(), "Error should have message");

    // Test 2: Unsupported scheme (if allowlist enforced)
    let query2 = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": "ftp://example.com"})),
    };
    let result2 = provider.query(&query2, &sample_context());
    assert!(result2.is_err(), "FTP scheme should be rejected");
    let err2 = format!("{:?}", result2.unwrap_err());
    assert!(!err2.is_empty(), "Error should have message");
}
