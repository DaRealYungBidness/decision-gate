// decision-gate-broker/tests/sources/http_tests.rs
// ============================================================================
// Module: HttpSource Unit Tests
// Description: Comprehensive tests for the HTTP-backed payload source.
// Purpose: Validate HTTP source input validation and size enforcement.
// Dependencies: decision-gate-broker, decision-gate-core, reqwest, tiny_http
// ============================================================================

//! ## Overview
//! Exercises [`decision_gate_broker::HttpSource`] fetch and validation paths.

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

use std::thread;
use std::time::Duration;

use decision_gate_broker::HttpSource;
use decision_gate_broker::HttpSourcePolicy;
use decision_gate_broker::MAX_SOURCE_BYTES;
use decision_gate_broker::Source;
use decision_gate_broker::SourceError;
use decision_gate_core::ContentRef;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use reqwest::blocking::Client;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server;

fn local_source() -> HttpSource {
    HttpSource::with_policy(HttpSourcePolicy::new().allow_private_networks()).expect("http source")
}

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

/// Tests http source new creates default client.
#[test]
fn http_source_new_creates_default_client() {
    let source = HttpSource::new();
    assert!(source.is_ok());
}

/// Tests http source with client uses custom client.
#[test]
fn http_source_with_client_uses_custom_client() {
    let client = Client::builder().timeout(Duration::from_secs(10)).build().expect("custom client");

    let _source = HttpSource::with_client(client);
    // Source created successfully with custom client
}

/// Tests http source blocks loopback by default.
#[test]
fn http_source_blocks_loopback_by_default() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let content_ref = ContentRef {
        uri: "http://127.0.0.1:8080/data".to_string(),
        content_hash,
        encryption: None,
    };

    let source = HttpSource::new().expect("http source");
    let err = source.fetch(&content_ref).unwrap_err();
    assert!(matches!(err, SourceError::Policy(_)));
}

/// Tests http source allowlist blocks unlisted hosts.
#[test]
fn http_source_allowlist_blocks_unlisted_hosts() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let content_ref = ContentRef {
        uri: "http://127.0.0.1:8080/data".to_string(),
        content_hash,
        encryption: None,
    };

    let policy = HttpSourcePolicy::new().allow_hosts(["example.com"]);
    let source = HttpSource::with_policy(policy).expect("http source");
    let err = source.fetch(&content_ref).unwrap_err();
    assert!(matches!(err, SourceError::Policy(_)));
}

/// Tests http source denylist overrides allowlist.
#[test]
fn http_source_denylist_overrides_allowlist() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let content_ref = ContentRef {
        uri: "http://127.0.0.1:8080/data".to_string(),
        content_hash,
        encryption: None,
    };

    let policy = HttpSourcePolicy::new()
        .allow_hosts(["127.0.0.1"])
        .deny_hosts(["127.0.0.1"])
        .allow_private_networks();
    let source = HttpSource::with_policy(policy).expect("http source");
    let err = source.fetch(&content_ref).unwrap_err();
    assert!(matches!(err, SourceError::Policy(_)));
}

// ============================================================================
// SECTION: Success Path Tests
// ============================================================================

/// Tests http source fetches bytes with content type.
#[test]
fn http_source_fetches_bytes_with_content_type() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();
    let body = b"remote payload".to_vec();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_data(body).with_header(
                Header::from_bytes("Content-Type", "application/octet-stream").unwrap(),
            );
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/file.bin");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"remote payload");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let payload = source.fetch(&content_ref).expect("http fetch");

    assert_eq!(payload.bytes, b"remote payload");
    assert_eq!(payload.content_type.as_deref(), Some("application/octet-stream"));

    handle.join().expect("server thread");
}

/// Tests http source fetches json content type.
#[test]
fn http_source_fetches_json_content_type() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();
    let body = br#"{"key": "value"}"#.to_vec();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_data(body).with_header(
                Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
            );
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/data.json");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, br#"{"key": "value"}"#);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let payload = source.fetch(&content_ref).expect("http fetch");

    assert_eq!(payload.bytes, br#"{"key": "value"}"#);
    assert_eq!(payload.content_type.as_deref(), Some("application/json; charset=utf-8"));

    handle.join().expect("server thread");
}

/// Tests http source handles missing content type.
#[test]
fn http_source_handles_missing_content_type() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();
    let body = b"no content type".to_vec();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_data(body);
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/raw");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"no content type");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let payload = source.fetch(&content_ref).expect("http fetch");

    assert_eq!(payload.bytes, b"no content type");
    assert!(payload.content_type.is_none());

    handle.join().expect("server thread");
}

/// Tests http source rejects oversized payloads.
#[test]
fn http_source_rejects_oversized_payload() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();
    let body = vec![0_u8; MAX_SOURCE_BYTES + 1];

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_data(body);
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/large");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"oversized");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::TooLarge { .. }));

    handle.join().expect("server thread");
}

/// Tests http source fetches empty response.
#[test]
fn http_source_fetches_empty_response() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_data(Vec::<u8>::new());
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/empty");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let payload = source.fetch(&content_ref).expect("http fetch");

    assert!(payload.bytes.is_empty());

    handle.join().expect("server thread");
}

/// Tests http source supports https scheme.
#[test]
fn http_source_supports_https_scheme() {
    // We can't easily test real HTTPS, but we can verify the scheme is accepted
    // by checking the error is a connection error, not a scheme error
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "https://localhost:65535/unreachable".to_string(),
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    // Should be HTTP error (connection failed), not UnsupportedScheme
    assert!(matches!(err, SourceError::Http(_)));
}

// ============================================================================
// SECTION: Error Path Tests
// ============================================================================

/// Tests http source rejects 404 not found.
#[test]
fn http_source_rejects_404_not_found() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response =
                Response::from_string("Not Found").with_status_code(tiny_http::StatusCode(404));
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/missing");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::Http(_)));
    assert!(err.to_string().contains("404"));

    handle.join().expect("server thread");
}

/// Tests http source rejects 500 server error.
#[test]
fn http_source_rejects_500_server_error() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_string("Internal Server Error")
                .with_status_code(tiny_http::StatusCode(500));
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/error");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::Http(_)));
    assert!(err.to_string().contains("500"));

    handle.join().expect("server thread");
}

/// Tests http source rejects non http scheme.
#[test]
fn http_source_rejects_non_http_scheme() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "file:///etc/passwd".to_string(),
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::UnsupportedScheme(_)));
    assert!(err.to_string().contains("file"));
}

/// Tests http source rejects ftp scheme.
#[test]
fn http_source_rejects_ftp_scheme() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "ftp://example.com/file.bin".to_string(),
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::UnsupportedScheme(_)));
    assert!(err.to_string().contains("ftp"));
}

/// Tests http source rejects malformed uri.
#[test]
fn http_source_rejects_malformed_uri() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "not a valid uri".to_string(),
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::InvalidUri(_)));
}

/// Tests http source handles connection refused.
#[test]
fn http_source_handles_connection_refused() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        // Port 0 should never be open
        uri: "http://127.0.0.1:1/unreachable".to_string(),
        content_hash,
        encryption: None,
    };

    let source = local_source();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::Http(_)));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests http source handles redirect codes as failure.
#[test]
fn http_source_handles_redirect_codes_as_failure() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            // 301 redirect without following
            let response = Response::from_string("Moved Permanently")
                .with_status_code(tiny_http::StatusCode(301))
                .with_header(Header::from_bytes("Location", "http://localhost/new").unwrap());
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}/redirect");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    // HttpSource disables redirects by default, so 3xx should fail closed.
    let source = local_source();
    let result = source.fetch(&content_ref);

    // Either it follows and fails on the new target, or reports HTTP error
    assert!(result.is_err());

    handle.join().expect("server thread");
}
