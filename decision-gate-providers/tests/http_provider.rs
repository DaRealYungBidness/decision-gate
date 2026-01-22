// decision-gate-providers/tests/http_provider.rs
// ============================================================================
// Module: HTTP Provider Tests
// Description: Comprehensive tests for HTTP endpoint evidence provider.
// Purpose: Validate HTTPS enforcement, host allowlist, size limits, and SSRF prevention.
// Dependencies: decision-gate-providers, decision-gate-core, tiny_http
// ============================================================================

//! ## Overview
//! Tests the HTTP provider for:
//! - Happy path: Status and `body_hash` predicates
//! - Boundary enforcement: HTTPS-only, host allowlist, response size limits
//! - Error handling: Invalid URLs, connection failures, unsupported schemes
//! - Adversarial: SSRF prevention (internal IP blocking)
//!
//! Security posture: Network is adversary-controlled. HTTPS is required by
//! default, and host allowlists prevent SSRF attacks.
//! See: `Docs/security/threat_model.md` - TM-HTTP-001

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
use std::thread;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderId;
use decision_gate_providers::HttpProvider;
use decision_gate_providers::HttpProviderConfig;
use serde_json::Value;
use serde_json::json;
use tiny_http::Response;
use tiny_http::Server;

use crate::common::oversized_string;
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
        timeout_ms: 5000,
        ..HttpProviderConfig::default()
    })
    .unwrap()
}

/// Spawns a local test server that responds with the given body and status.
fn spawn_server(body: &'static str, status: u16) -> (String, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let url = format!("http://{addr}");

    let handle = thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_string(body).with_status_code(status);
            let _ = request.respond(response);
        }
    });

    (url, handle)
}

// ============================================================================
// SECTION: Happy Path Tests - Status Predicate
// ============================================================================

/// Tests that HTTP provider returns status code for successful request.
#[test]
fn http_provider_returns_status() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_u64(), Some(200));

    handle.join().unwrap();
}

/// Tests that HTTP provider returns 404 status correctly.
#[test]
fn http_provider_returns_404_status() {
    let (url, handle) = spawn_server("not found", 404);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_u64(), Some(404));

    handle.join().unwrap();
}

/// Tests that HTTP provider returns 500 status correctly.
#[test]
fn http_provider_returns_500_status() {
    let (url, handle) = spawn_server("error", 500);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_u64(), Some(500));

    handle.join().unwrap();
}

/// Tests that evidence anchor and ref are set correctly.
#[test]
fn http_provider_sets_evidence_metadata() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": &url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();

    let anchor = result.evidence_anchor.unwrap();
    assert_eq!(anchor.anchor_type, "url");
    assert!(anchor.anchor_value.contains("127.0.0.1"));

    let evidence_ref = result.evidence_ref.unwrap();
    assert!(evidence_ref.uri.contains("127.0.0.1"));

    handle.join().unwrap();
}

// ============================================================================
// SECTION: Happy Path Tests - Body Hash Predicate
// ============================================================================

/// Tests that `body_hash` returns a hash of the response body.
#[test]
fn http_provider_body_hash_returns_hash() {
    let (url, handle) = spawn_server("hello world", 200);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();

    // The body_hash should return a hash object
    let EvidenceValue::Json(value) = result.value.unwrap() else {
        panic!("expected json evidence");
    };
    // Hash should be an object with algorithm and value (hex-encoded digest)
    let obj = value.as_object().expect("expected object");
    assert!(obj.contains_key("algorithm"));
    assert!(obj.contains_key("value"));

    handle.join().unwrap();
}

/// Tests that `body_hash` is deterministic for same content.
#[test]
fn http_provider_body_hash_deterministic() {
    let (url1, handle1) = spawn_server("identical content", 200);
    let (url2, handle2) = spawn_server("identical content", 200);
    let provider = local_provider();

    let query1 = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url1})),
    };
    let query2 = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url2})),
    };

    let result1 = provider.query(&query1, &sample_context()).unwrap();
    let result2 = provider.query(&query2, &sample_context()).unwrap();

    // Same content should produce same hash
    assert_eq!(result1.value, result2.value);

    handle1.join().unwrap();
    handle2.join().unwrap();
}

/// Tests that `body_hash` differs for different content.
#[test]
fn http_provider_body_hash_differs_for_different_content() {
    let (url1, handle1) = spawn_server("content A", 200);
    let (url2, handle2) = spawn_server("content B", 200);
    let provider = local_provider();

    let query1 = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url1})),
    };
    let query2 = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url2})),
    };

    let result1 = provider.query(&query1, &sample_context()).unwrap();
    let result2 = provider.query(&query2, &sample_context()).unwrap();

    // Different content should produce different hash
    assert_ne!(result1.value, result2.value);

    handle1.join().unwrap();
    handle2.join().unwrap();
}

// ============================================================================
// SECTION: Boundary Enforcement - HTTPS Only
// ============================================================================

/// Tests that HTTP is rejected by default (HTTPS required).
///
/// Security: HTTPS-only prevents man-in-the-middle attacks.
#[test]
fn http_scheme_rejected_by_default() {
    // Default config does not allow HTTP
    let provider = HttpProvider::new(HttpProviderConfig::default()).unwrap();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": "http://example.com/"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported url scheme"));
}

/// Tests that HTTP is allowed when explicitly enabled.
#[test]
fn http_scheme_allowed_when_enabled() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = local_provider(); // Has allow_http: true

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());

    handle.join().unwrap();
}

/// Tests that FTP scheme is rejected.
#[test]
fn http_ftp_scheme_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": "ftp://example.com/file"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported url scheme"));
}

/// Tests that file:// scheme is rejected.
#[test]
fn http_file_scheme_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": "file:///etc/passwd"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported url scheme"));
}

// ============================================================================
// SECTION: Boundary Enforcement - Host Allowlist
// ============================================================================

/// Tests that URLs not in host allowlist are rejected.
///
/// Security: Host allowlist prevents unauthorized requests.
#[test]
fn http_host_not_in_allowlist_rejected() {
    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("allowed.example.com".to_string());
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        ..HttpProviderConfig::default()
    })
    .unwrap();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": "http://forbidden.example.com/"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("host not allowed"));
}

/// Tests that URLs in host allowlist are permitted.
#[test]
fn http_host_in_allowlist_permitted() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = local_provider(); // Has 127.0.0.1 in allowlist

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());

    handle.join().unwrap();
}

/// Tests that empty allowlist rejects all hosts.
#[test]
fn http_empty_allowlist_rejects_all() {
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(BTreeSet::new()),
        ..HttpProviderConfig::default()
    })
    .unwrap();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": "http://any.example.com/"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
}

/// Tests that None allowlist allows all hosts (no restriction).
#[test]
fn http_no_allowlist_allows_all() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: None, // No restriction
        ..HttpProviderConfig::default()
    })
    .unwrap();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());

    handle.join().unwrap();
}

// ============================================================================
// SECTION: Boundary Enforcement - Response Size Limits
// ============================================================================

/// Tests that responses exceeding `max_response_bytes` are rejected.
///
/// Threat model: Resource exhaustion via large responses.
#[test]
fn http_response_exceeds_size_limit_rejected() {
    let large_body = oversized_string(200);
    let large_body_static: &'static str = Box::leak(large_body.into_boxed_str());
    let (url, handle) = spawn_server(large_body_static, 200);

    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("127.0.0.1".to_string());
    let provider = HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowed_hosts),
        max_response_bytes: 100, // Small limit
        ..HttpProviderConfig::default()
    })
    .unwrap();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("exceeds size limit"));

    handle.join().unwrap();
}

// ============================================================================
// SECTION: Error Path Tests - Invalid Parameters
// ============================================================================

/// Tests that unsupported predicates are rejected.
#[test]
fn http_unsupported_predicate_rejected() {
    let provider = local_provider();

    // Use localhost URL to pass host allowlist check and reach predicate validation
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "get".to_string(),
        params: Some(json!({"url": "http://127.0.0.1:9999/"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported http predicate"));
}

/// Tests that missing params are rejected.
#[test]
fn http_missing_params_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: None,
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("requires params"));
}

/// Tests that non-object params are rejected.
#[test]
fn http_params_not_object_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!("not_an_object")),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be an object"));
}

/// Tests that missing url param is rejected.
#[test]
fn http_missing_url_param_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"other": "value"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("missing url"));
}

/// Tests that non-string url param is rejected.
#[test]
fn http_url_param_not_string_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": 12345})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be a string"));
}

/// Tests that invalid URLs are rejected.
#[test]
fn http_invalid_url_rejected() {
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": "not-a-valid-url"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("invalid url"));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests `content_type` is set correctly.
#[test]
fn http_content_type_set() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    assert_eq!(result.content_type, Some("application/json".to_string()));

    handle.join().unwrap();
}

/// Tests that empty body is handled correctly for `body_hash`.
#[test]
fn http_empty_body_hash() {
    let (url, handle) = spawn_server("", 200);
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "body_hash".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();

    // Empty body should still produce a valid hash
    let EvidenceValue::Json(value) = result.value.unwrap() else {
        panic!("expected json evidence");
    };
    let obj = value.as_object().expect("expected object");
    assert!(obj.contains_key("algorithm"));
    assert!(obj.contains_key("value"));

    handle.join().unwrap();
}

/// Tests URL with port number is handled correctly.
#[test]
fn http_url_with_port() {
    let (url, handle) = spawn_server("ok", 200);
    let provider = local_provider();

    // The URL from spawn_server already includes a port
    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());

    handle.join().unwrap();
}

/// Tests URL with path is handled correctly.
#[test]
fn http_url_with_path() {
    let (base_url, handle) = spawn_server("ok", 200);
    let url = format!("{base_url}/some/path");
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());

    handle.join().unwrap();
}

/// Tests URL with query string is handled correctly.
#[test]
fn http_url_with_query_string() {
    let (base_url, handle) = spawn_server("ok", 200);
    let url = format!("{base_url}?foo=bar&baz=qux");
    let provider = local_provider();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        predicate: "status".to_string(),
        params: Some(json!({"url": url})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());

    handle.join().unwrap();
}
