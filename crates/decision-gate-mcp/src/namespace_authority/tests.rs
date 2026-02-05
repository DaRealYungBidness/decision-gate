// crates/decision-gate-mcp/src/namespace_authority/tests.rs
// ============================================================================
// Module: Namespace Authority Tests
// Description: Unit tests for Asset Core namespace authority behavior.
// Purpose: Validate URL normalization, header injection, and status mapping.
// Dependencies: decision-gate-mcp, axum
// ============================================================================

//! ## Overview
//! Exercises the Asset Core namespace authority behavior with in-memory HTTP
//! servers to validate header handling and status-to-error mappings.
//!
//! Security posture: Tests cover rejection paths for untrusted headers; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions use unwrap/expect for clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use decision_gate_core::NamespaceId;
use tokio::sync::oneshot;

use super::AssetCoreNamespaceAuthority;
use super::NamespaceAuthority;
use super::NamespaceAuthorityError;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

#[derive(Default)]
struct HeaderCapture {
    authorization: Option<String>,
    correlation: Option<String>,
}

struct TestServerState {
    status: StatusCode,
    capture: Option<Arc<Mutex<HeaderCapture>>>,
}

async fn namespace_handler(
    State(state): State<Arc<TestServerState>>,
    headers: HeaderMap,
    Path(_namespace): Path<String>,
) -> StatusCode {
    if let Some(capture) = state.capture.as_ref() {
        let mut guard = capture.lock().expect("capture lock");
        guard.authorization =
            headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()).map(str::to_string);
        guard.correlation = headers
            .get("x-correlation-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
    }
    state.status
}

async fn spawn_namespace_server(
    status: StatusCode,
    capture: Option<Arc<Mutex<HeaderCapture>>>,
) -> (String, oneshot::Sender<()>) {
    let state = Arc::new(TestServerState {
        status,
        capture,
    });
    let app = Router::new()
        .route("/v1/write/namespaces/{namespace}", get(namespace_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });
    (format!("http://{}", addr), shutdown_tx)
}

fn authority_with_base(
    base_url: String,
    auth_token: Option<String>,
) -> AssetCoreNamespaceAuthority {
    AssetCoreNamespaceAuthority::new(
        base_url,
        auth_token,
        Duration::from_millis(250),
        Duration::from_millis(250),
    )
    .expect("authority")
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn base_url_trimmed_on_construction() {
    let authority = authority_with_base("http://example.local/".to_string(), None);
    assert_eq!(authority.base_url, "http://example.local");
}

#[tokio::test]
async fn headers_include_bearer_and_correlation() {
    let capture = Arc::new(Mutex::new(HeaderCapture::default()));
    let (base_url, shutdown_tx) =
        spawn_namespace_server(StatusCode::OK, Some(Arc::clone(&capture))).await;
    let authority = authority_with_base(base_url, Some("token-123".to_string()));
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    authority.ensure_namespace(None, &namespace_id, Some("corr-123")).await.expect("ensure ok");
    let headers = capture.lock().expect("capture lock");
    assert_eq!(headers.authorization.as_deref(), Some("Bearer token-123"));
    assert_eq!(headers.correlation.as_deref(), Some("corr-123"));
    let _ = shutdown_tx.send(());
}

#[test]
fn invalid_request_id_rejected() {
    let authority = authority_with_base("http://example.local".to_string(), None);
    let err = authority.build_headers(Some("bad request id")).expect_err("invalid request id");
    assert!(matches!(err, NamespaceAuthorityError::InvalidNamespace(_)));
}

#[test]
fn invalid_auth_token_rejected() {
    let authority = authority_with_base("http://example.local".to_string(), Some("bad\nid".into()));
    let err = authority.build_headers(None).expect_err("invalid auth token");
    assert!(matches!(err, NamespaceAuthorityError::InvalidNamespace(_)));
}

#[tokio::test]
async fn status_mappings_are_consistent() {
    let namespace_id = NamespaceId::from_raw(42).expect("nonzero namespaceid");

    for (status, expectation) in [
        (StatusCode::OK, Ok(())),
        (
            StatusCode::NOT_FOUND,
            Err(NamespaceAuthorityError::Denied("namespace not found".to_string())),
        ),
        (
            StatusCode::UNAUTHORIZED,
            Err(NamespaceAuthorityError::Denied("namespace not authorized".to_string())),
        ),
        (
            StatusCode::FORBIDDEN,
            Err(NamespaceAuthorityError::Denied("namespace not authorized".to_string())),
        ),
        (
            StatusCode::BAD_GATEWAY,
            Err(NamespaceAuthorityError::Unavailable(
                "namespace authority error: status 502 Bad Gateway".to_string(),
            )),
        ),
    ] {
        let (base_url, shutdown_tx) = spawn_namespace_server(status, None).await;
        let authority = authority_with_base(base_url, None);
        let result = authority.ensure_namespace(None, &namespace_id, None).await;
        match (result, expectation) {
            (Ok(()), Ok(())) => {}
            (Err(actual), Err(expected)) => assert_eq!(actual.to_string(), expected.to_string()),
            (actual, expected) => panic!("unexpected result: {actual:?} vs {expected:?}"),
        }
        let _ = shutdown_tx.send(());
    }
}
