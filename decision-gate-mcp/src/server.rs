// decision-gate-mcp/src/server.rs
// ============================================================================
// Module: MCP Server
// Description: MCP server implementations for stdio, HTTP, and SSE transports.
// Purpose: Expose Decision Gate tools via JSON-RPC 2.0.
// Dependencies: decision-gate-core, axum, tokio
// ============================================================================

//! ## Overview
//! The MCP server exposes Decision Gate tools using JSON-RPC 2.0. It supports
//! stdio, HTTP, and SSE transports and always routes calls through
//! [`crate::tools::ToolRouter`]. Security posture: inputs are untrusted and must
//! be validated; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use axum::Router;
use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::response::IntoResponse;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::post;
use decision_gate_contract::ToolName;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::TrustRequirement;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::PrivatePkcs1KeyDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use rustls::pki_types::PrivateSec1KeyDer;
use rustls::server::WebPkiClientVerifier;
use rustls_pemfile::Item;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio_stream::wrappers::ReceiverStream;

use crate::audit::McpAuditEvent;
use crate::audit::McpAuditSink;
use crate::audit::McpFileAuditSink;
use crate::audit::McpNoopAuditSink;
use crate::audit::McpStderrAuditSink;
use crate::auth::DefaultToolAuthz;
use crate::auth::RequestContext;
use crate::auth::StderrAuditSink;
use crate::capabilities::CapabilityRegistry;
use crate::config::DecisionGateConfig;
use crate::config::ProviderType;
use crate::config::RateLimitConfig;
use crate::config::RunStateStoreType;
use crate::config::SchemaRegistryType;
use crate::config::ServerAuditConfig;
use crate::config::ServerAuthMode;
use crate::config::ServerTlsConfig;
use crate::config::ServerTransport;
use crate::evidence::FederatedEvidenceProvider;
use crate::telemetry::McpMethod;
use crate::telemetry::McpMetricEvent;
use crate::telemetry::McpMetrics;
use crate::telemetry::McpOutcome;
use crate::telemetry::NoopMetrics;
use crate::tools::ProviderTransport;
use crate::tools::SchemaRegistryLimits;
use crate::tools::ToolDefinition;
use crate::tools::ToolError;
use crate::tools::ToolRouter;
use crate::tools::ToolRouterConfig;

// ============================================================================
// SECTION: MCP Server
// ============================================================================

/// MCP server instance.
pub struct McpServer {
    /// Server configuration.
    config: DecisionGateConfig,
    /// Tool router for request dispatch.
    router: ToolRouter,
    /// Metrics sink for observability.
    metrics: Arc<dyn McpMetrics>,
    /// Audit sink for request logging.
    audit: Arc<dyn McpAuditSink>,
}

impl McpServer {
    /// Builds a new MCP server from configuration.
    ///
    /// # Errors
    ///
    /// Returns [`McpServerError`] when initialization fails.
    pub fn from_config(config: DecisionGateConfig) -> Result<Self, McpServerError> {
        Self::from_config_with_metrics(config, Arc::new(NoopMetrics))
    }

    /// Builds a new MCP server with a custom metrics sink.
    ///
    /// # Errors
    ///
    /// Returns [`McpServerError`] when initialization fails.
    pub fn from_config_with_metrics(
        config: DecisionGateConfig,
        metrics: Arc<dyn McpMetrics>,
    ) -> Result<Self, McpServerError> {
        let audit = build_audit_sink(&config.server.audit)?;
        Self::from_config_with_observability(config, metrics, audit)
    }

    /// Builds a new MCP server with custom metrics and audit sinks.
    ///
    /// # Errors
    ///
    /// Returns [`McpServerError`] when initialization fails.
    pub fn from_config_with_observability(
        mut config: DecisionGateConfig,
        metrics: Arc<dyn McpMetrics>,
        audit: Arc<dyn McpAuditSink>,
    ) -> Result<Self, McpServerError> {
        config.validate().map_err(|err| McpServerError::Config(err.to_string()))?;
        let evidence = FederatedEvidenceProvider::from_config(&config)
            .map_err(|err| McpServerError::Init(err.to_string()))?;
        let capabilities = CapabilityRegistry::from_config(&config)
            .map_err(|err| McpServerError::Init(err.to_string()))?;
        let store = build_run_state_store(&config)?;
        let schema_registry = build_schema_registry(&config)?;
        let provider_transports = build_provider_transports(&config);
        let schema_registry_limits = build_schema_registry_limits(&config)?;
        let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
        let auth_audit = Arc::new(StderrAuditSink);
        let dispatch_policy = config
            .policy
            .dispatch_policy()
            .map_err(|err| McpServerError::Config(err.to_string()))?;
        let router = ToolRouter::new(ToolRouterConfig {
            evidence,
            evidence_policy: config.evidence.clone(),
            validation: config.validation.clone(),
            dispatch_policy,
            store,
            schema_registry,
            provider_transports,
            schema_registry_limits,
            capabilities: Arc::new(capabilities),
            authz,
            audit: auth_audit,
            trust_requirement: TrustRequirement {
                min_lane: config.trust.min_lane,
            },
        });
        emit_local_only_warning(&config.server);
        Ok(Self {
            config,
            router,
            metrics,
            audit,
        })
    }

    /// Serves requests using the configured transport.
    ///
    /// # Errors
    ///
    /// Returns [`McpServerError`] when the server fails.
    pub async fn serve(self) -> Result<(), McpServerError> {
        let transport = self.config.server.transport;
        match transport {
            ServerTransport::Stdio => serve_stdio(
                &self.router,
                Arc::clone(&self.metrics),
                Arc::clone(&self.audit),
                &self.config.server,
            ),
            ServerTransport::Http => {
                serve_http(self.config, self.router, self.metrics, self.audit).await
            }
            ServerTransport::Sse => {
                serve_sse(self.config, self.router, self.metrics, self.audit).await
            }
        }
    }
}

/// Builds the run state store from MCP configuration.
fn build_run_state_store(
    config: &DecisionGateConfig,
) -> Result<SharedRunStateStore, McpServerError> {
    let store = match config.run_state_store.store_type {
        RunStateStoreType::Memory => SharedRunStateStore::from_store(InMemoryRunStateStore::new()),
        RunStateStoreType::Sqlite => {
            let path = config.run_state_store.path.clone().ok_or_else(|| {
                McpServerError::Config("sqlite run_state_store requires path".to_string())
            })?;
            let sqlite_config = SqliteStoreConfig {
                path,
                busy_timeout_ms: config.run_state_store.busy_timeout_ms,
                journal_mode: config.run_state_store.journal_mode,
                sync_mode: config.run_state_store.sync_mode,
                max_versions: config.run_state_store.max_versions,
            };
            let store = SqliteRunStateStore::new(sqlite_config)
                .map_err(|err| McpServerError::Init(err.to_string()))?;
            SharedRunStateStore::from_store(store)
        }
    };
    Ok(store)
}

/// Builds the schema registry from MCP configuration.
fn build_schema_registry(
    config: &DecisionGateConfig,
) -> Result<SharedDataShapeRegistry, McpServerError> {
    let registry = match config.schema_registry.registry_type {
        SchemaRegistryType::Memory => {
            let max_entries = config
                .schema_registry
                .max_entries
                .map(|value| {
                    usize::try_from(value).map_err(|_| {
                        McpServerError::Config(
                            "schema_registry max_entries exceeds platform limits".to_string(),
                        )
                    })
                })
                .transpose()?;
            SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::with_limits(
                config.schema_registry.max_schema_bytes,
                max_entries,
            ))
        }
        SchemaRegistryType::Sqlite => {
            let path = config.schema_registry.path.clone().ok_or_else(|| {
                McpServerError::Config("sqlite schema_registry requires path".to_string())
            })?;
            let sqlite_config = SqliteStoreConfig {
                path,
                busy_timeout_ms: config.schema_registry.busy_timeout_ms,
                journal_mode: config.schema_registry.journal_mode,
                sync_mode: config.schema_registry.sync_mode,
                max_versions: None,
            };
            let store = SqliteRunStateStore::new(sqlite_config)
                .map_err(|err| McpServerError::Init(err.to_string()))?;
            SharedDataShapeRegistry::from_registry(store)
        }
    };
    Ok(registry)
}

/// Builds the provider transport map from configuration.
fn build_provider_transports(config: &DecisionGateConfig) -> BTreeMap<String, ProviderTransport> {
    let mut transports = BTreeMap::new();
    for provider in &config.providers {
        let transport = match provider.provider_type {
            ProviderType::Builtin => ProviderTransport::Builtin,
            ProviderType::Mcp => ProviderTransport::Mcp,
        };
        transports.insert(provider.name.clone(), transport);
    }
    transports
}

/// Builds schema registry limits from configuration.
fn build_schema_registry_limits(
    config: &DecisionGateConfig,
) -> Result<SchemaRegistryLimits, McpServerError> {
    let max_entries = config
        .schema_registry
        .max_entries
        .map(|value| {
            usize::try_from(value).map_err(|_| {
                McpServerError::Config(
                    "schema_registry max_entries exceeds platform limits".to_string(),
                )
            })
        })
        .transpose()?;
    Ok(SchemaRegistryLimits {
        max_schema_bytes: config.schema_registry.max_schema_bytes,
        max_entries,
    })
}

/// Builds an audit sink from server configuration.
fn build_audit_sink(config: &ServerAuditConfig) -> Result<Arc<dyn McpAuditSink>, McpServerError> {
    if !config.enabled {
        return Ok(Arc::new(McpNoopAuditSink));
    }
    if let Some(path) = &config.path {
        let sink = McpFileAuditSink::new(Path::new(path))
            .map_err(|err| McpServerError::Config(format!("audit log open failed: {err}")))?;
        return Ok(Arc::new(sink));
    }
    Ok(Arc::new(McpStderrAuditSink))
}

/// Builds a TLS config for HTTP/SSE transports.
fn build_tls_config(
    config: &ServerTlsConfig,
) -> Result<axum_server::tls_rustls::RustlsConfig, McpServerError> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let certs = load_certificates(&config.cert_path)?;
    let key = load_private_key(&config.key_path)?;
    let builder = if let Some(ca_path) = &config.client_ca_path {
        let roots = load_root_store(ca_path)?;
        let roots = Arc::new(roots);
        let verifier = if config.require_client_cert {
            WebPkiClientVerifier::builder(roots)
        } else {
            WebPkiClientVerifier::builder(roots).allow_unauthenticated()
        }
        .build()
        .map_err(|err| McpServerError::Config(format!("tls client verifier failed: {err}")))?;
        rustls::ServerConfig::builder().with_client_cert_verifier(verifier)
    } else {
        rustls::ServerConfig::builder().with_no_client_auth()
    };
    let mut server_config = builder
        .with_single_cert(certs, key)
        .map_err(|err| McpServerError::Config(format!("tls config invalid: {err}")))?;
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(server_config)))
}

/// Loads a PEM-encoded certificate chain from disk.
fn load_certificates(path: &str) -> Result<Vec<CertificateDer<'static>>, McpServerError> {
    let file = File::open(path)
        .map_err(|err| McpServerError::Config(format!("tls cert open failed: {err}")))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .map_err(|err| McpServerError::Config(format!("tls cert read failed: {err}")))?;
    if certs.is_empty() {
        return Err(McpServerError::Config("tls cert file contains no certificates".to_string()));
    }
    Ok(certs.into_iter().map(CertificateDer::from).collect())
}

/// Loads a PEM-encoded private key from disk.
fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>, McpServerError> {
    let file = File::open(path)
        .map_err(|err| McpServerError::Config(format!("tls key open failed: {err}")))?;
    let mut reader = BufReader::new(file);
    let items = rustls_pemfile::read_all(&mut reader)
        .map_err(|err| McpServerError::Config(format!("tls key read failed: {err}")))?;
    for item in items {
        match item {
            Item::PKCS8Key(key) => return Ok(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key))),
            Item::RSAKey(key) => return Ok(PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(key))),
            Item::ECKey(key) => return Ok(PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(key))),
            _ => {}
        }
    }
    Err(McpServerError::Config("tls key file contains no private key".to_string()))
}

/// Loads a PEM-encoded CA bundle into a root store.
fn load_root_store(path: &str) -> Result<RootCertStore, McpServerError> {
    let file = File::open(path)
        .map_err(|err| McpServerError::Config(format!("tls ca open failed: {err}")))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .map_err(|err| McpServerError::Config(format!("tls ca read failed: {err}")))?;
    if certs.is_empty() {
        return Err(McpServerError::Config(
            "tls client ca file contains no certificates".to_string(),
        ));
    }
    let mut store = RootCertStore::empty();
    for cert in certs {
        store
            .add(CertificateDer::from(cert))
            .map_err(|err| McpServerError::Config(format!("tls ca invalid: {err}")))?;
    }
    Ok(store)
}

// ============================================================================
// SECTION: Stdio Transport
// ============================================================================

/// Serves JSON-RPC requests over stdin/stdout.
fn serve_stdio(
    router: &ToolRouter,
    metrics: Arc<dyn McpMetrics>,
    audit: Arc<dyn McpAuditSink>,
    server: &crate::config::ServerConfig,
) -> Result<(), McpServerError> {
    let mut reader = BufReader::new(std::io::stdin());
    let mut writer = std::io::stdout();
    let state = build_server_state(router.clone(), server, metrics, audit);
    loop {
        let bytes = read_framed(&mut reader, server.max_body_bytes)?;
        let context = RequestContext::stdio();
        let response = parse_request(&state, &context, &Bytes::from(bytes));
        let payload = serde_json::to_vec(&response.1)
            .map_err(|_| McpServerError::Transport("json-rpc serialization failed".to_string()))?;
        write_framed(&mut writer, &payload)?;
    }
}

// ============================================================================
// SECTION: HTTP Transport
// ============================================================================

/// Serves JSON-RPC requests over HTTP.
async fn serve_http(
    config: DecisionGateConfig,
    router: ToolRouter,
    metrics: Arc<dyn McpMetrics>,
    audit: Arc<dyn McpAuditSink>,
) -> Result<(), McpServerError> {
    let bind = config
        .server
        .bind
        .as_ref()
        .ok_or_else(|| McpServerError::Config("bind address required".to_string()))?;
    let addr: SocketAddr =
        bind.parse().map_err(|_| McpServerError::Config("invalid bind address".to_string()))?;
    let state = Arc::new(build_server_state(router, &config.server, metrics, audit));
    let app = Router::new().route("/rpc", post(handle_http)).with_state(state);
    if let Some(tls) = &config.server.tls {
        let tls_config = build_tls_config(tls)?;
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|_| McpServerError::Transport("http tls server failed".to_string()))
    } else {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|_| McpServerError::Transport("http bind failed".to_string()))?;
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|_| McpServerError::Transport("http server failed".to_string()))
    }
}

/// Serves JSON-RPC requests over SSE.
async fn serve_sse(
    config: DecisionGateConfig,
    router: ToolRouter,
    metrics: Arc<dyn McpMetrics>,
    audit: Arc<dyn McpAuditSink>,
) -> Result<(), McpServerError> {
    let bind = config
        .server
        .bind
        .as_ref()
        .ok_or_else(|| McpServerError::Config("bind address required".to_string()))?;
    let addr: SocketAddr =
        bind.parse().map_err(|_| McpServerError::Config("invalid bind address".to_string()))?;
    let state = Arc::new(build_server_state(router, &config.server, metrics, audit));
    let app = Router::new().route("/rpc", post(handle_sse)).with_state(state);
    if let Some(tls) = &config.server.tls {
        let tls_config = build_tls_config(tls)?;
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|_| McpServerError::Transport("sse tls server failed".to_string()))
    } else {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|_| McpServerError::Transport("sse bind failed".to_string()))?;
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|_| McpServerError::Transport("sse server failed".to_string()))
    }
}

/// Shared server state for HTTP/SSE handlers.
#[derive(Clone)]
struct ServerState {
    /// Tool router for request dispatch.
    router: ToolRouter,
    /// Maximum allowed request body size.
    max_body_bytes: usize,
    /// Metrics sink for request telemetry.
    metrics: Arc<dyn McpMetrics>,
    /// Audit sink for request logging.
    audit: Arc<dyn McpAuditSink>,
    /// Rate limiter for incoming requests.
    rate_limiter: Option<Arc<RateLimiter>>,
    /// Concurrency limiter for inflight requests.
    inflight: Arc<Semaphore>,
}

fn build_server_state(
    router: ToolRouter,
    server: &crate::config::ServerConfig,
    metrics: Arc<dyn McpMetrics>,
    audit: Arc<dyn McpAuditSink>,
) -> ServerState {
    let rate_limiter =
        server.limits.rate_limit.as_ref().map(|config| Arc::new(RateLimiter::new(config.clone())));
    let inflight = Arc::new(Semaphore::new(server.limits.max_inflight));
    ServerState {
        router,
        max_body_bytes: server.max_body_bytes,
        metrics,
        audit,
        rate_limiter,
        inflight,
    }
}

// ============================================================================
// SECTION: Limits
// ============================================================================

/// Fixed-window rate limiter with in-memory buckets.
struct RateLimiter {
    /// Rate limit configuration.
    config: RateLimitConfig,
    /// Per-key request buckets.
    buckets: std::sync::Mutex<HashMap<String, RateLimitBucket>>,
}

/// Rolling state for a single rate limit key.
struct RateLimitBucket {
    /// Window start time for the current bucket.
    window_start: Instant,
    /// Requests observed in the current window.
    count: u32,
    /// Last request timestamp for eviction.
    last_seen: Instant,
}

/// Decision returned by the rate limiter.
enum RateLimitDecision {
    /// Allow the request.
    Allow,
    /// Limit the request with a retry delay.
    Limited {
        /// Milliseconds before retrying the request.
        retry_after_ms: u64,
    },
    /// Reject because the limiter is over capacity.
    OverCapacity,
}

impl RateLimiter {
    /// Creates a new rate limiter from configuration.
    fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Checks the limiter for the given key and updates the bucket.
    fn check(&self, key: &str) -> RateLimitDecision {
        let window = Duration::from_millis(self.config.window_ms);
        let ttl = Duration::from_millis(self.config.window_ms.saturating_mul(2));
        let now = Instant::now();
        {
            let Ok(mut buckets) = self.buckets.lock() else {
                return RateLimitDecision::OverCapacity;
            };

            if buckets.len() > self.config.max_entries {
                buckets.retain(|_, bucket| now.duration_since(bucket.last_seen) <= ttl);
            }

            if buckets.len() > self.config.max_entries {
                RateLimitDecision::OverCapacity
            } else {
                let bucket = buckets.entry(key.to_string()).or_insert(RateLimitBucket {
                    window_start: now,
                    count: 0,
                    last_seen: now,
                });

                if now.duration_since(bucket.window_start) >= window {
                    bucket.window_start = now;
                    bucket.count = 0;
                }

                bucket.last_seen = now;
                if bucket.count >= self.config.max_requests {
                    let elapsed = now.duration_since(bucket.window_start);
                    let retry_after_ms = u64::try_from(window.saturating_sub(elapsed).as_millis())
                        .unwrap_or(u64::MAX);
                    RateLimitDecision::Limited {
                        retry_after_ms,
                    }
                } else {
                    bucket.count = bucket.count.saturating_add(1);
                    RateLimitDecision::Allow
                }
            }
        }
    }
}

/// Handles HTTP JSON-RPC requests.
async fn handle_http(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let context = http_request_context(ServerTransport::Http, peer, &headers);
    let response = parse_request(&state, &context, &bytes);
    (response.0, axum::Json(response.1))
}

/// Handles SSE JSON-RPC requests.
async fn handle_sse(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    bytes: Bytes,
) -> impl IntoResponse {
    let context = http_request_context(ServerTransport::Sse, peer, &headers);
    let response = parse_request(&state, &context, &bytes);
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(1);
    let payload = serde_json::to_string(&response.1).unwrap_or_else(|_| {
        "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32060,\"message\":\"serialization \
         failed\"}}"
            .to_string()
    });
    let _ = tx.send(Ok(Event::default().data(payload))).await;
    Sse::new(ReceiverStream::new(rx))
}

// ============================================================================
// SECTION: JSON-RPC Handling
// ============================================================================

/// Incoming JSON-RPC request payload.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    /// JSON-RPC protocol version.
    jsonrpc: String,
    /// Request identifier.
    id: Value,
    /// Method name.
    method: String,
    /// Optional parameters payload.
    params: Option<Value>,
}

/// JSON-RPC response envelope.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    /// JSON-RPC protocol version.
    jsonrpc: &'static str,
    /// Request identifier.
    id: Value,
    /// Successful result payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    /// Error payload when the request fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error payload.
#[derive(Debug, Serialize)]
struct JsonRpcError {
    /// Error code.
    code: i64,
    /// Human-readable error message.
    message: String,
    /// Structured error metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<JsonRpcErrorData>,
}

/// JSON-RPC error metadata payload.
#[derive(Debug, Serialize)]
struct JsonRpcErrorData {
    /// Normalized error kind label.
    kind: &'static str,
    /// Whether the request may be retried safely.
    retryable: bool,
    /// Request identifier when provided.
    request_id: Option<String>,
    /// Suggested retry delay in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_ms: Option<u64>,
}

/// Tool call parameters for JSON-RPC requests.
#[derive(Debug, Deserialize)]
struct ToolCallParams {
    /// Tool name.
    name: String,
    /// Raw JSON arguments.
    arguments: Value,
}

/// Tool list response payload.
#[derive(Debug, Serialize)]
struct ToolListResult {
    /// Registered tool definitions.
    tools: Vec<ToolDefinition>,
}

/// Tool call response payload.
#[derive(Debug, Serialize)]
struct ToolCallResult {
    /// Tool output content.
    content: Vec<ToolContent>,
}

/// Tool output payloads for JSON-RPC responses.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    /// JSON tool output.
    Json {
        /// JSON payload.
        json: Value,
    },
}

#[derive(Debug, Clone, Copy)]
struct McpRequestInfo {
    /// JSON-RPC method classification.
    method: McpMethod,
    /// Tool name when available.
    tool: Option<ToolName>,
}

/// Dispatches a JSON-RPC request to the tool router.
fn handle_request(
    router: &ToolRouter,
    base_context: &RequestContext,
    request: JsonRpcRequest,
) -> (StatusCode, JsonRpcResponse, McpRequestInfo) {
    let context = base_context.clone();
    if request.jsonrpc != "2.0" {
        return invalid_version_response(&request);
    }
    match request.method.as_str() {
        "tools/list" => handle_tools_list(router, &context, request.id),
        "tools/call" => handle_tools_call(router, &context, request.id, request.params),
        _ => method_not_found_response(&request),
    }
}

/// Builds the response for an invalid JSON-RPC version.
fn invalid_version_response(
    request: &JsonRpcRequest,
) -> (StatusCode, JsonRpcResponse, McpRequestInfo) {
    let request_id = Some(request.id.to_string());
    (
        StatusCode::BAD_REQUEST,
        jsonrpc_error_response(
            request.id.clone(),
            -32600,
            "invalid json-rpc version".to_string(),
            request_id,
            None,
        ),
        McpRequestInfo {
            method: McpMethod::Invalid,
            tool: None,
        },
    )
}

/// Builds the response for unknown JSON-RPC methods.
fn method_not_found_response(
    request: &JsonRpcRequest,
) -> (StatusCode, JsonRpcResponse, McpRequestInfo) {
    let request_id = Some(request.id.to_string());
    (
        StatusCode::BAD_REQUEST,
        jsonrpc_error_response(
            request.id.clone(),
            -32601,
            "method not found".to_string(),
            request_id,
            None,
        ),
        McpRequestInfo {
            method: McpMethod::Other,
            tool: None,
        },
    )
}

/// Handles `tools/list` requests and serializes the response.
fn handle_tools_list(
    router: &ToolRouter,
    context: &RequestContext,
    id: Value,
) -> (StatusCode, JsonRpcResponse, McpRequestInfo) {
    let info = McpRequestInfo {
        method: McpMethod::ToolsList,
        tool: None,
    };
    match router.list_tools(context) {
        Ok(tools) => {
            if let Ok(value) = serde_json::to_value(ToolListResult {
                tools,
            }) {
                (
                    StatusCode::OK,
                    JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: Some(value),
                        error: None,
                    },
                    info,
                )
            } else {
                let response = jsonrpc_error(id, ToolError::Serialization);
                (response.0, response.1, info)
            }
        }
        Err(err) => {
            let response = jsonrpc_error(id, err);
            (response.0, response.1, info)
        }
    }
}

/// Handles `tools/call` requests and serializes the response.
fn handle_tools_call(
    router: &ToolRouter,
    context: &RequestContext,
    id: Value,
    params: Option<Value>,
) -> (StatusCode, JsonRpcResponse, McpRequestInfo) {
    let params = params.unwrap_or(Value::Null);
    let call = serde_json::from_value::<ToolCallParams>(params);
    match call {
        Ok(call) => {
            let info = McpRequestInfo {
                method: McpMethod::ToolsCall,
                tool: ToolName::parse(&call.name),
            };
            match call_tool_with_blocking(router, context, &call.name, call.arguments) {
                Ok(result) => {
                    if let Ok(value) = serde_json::to_value(ToolCallResult {
                        content: vec![ToolContent::Json {
                            json: result,
                        }],
                    }) {
                        (
                            StatusCode::OK,
                            JsonRpcResponse {
                                jsonrpc: "2.0",
                                id,
                                result: Some(value),
                                error: None,
                            },
                            info,
                        )
                    } else {
                        let response = jsonrpc_error(id, ToolError::Serialization);
                        (response.0, response.1, info)
                    }
                }
                Err(err) => {
                    let response = jsonrpc_error(id, err);
                    (response.0, response.1, info)
                }
            }
        }
        Err(_) => invalid_tool_params_response(id),
    }
}

/// Builds the response for invalid tool call parameters.
fn invalid_tool_params_response(id: Value) -> (StatusCode, JsonRpcResponse, McpRequestInfo) {
    let request_id = Some(id.to_string());
    (
        StatusCode::BAD_REQUEST,
        jsonrpc_error_response(id, -32602, "invalid tool params".to_string(), request_id, None),
        McpRequestInfo {
            method: McpMethod::ToolsCall,
            tool: None,
        },
    )
}

/// Executes a tool call, shifting to a blocking context when available.
fn call_tool_with_blocking(
    router: &ToolRouter,
    context: &RequestContext,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| router.handle_tool_call(context, name, arguments))
        }
        _ => router.handle_tool_call(context, name, arguments),
    }
}

/// Request size and timing metadata used for metrics.
struct RequestTiming {
    /// Request size in bytes.
    request_bytes: usize,
    /// Request start time.
    started_at: Instant,
}

/// Records metrics/audit and returns a JSON-RPC error response.
fn reject_request(
    state: &ServerState,
    context: &RequestContext,
    status: StatusCode,
    code: i64,
    message: &str,
    timing: &RequestTiming,
    retry_after_ms: Option<u64>,
) -> (StatusCode, JsonRpcResponse) {
    let response = jsonrpc_error_response(
        Value::Null,
        code,
        message.to_string(),
        context.request_id.clone(),
        retry_after_ms,
    );
    let info = McpRequestInfo {
        method: McpMethod::Invalid,
        tool: None,
    };
    record_metrics(
        state,
        context,
        info,
        &response,
        timing.request_bytes,
        timing.started_at.elapsed(),
    );
    record_audit(state, context, info, &response, timing.request_bytes);
    (status, response)
}

/// Parses and validates a JSON-RPC request payload.
fn parse_request(
    state: &ServerState,
    context: &RequestContext,
    bytes: &Bytes,
) -> (StatusCode, JsonRpcResponse) {
    let started_at = Instant::now();
    let request_bytes = bytes.len();
    let timing = RequestTiming {
        request_bytes,
        started_at,
    };
    let permit = state.inflight.try_acquire().ok();
    if permit.is_none() {
        return reject_request(
            state,
            context,
            StatusCode::SERVICE_UNAVAILABLE,
            -32072,
            "server overloaded",
            &timing,
            None,
        );
    }

    if let Some(rate_limiter) = &state.rate_limiter {
        match rate_limiter.check(&rate_limit_key(context)) {
            RateLimitDecision::Allow => {}
            RateLimitDecision::Limited {
                retry_after_ms,
            } => {
                return reject_request(
                    state,
                    context,
                    StatusCode::TOO_MANY_REQUESTS,
                    -32071,
                    "rate limit exceeded",
                    &timing,
                    Some(retry_after_ms),
                );
            }
            RateLimitDecision::OverCapacity => {
                return reject_request(
                    state,
                    context,
                    StatusCode::SERVICE_UNAVAILABLE,
                    -32072,
                    "rate limiter overloaded",
                    &timing,
                    None,
                );
            }
        }
    }

    if bytes.len() > state.max_body_bytes {
        return reject_request(
            state,
            context,
            StatusCode::PAYLOAD_TOO_LARGE,
            -32070,
            "request body too large",
            &timing,
            None,
        );
    }

    let request: JsonRpcRequest = match serde_json::from_slice(bytes.as_ref()) {
        Ok(request) => request,
        Err(_) => {
            return reject_request(
                state,
                context,
                StatusCode::BAD_REQUEST,
                -32600,
                "invalid json-rpc request",
                &timing,
                None,
            );
        }
    };

    let context = context.clone().with_request_id(request.id.to_string());
    let (status, response, info) = handle_request(&state.router, &context, request);
    record_metrics(
        state,
        &context,
        info,
        &response,
        timing.request_bytes,
        timing.started_at.elapsed(),
    );
    record_audit(state, &context, info, &response, timing.request_bytes);
    drop(permit);
    (status, response)
}

/// Builds an auth context for HTTP/SSE requests from headers.
fn http_request_context(
    transport: ServerTransport,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> RequestContext {
    let auth_header =
        headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()).map(str::to_string);
    let client_subject = headers
        .get("x-decision-gate-client-subject")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    RequestContext::http(transport, Some(peer.ip()), auth_header, client_subject)
}

/// Derives the rate limit key for a request.
fn rate_limit_key(context: &RequestContext) -> String {
    if let Ok(token) = crate::auth::parse_bearer_token(context.auth_header.as_deref()) {
        let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
        return format!("bearer:{}", digest.value);
    }
    if let Some(subject) = &context.client_subject {
        return format!("mtls:{subject}");
    }
    if let Some(peer_ip) = context.peer_ip {
        return format!("ip:{peer_ip}");
    }
    match context.transport {
        ServerTransport::Stdio => "transport:stdio".to_string(),
        ServerTransport::Http => "transport:http".to_string(),
        ServerTransport::Sse => "transport:sse".to_string(),
    }
}

/// Emits a warning when running without explicit auth policy.
fn emit_local_only_warning(server: &crate::config::ServerConfig) {
    let auth_mode = server.auth.as_ref().map_or(ServerAuthMode::LocalOnly, |auth| auth.mode);
    if auth_mode == ServerAuthMode::LocalOnly {
        let _ = writeln!(
            std::io::stderr(),
            "decision-gate-mcp: WARNING: server running in local-only mode without explicit auth; \
             configure server.auth to enable bearer_token or mtls"
        );
    }
}

/// Emits metrics events for a request.
fn record_metrics(
    state: &ServerState,
    context: &RequestContext,
    info: McpRequestInfo,
    response: &JsonRpcResponse,
    request_bytes: usize,
    latency: std::time::Duration,
) {
    let outcome = if response.error.is_some() { McpOutcome::Error } else { McpOutcome::Ok };
    let error_code = response.error.as_ref().map(|error| error.code);
    let error_kind = error_code.and_then(error_kind_for_code);
    let response_bytes = serde_json::to_vec(response).map_or(0, |payload| payload.len());
    let event = McpMetricEvent {
        transport: context.transport,
        method: info.method,
        tool: info.tool,
        outcome,
        error_code,
        error_kind,
        request_bytes,
        response_bytes,
    };
    state.metrics.record_request(event.clone());
    state.metrics.record_latency(event, latency);
}

/// Emits an audit record for a request.
fn record_audit(
    state: &ServerState,
    context: &RequestContext,
    info: McpRequestInfo,
    response: &JsonRpcResponse,
    request_bytes: usize,
) {
    let outcome = if response.error.is_some() { McpOutcome::Error } else { McpOutcome::Ok };
    let error_code = response.error.as_ref().map(|error| error.code);
    let error_kind = error_code.and_then(error_kind_for_code);
    let response_bytes = serde_json::to_vec(response).map_or(0, |payload| payload.len());
    let redaction = match info.tool {
        Some(ToolName::EvidenceQuery) => "evidence",
        _ => "full",
    };
    let event = McpAuditEvent::new(crate::audit::McpAuditEventParams {
        request_id: context.request_id.clone(),
        transport: context.transport,
        peer_ip: context.peer_ip.map(|ip| ip.to_string()),
        method: info.method,
        tool: info.tool,
        outcome,
        error_code,
        error_kind,
        request_bytes,
        response_bytes,
        client_subject: context.client_subject.clone(),
        redaction,
    });
    state.audit.record(&event);
}

/// Maps JSON-RPC error codes to audit labels.
const fn error_kind_for_code(code: i64) -> Option<&'static str> {
    match code {
        -32600 => Some("invalid_request"),
        -32601 => Some("method_not_found"),
        -32602 => Some("invalid_params"),
        -32001 => Some("unauthenticated"),
        -32003 => Some("unauthorized"),
        -32004 => Some("not_found"),
        -32009 => Some("conflict"),
        -32020 => Some("evidence"),
        -32030 => Some("control_plane"),
        -32040 => Some("runpack"),
        -32050 => Some("internal"),
        -32060 => Some("serialization"),
        -32070 => Some("request_too_large"),
        -32071 => Some("rate_limited"),
        -32072 => Some("inflight_limit"),
        _ => None,
    }
}

/// Builds a JSON-RPC error response for a tool failure.
fn jsonrpc_error(id: Value, error: ToolError) -> (StatusCode, JsonRpcResponse) {
    let (status, code, message) = match error {
        ToolError::UnknownTool => (StatusCode::BAD_REQUEST, -32601, "unknown tool".to_string()),
        ToolError::Unauthenticated(_) => {
            (StatusCode::UNAUTHORIZED, -32001, "unauthenticated".to_string())
        }
        ToolError::Unauthorized(_) => (StatusCode::FORBIDDEN, -32003, "unauthorized".to_string()),
        ToolError::InvalidParams(message) => (StatusCode::BAD_REQUEST, -32602, message),
        ToolError::CapabilityViolation {
            code,
            message,
        } => (StatusCode::BAD_REQUEST, -32602, format!("{code}: {message}")),
        ToolError::NotFound(message) => (StatusCode::OK, -32004, message),
        ToolError::Conflict(message) => (StatusCode::OK, -32009, message),
        ToolError::Evidence(message) => (StatusCode::OK, -32020, message),
        ToolError::ControlPlane(err) => (StatusCode::OK, -32030, err.to_string()),
        ToolError::Runpack(message) => (StatusCode::OK, -32040, message),
        ToolError::Internal(message) => (StatusCode::OK, -32050, message),
        ToolError::Serialization => (StatusCode::OK, -32060, "serialization failed".to_string()),
    };
    let request_id = Some(id.to_string());
    (status, jsonrpc_error_response(id, code, message, request_id, None))
}

/// Builds a JSON-RPC error response with structured metadata.
fn jsonrpc_error_response(
    id: Value,
    code: i64,
    message: String,
    request_id: Option<String>,
    retry_after_ms: Option<u64>,
) -> JsonRpcResponse {
    let error_data = JsonRpcErrorData {
        kind: error_kind_label(code),
        retryable: retryable_for_code(code),
        request_id,
        retry_after_ms,
    };
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data: Some(error_data),
        }),
    }
}

/// Returns a stable error kind label for the given code.
fn error_kind_label(code: i64) -> &'static str {
    error_kind_for_code(code).unwrap_or("unknown")
}

/// Returns true when the error code is retryable.
const fn retryable_for_code(code: i64) -> bool {
    matches!(code, -32071 | -32072)
}

// ============================================================================
// SECTION: Framing Helpers
// ============================================================================

/// Reads a framed stdio payload using MCP Content-Length headers.
fn read_framed(
    reader: &mut BufReader<impl Read>,
    max_body_bytes: usize,
) -> Result<Vec<u8>, McpServerError> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|_| McpServerError::Transport("stdio read failed".to_string()))?;
        if bytes == 0 {
            return Err(McpServerError::Transport("stdio closed".to_string()));
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .map_err(|_| McpServerError::Transport("invalid content length".to_string()))?;
            content_length = Some(parsed);
        }
    }
    let len = content_length
        .ok_or_else(|| McpServerError::Transport("missing content length".to_string()))?;
    if len > max_body_bytes {
        return Err(McpServerError::Transport("payload too large".to_string()));
    }
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|_| McpServerError::Transport("stdio read failed".to_string()))?;
    Ok(buf)
}

/// Writes a framed stdio payload using MCP Content-Length headers.
fn write_framed(writer: &mut impl Write, payload: &[u8]) -> Result<(), McpServerError> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer
        .write_all(header.as_bytes())
        .map_err(|_| McpServerError::Transport("stdio write failed".to_string()))?;
    writer
        .write_all(payload)
        .map_err(|_| McpServerError::Transport("stdio write failed".to_string()))?;
    writer.flush().map_err(|_| McpServerError::Transport("stdio write failed".to_string()))
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// MCP server errors.
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    /// Configuration errors.
    #[error("config error: {0}")]
    Config(String),
    /// Initialization errors.
    #[error("init error: {0}")]
    Init(String),
    /// Transport errors.
    #[error("transport error: {0}")]
    Transport(String),
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests {
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
        reason = "Test-only framing assertions."
    )]

    use std::io::BufReader;
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use axum::body::Bytes;
    use axum::http::StatusCode;
    use decision_gate_core::InMemoryDataShapeRegistry;
    use decision_gate_core::InMemoryRunStateStore;
    use decision_gate_core::SharedDataShapeRegistry;
    use decision_gate_core::SharedRunStateStore;
    use decision_gate_core::TrustRequirement;
    use serde_json::json;

    use super::build_provider_transports;
    use super::build_schema_registry_limits;
    use super::build_server_state;
    use super::parse_request;
    use super::read_framed;
    use crate::audit::McpAuditEvent;
    use crate::audit::McpAuditSink;
    use crate::auth::DefaultToolAuthz;
    use crate::auth::NoopAuditSink;
    use crate::auth::RequestContext;
    use crate::capabilities::CapabilityRegistry;
    use crate::config::DecisionGateConfig;
    use crate::config::EvidencePolicyConfig;
    use crate::config::PolicyConfig;
    use crate::config::ProviderConfig;
    use crate::config::ProviderTimeoutConfig;
    use crate::config::ProviderType;
    use crate::config::RateLimitConfig;
    use crate::config::RunStateStoreConfig;
    use crate::config::SchemaRegistryConfig;
    use crate::config::ServerAuthConfig;
    use crate::config::ServerAuthMode;
    use crate::config::ServerConfig;
    use crate::config::ServerTransport;
    use crate::config::TrustConfig;
    use crate::config::ValidationConfig;
    use crate::evidence::FederatedEvidenceProvider;
    use crate::telemetry::McpMethod;
    use crate::telemetry::McpMetricEvent;
    use crate::telemetry::McpMetrics;
    use crate::telemetry::McpOutcome;
    use crate::tools::ToolRouter;
    use crate::tools::ToolRouterConfig;

    #[derive(Default)]
    struct TestMetrics {
        events: Mutex<Vec<McpMetricEvent>>,
        latencies: Mutex<Vec<(McpMetricEvent, Duration)>>,
    }

    impl McpMetrics for TestMetrics {
        fn record_request(&self, event: McpMetricEvent) {
            self.events.lock().expect("events lock").push(event);
        }

        fn record_latency(&self, event: McpMetricEvent, latency: Duration) {
            self.latencies.lock().expect("latencies lock").push((event, latency));
        }
    }

    #[derive(Default)]
    struct TestAudit {
        events: Mutex<Vec<McpAuditEvent>>,
    }

    impl McpAuditSink for TestAudit {
        fn record(&self, event: &McpAuditEvent) {
            self.events.lock().expect("events lock").push(event.clone());
        }
    }

    fn sample_config() -> DecisionGateConfig {
        DecisionGateConfig {
            server: ServerConfig::default(),
            trust: TrustConfig::default(),
            evidence: EvidencePolicyConfig::default(),
            validation: ValidationConfig::default(),
            policy: PolicyConfig::default(),
            run_state_store: RunStateStoreConfig::default(),
            schema_registry: SchemaRegistryConfig::default(),
            providers: builtin_providers(),
        }
    }

    fn sample_router(config: &DecisionGateConfig) -> ToolRouter {
        let evidence = FederatedEvidenceProvider::from_config(config).expect("evidence provider");
        let capabilities = CapabilityRegistry::from_config(config).expect("capabilities");
        let store = SharedRunStateStore::from_store(InMemoryRunStateStore::new());
        let schema_registry =
            SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new());
        let provider_transports = build_provider_transports(config);
        let schema_registry_limits =
            build_schema_registry_limits(config).expect("schema registry limits");
        let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
        let audit = Arc::new(NoopAuditSink);
        ToolRouter::new(ToolRouterConfig {
            evidence,
            evidence_policy: config.evidence.clone(),
            validation: config.validation.clone(),
            dispatch_policy: config.policy.dispatch_policy().expect("dispatch policy"),
            store,
            schema_registry,
            provider_transports,
            schema_registry_limits,
            capabilities: Arc::new(capabilities),
            authz,
            audit,
            trust_requirement: TrustRequirement {
                min_lane: config.trust.min_lane,
            },
        })
    }

    fn builtin_providers() -> Vec<ProviderConfig> {
        vec![
            builtin_provider("time"),
            builtin_provider("env"),
            builtin_provider("json"),
            builtin_provider("http"),
        ]
    }

    fn builtin_provider(name: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: ProviderType::Builtin,
            command: Vec::new(),
            url: None,
            allow_insecure_http: false,
            capabilities_path: None,
            auth: None,
            trust: None,
            allow_raw: false,
            timeouts: ProviderTimeoutConfig::default(),
            config: None,
        }
    }

    #[test]
    fn read_framed_rejects_payload_over_limit() {
        let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let framed = format!(
            "Content-Length: {}\r\n\r\n{}",
            payload.len(),
            String::from_utf8_lossy(payload)
        );
        let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
        let result = read_framed(&mut reader, payload.len() - 1);
        assert!(result.is_err());
    }

    #[test]
    fn read_framed_accepts_payload_at_limit() {
        let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let framed = format!(
            "Content-Length: {}\r\n\r\n{}",
            payload.len(),
            String::from_utf8_lossy(payload)
        );
        let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
        let result = read_framed(&mut reader, payload.len());
        assert!(result.is_ok());
        let bytes = result.expect("payload read");
        assert_eq!(bytes, payload);
    }

    #[test]
    fn metrics_recorded_for_tools_list() {
        let mut config = sample_config();
        config.server.limits.max_inflight = 1;
        let metrics = Arc::new(TestMetrics::default());
        let audit = Arc::new(TestAudit::default());
        let state =
            build_server_state(sample_router(&config), &config.server, metrics.clone(), audit);
        let context = RequestContext::stdio();
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
        });
        let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
        let response = parse_request(&state, &context, &bytes);
        assert_eq!(response.0, StatusCode::OK);

        let events = metrics.events.lock().expect("events lock");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.method, McpMethod::ToolsList);
        assert_eq!(event.outcome, McpOutcome::Ok);
        assert_eq!(event.error_code, None);
        assert!(event.response_bytes > 0);
        drop(events);

        let latencies = metrics.latencies.lock().expect("latencies lock");
        assert_eq!(latencies.len(), 1);
        assert_eq!(latencies[0].0.method, McpMethod::ToolsList);
        drop(latencies);
    }

    #[test]
    fn metrics_recorded_for_unauthenticated_list() {
        let mut config = sample_config();
        config.server.auth = Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
        });
        let metrics = Arc::new(TestMetrics::default());
        let audit = Arc::new(TestAudit::default());
        let state =
            build_server_state(sample_router(&config), &config.server, metrics.clone(), audit);
        let context = RequestContext::http(
            ServerTransport::Http,
            Some(std::net::IpAddr::from([127, 0, 0, 1])),
            None,
            None,
        );
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
        });
        let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
        let response = parse_request(&state, &context, &bytes);
        assert_eq!(response.0, StatusCode::UNAUTHORIZED);

        let events = metrics.events.lock().expect("events lock");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.method, McpMethod::ToolsList);
        assert_eq!(event.outcome, McpOutcome::Error);
        assert_eq!(event.error_code, Some(-32001));
        assert_eq!(event.error_kind, Some("unauthenticated"));
        drop(events);
    }

    #[test]
    fn rate_limit_rejects_after_threshold() {
        let mut config = sample_config();
        config.server.limits.rate_limit = Some(RateLimitConfig {
            max_requests: 1,
            window_ms: 60_000,
            max_entries: 8,
        });
        let metrics = Arc::new(TestMetrics::default());
        let audit = Arc::new(TestAudit::default());
        let state = build_server_state(sample_router(&config), &config.server, metrics, audit);
        let context = RequestContext::http(
            ServerTransport::Http,
            Some(std::net::IpAddr::from([127, 0, 0, 1])),
            None,
            None,
        );
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
        });
        let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
        let first = parse_request(&state, &context, &bytes);
        assert_eq!(first.0, StatusCode::OK);
        let second = parse_request(&state, &context, &bytes);
        assert_eq!(second.0, StatusCode::TOO_MANY_REQUESTS);
        let error = second.1.error.expect("rate limit error");
        assert_eq!(error.code, -32071);
        let data = error.data.expect("error data");
        assert_eq!(data.kind, "rate_limited");
        assert!(data.retryable);
    }

    #[test]
    fn inflight_limit_rejects_when_exhausted() {
        let mut config = sample_config();
        config.server.limits.max_inflight = 1;
        let metrics = Arc::new(TestMetrics::default());
        let audit = Arc::new(TestAudit::default());
        let state = build_server_state(sample_router(&config), &config.server, metrics, audit);
        assert_eq!(state.inflight.available_permits(), 1);
        let permit = state.inflight.try_acquire().expect("permit");
        assert_eq!(state.inflight.available_permits(), 0);
        let context = RequestContext::http(
            ServerTransport::Http,
            Some(std::net::IpAddr::from([127, 0, 0, 1])),
            None,
            None,
        );
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
        });
        let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
        let response = parse_request(&state, &context, &bytes);
        drop(permit);
        assert_eq!(response.0, StatusCode::SERVICE_UNAVAILABLE);
        let error = response.1.error.expect("inflight error");
        assert_eq!(error.code, -32072);
        let data = error.data.expect("error data");
        assert_eq!(data.kind, "inflight_limit");
        assert!(data.retryable);
    }

    #[test]
    fn audit_records_evidence_redaction() {
        let config = sample_config();
        let metrics = Arc::new(TestMetrics::default());
        let audit = Arc::new(TestAudit::default());
        let state =
            build_server_state(sample_router(&config), &config.server, metrics, audit.clone());
        let context = RequestContext::stdio();
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "evidence_query",
                "arguments": {}
            }
        });
        let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
        let _ = parse_request(&state, &context, &bytes);
        let events = audit.events.lock().expect("events lock");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.redaction, "evidence");
        drop(events);
    }
}
