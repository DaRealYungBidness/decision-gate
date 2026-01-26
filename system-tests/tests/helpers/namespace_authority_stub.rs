// system-tests/tests/helpers/namespace_authority_stub.rs
// ============================================================================
// Module: Namespace Authority Stub
// Description: Stub Asset Core namespace authority for system-tests.
// Purpose: Validate namespace authority behavior with deterministic responses.
// Dependencies: axum
// ============================================================================

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use serde::Serialize;
use tokio::task::JoinHandle;

use super::harness::allocate_bind_addr;

#[derive(Clone)]
struct AuthorityState {
    allowed: Arc<BTreeSet<u64>>,
    requests: Arc<Mutex<Vec<NamespaceRequest>>>,
}

/// Captured namespace authority request metadata.
#[derive(Clone, Debug, Serialize)]
pub struct NamespaceRequest {
    pub namespace_id: u64,
    pub correlation_id: Option<String>,
}

/// Handle for the namespace authority stub server.
pub struct NamespaceAuthorityStubHandle {
    base_url: String,
    join: JoinHandle<()>,
    requests: Arc<Mutex<Vec<NamespaceRequest>>>,
}

impl NamespaceAuthorityStubHandle {
    /// Returns the base URL for the stub authority.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns captured namespace authority requests.
    pub fn requests(&self) -> Vec<NamespaceRequest> {
        self.requests.lock().map_or_else(|_| Vec::new(), |entries| entries.clone())
    }
}

impl Drop for NamespaceAuthorityStubHandle {
    fn drop(&mut self) {
        self.join.abort();
    }
}

/// Spawns a namespace authority stub with a set of allowed namespace IDs.
pub async fn spawn_namespace_authority_stub(
    allowed: Vec<u64>,
) -> Result<NamespaceAuthorityStubHandle, String> {
    let addr = allocate_bind_addr()?;
    let allowed: BTreeSet<u64> = allowed.into_iter().collect();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let state = AuthorityState {
        allowed: Arc::new(allowed),
        requests: Arc::clone(&requests),
    };
    let app =
        Router::new().route("/v1/write/namespaces/:id", get(handle_namespace)).with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| format!("namespace authority bind failed: {err}"))?;
    let base_url = format!("http://{}", listener.local_addr().map_err(|err| err.to_string())?);
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(NamespaceAuthorityStubHandle {
        base_url,
        join,
        requests,
    })
}

async fn handle_namespace(
    State(state): State<AuthorityState>,
    Path(namespace_id): Path<u64>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let correlation_id = headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    record_request(&state, namespace_id, correlation_id);
    if state.allowed.contains(&namespace_id) { StatusCode::OK } else { StatusCode::NOT_FOUND }
}

fn record_request(state: &AuthorityState, namespace_id: u64, correlation_id: Option<String>) {
    let Ok(mut guard) = state.requests.lock() else {
        return;
    };
    guard.push(NamespaceRequest {
        namespace_id,
        correlation_id,
    });
}
