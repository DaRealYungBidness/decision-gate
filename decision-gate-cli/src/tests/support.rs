// decision-gate-cli/src/tests/support.rs
// ============================================================================
// Module: CLI Test Support Helpers
// Description: Shared helpers for HTTP test servers and JSON-RPC responses.
// Purpose: Provide reusable fixtures for CLI unit tests without external deps.
// Dependencies: hyper, tokio, http-body-util, serde_json
// ============================================================================

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::body::Body;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Captured HTTP request data for assertions.
#[derive(Clone, Debug)]
pub struct CapturedRequest {
    /// Request headers.
    pub headers: hyper::HeaderMap,
    /// Raw request body bytes.
    pub body: Bytes,
}

/// Test response wrapper.
#[derive(Clone, Debug)]
pub struct TestResponse {
    /// HTTP status code.
    pub status: StatusCode,
    /// Response headers.
    pub headers: hyper::HeaderMap,
    /// Response body bytes.
    pub body: Bytes,
    /// Whether to omit the Content-Length header.
    pub omit_content_length: bool,
}

impl TestResponse {
    /// Builds a JSON response with Content-Type set.
    pub fn json(value: &Value) -> Self {
        let body = serde_json::to_vec(value).expect("serialize json response");
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        );
        Self {
            status: StatusCode::OK,
            headers,
            body: Bytes::from(body),
            omit_content_length: false,
        }
    }

    /// Builds a raw response with custom status and headers.
    pub fn raw(status: StatusCode, headers: hyper::HeaderMap, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body,
            omit_content_length: false,
        }
    }

    /// Builds a response without Content-Length.
    pub fn raw_without_length(status: StatusCode, headers: hyper::HeaderMap, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body,
            omit_content_length: true,
        }
    }

    /// Builds an SSE response for a JSON payload.
    pub fn sse_json(value: &Value) -> Self {
        let payload = serde_json::to_string(value).expect("serialize json");
        let body = format!("data: {payload}\n\n");
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("text/event-stream"),
        );
        Self {
            status: StatusCode::OK,
            headers,
            body: Bytes::from(body),
            omit_content_length: false,
        }
    }
}

impl From<TestResponse> for Response<Full<Bytes>> {
    fn from(value: TestResponse) -> Self {
        let mut response = Response::new(Full::new(value.body));
        *response.status_mut() = value.status;
        *response.headers_mut() = value.headers;
        if !value.omit_content_length
            && !response.headers().contains_key(hyper::header::CONTENT_LENGTH)
        {
            let len = response.body().size_hint().upper().unwrap_or_default();
            let _ = response.headers_mut().insert(
                hyper::header::CONTENT_LENGTH,
                hyper::header::HeaderValue::from_str(&len.to_string())
                    .expect("content-length header value"),
            );
        }
        response
    }
}

type Responder = Arc<Mutex<Box<dyn FnMut(CapturedRequest) -> TestResponse + Send>>>;

/// Lightweight HTTP test server with request capture.
pub struct TestHttpServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    shutdown: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
}

impl TestHttpServer {
    /// Starts the server with a responder callback.
    pub async fn start<F>(responder: F) -> Self
    where
        F: FnMut(CapturedRequest) -> TestResponse + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test listener");
        let addr = listener.local_addr().expect("local addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let responder: Responder = Arc::new(Mutex::new(Box::new(responder)));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let requests_task = Arc::clone(&requests);
        let responder_task = Arc::clone(&responder);

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    accept = listener.accept() => {
                        let Ok((stream, _)) = accept else { continue };
                        let requests = Arc::clone(&requests_task);
                        let responder = Arc::clone(&responder_task);
                        tokio::spawn(async move {
                            let io = TokioIo::new(stream);
                            let service = service_fn(move |req: Request<Incoming>| {
                                let requests = Arc::clone(&requests);
                                let responder = Arc::clone(&responder);
                                async move {
                                    let (parts, body) = req.into_parts();
                                    let bytes = body.collect().await?.to_bytes();
                                    let captured = CapturedRequest {
                                        headers: parts.headers,
                                        body: bytes,
                                    };
                                    let response = responder.lock().await.as_mut()(captured.clone());
                                    requests.lock().await.push(captured);
                                    let response: Response<Full<Bytes>> = response.into();
                                    Ok::<_, hyper::Error>(response)
                                }
                            });
                            let _ = http1::Builder::new().serve_connection(io, service).await;
                        });
                    }
                }
            }
        });

        Self {
            addr,
            requests,
            shutdown: Some(shutdown_tx),
            handle,
        }
    }

    /// Returns the base URL for the server.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Returns a snapshot of captured requests.
    pub async fn requests(&self) -> Vec<CapturedRequest> {
        self.requests.lock().await.clone()
    }

    /// Shuts down the server.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let _ = self.handle.await;
    }
}

/// Builds a JSON-RPC response object.
pub fn jsonrpc_result(result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": result
    })
}

/// Builds a JSON-RPC error response object.
pub fn jsonrpc_error(code: i64, message: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": code, "message": message }
    })
}
