// decision-gate-mcp/src/server/tests.rs
// ============================================================================
// Module: MCP Server Unit Tests
// Description: Unit tests for server framing, metrics, and audit behavior.
// Purpose: Validate server module behavior with in-memory fixtures.
// Dependencies: decision-gate-mcp
// ============================================================================

//! ## Overview
//! Exercises MCP server framing, metrics, and audit hooks with in-memory fixtures.
//!
//! Security posture: Tests exercise untrusted request handling; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

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

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::BufReader;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::http::header::WWW_AUTHENTICATE;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::TenantId;
use serde_json::json;

use super::JsonRpcResponse;
use super::ServerState;
use super::build_provider_transports;
use super::build_response_headers;
use super::build_schema_registry_limits;
use super::build_server_state;
use super::parse_request;
use super::read_framed;
use crate::audit::McpAuditEvent;
use crate::audit::McpAuditSink;
use crate::audit::McpNoopAuditSink;
use crate::auth::DefaultToolAuthz;
use crate::auth::NoopAuditSink;
use crate::auth::RequestContext;
use crate::capabilities::CapabilityRegistry;
use crate::config::DecisionGateConfig;
use crate::config::DocsConfig;
use crate::config::EvidencePolicyConfig;
use crate::config::PolicyConfig;
use crate::config::PrincipalConfig;
use crate::config::PrincipalRoleConfig;
use crate::config::ProviderConfig;
use crate::config::ProviderTimeoutConfig;
use crate::config::ProviderType;
use crate::config::RateLimitConfig;
use crate::config::RunStateStoreConfig;
use crate::config::SchemaRegistryConfig;
use crate::config::ServerAuthConfig;
use crate::config::ServerAuthMode;
use crate::config::ServerConfig;
use crate::config::ServerToolsConfig;
use crate::config::ServerTransport;
use crate::config::TrustConfig;
use crate::config::ValidationConfig;
use crate::docs::DocsCatalog;
use crate::docs::RESOURCE_URI_PREFIX;
use crate::evidence::FederatedEvidenceProvider;
use crate::namespace_authority::NoopNamespaceAuthority;
use crate::telemetry::McpMethod;
use crate::telemetry::McpMetricEvent;
use crate::telemetry::McpMetrics;
use crate::telemetry::McpOutcome;
use crate::tenant_authz::NoopTenantAuthorizer;
use crate::tools::DocsProvider;
use crate::tools::ToolError;
use crate::tools::ToolRouter;
use crate::tools::ToolRouterConfig;
use crate::tools::ToolVisibilityResolver;
use crate::usage::NoopUsageMeter;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

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
        server: ServerConfig {
            auth: Some(ServerAuthConfig {
                mode: ServerAuthMode::LocalOnly,
                bearer_tokens: Vec::new(),
                mtls_subjects: Vec::new(),
                allowed_tools: Vec::new(),
                principals: vec![PrincipalConfig {
                    subject: "stdio".to_string(),
                    policy_class: Some("prod".to_string()),
                    roles: vec![PrincipalRoleConfig {
                        name: "TenantAdmin".to_string(),
                        tenant_id: Some(TenantId::from_raw(100).expect("nonzero tenantid")),
                        namespace_id: Some(NamespaceId::from_raw(1).expect("nonzero namespaceid")),
                    }],
                }],
            }),
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: crate::config::NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::from_raw(100).expect("nonzero tenantid")],
            ..crate::config::NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: crate::config::AnchorPolicyConfig::default(),
        provider_discovery: crate::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: builtin_providers(),
        dev: crate::config::DevConfig::default(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    }
}

fn sample_router(config: &DecisionGateConfig) -> ToolRouter {
    sample_router_with_overrides(config, None, None)
}

fn sample_router_with_overrides(
    config: &DecisionGateConfig,
    docs_provider: Option<Arc<dyn DocsProvider>>,
    tool_visibility_resolver: Option<Arc<dyn ToolVisibilityResolver>>,
) -> ToolRouter {
    let evidence = FederatedEvidenceProvider::from_config(config).expect("evidence provider");
    let capabilities = CapabilityRegistry::from_config(config).expect("capabilities");
    let store = SharedRunStateStore::from_store(InMemoryRunStateStore::new());
    let schema_registry = SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new());
    let provider_transports = build_provider_transports(config);
    let schema_registry_limits =
        build_schema_registry_limits(config).expect("schema registry limits");
    let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
    let principal_resolver =
        crate::registry_acl::PrincipalResolver::from_config(config.server.auth.as_ref());
    let registry_acl = crate::registry_acl::RegistryAcl::new(&config.schema_registry.acl);
    let audit = Arc::new(NoopAuditSink);
    let default_namespace_tenants =
        config.namespace.default_tenants.iter().copied().collect::<std::collections::BTreeSet<_>>();
    let docs_catalog = DocsCatalog::from_config(&config.docs).expect("docs catalog");
    let provider_trust_overrides = if config.is_dev_permissive() {
        config
            .dev
            .permissive_exempt_providers
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    decision_gate_core::TrustRequirement {
                        min_lane: config.trust.min_lane,
                    },
                )
            })
            .collect()
    } else {
        std::collections::BTreeMap::new()
    };
    let runpack_security_context = Some(decision_gate_core::RunpackSecurityContext {
        dev_permissive: config.is_dev_permissive(),
        namespace_authority: "dg_registry".to_string(),
    });
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
        provider_discovery: config.provider_discovery.clone(),
        authz,
        tenant_authorizer: Arc::new(NoopTenantAuthorizer),
        usage_meter: Arc::new(NoopUsageMeter),
        runpack_storage: None,
        runpack_object_store: None,
        audit,
        trust_requirement: config.effective_trust_requirement(),
        anchor_policy: config.anchors.to_policy(),
        provider_trust_overrides,
        runpack_security_context,
        precheck_audit: Arc::new(McpNoopAuditSink),
        precheck_audit_payloads: config.server.audit.log_precheck_payloads,
        registry_acl,
        principal_resolver,
        scenario_next_feedback: config.server.feedback.scenario_next.clone(),
        docs_config: config.docs.clone(),
        docs_catalog,
        tools: config.server.tools.clone(),
        docs_provider,
        tool_visibility_resolver,
        allow_default_namespace: config.allow_default_namespace(),
        default_namespace_tenants,
        namespace_authority: Arc::new(NoopNamespaceAuthority),
    })
}

struct RateLimitedDocsProvider {
    retry_after_ms: u64,
}

impl DocsProvider for RateLimitedDocsProvider {
    fn is_search_enabled(
        &self,
        _context: &RequestContext,
        _auth: &crate::auth::AuthContext,
    ) -> bool {
        true
    }

    fn is_resources_enabled(
        &self,
        _context: &RequestContext,
        _auth: &crate::auth::AuthContext,
    ) -> bool {
        false
    }

    fn search(
        &self,
        _context: &RequestContext,
        _auth: &crate::auth::AuthContext,
        _request: crate::docs::DocsSearchRequest,
    ) -> Result<crate::docs::SearchResult, ToolError> {
        Err(ToolError::RateLimited {
            message: "rate limited".to_string(),
            retry_after_ms: Some(self.retry_after_ms),
        })
    }

    fn list_resources(
        &self,
        _context: &RequestContext,
        _auth: &crate::auth::AuthContext,
    ) -> Result<Vec<crate::docs::ResourceMetadata>, ToolError> {
        Ok(Vec::new())
    }

    fn read_resource(
        &self,
        _context: &RequestContext,
        _auth: &crate::auth::AuthContext,
        _uri: &str,
    ) -> Result<crate::docs::ResourceContent, ToolError> {
        Err(ToolError::UnknownTool)
    }
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

fn parse_request_sync(
    state: &ServerState,
    context: &RequestContext,
    bytes: &Bytes,
) -> (StatusCode, JsonRpcResponse) {
    tokio::runtime::Runtime::new().expect("runtime").block_on(parse_request(state, context, bytes))
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn read_framed_rejects_payload_over_limit() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let framed =
        format!("Content-Length: {}\r\n\r\n{}", payload.len(), String::from_utf8_lossy(payload));
    let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
    let result = read_framed(&mut reader, payload.len() - 1);
    assert!(result.is_err());
}

#[test]
fn read_framed_accepts_payload_at_limit() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let framed =
        format!("Content-Length: {}\r\n\r\n{}", payload.len(), String::from_utf8_lossy(payload));
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
        build_server_state(sample_router(&config), &config.server, metrics.clone(), audit, None);
    let context = RequestContext::stdio();
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    let response = parse_request_sync(&state, &context, &bytes);
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

// ============================================================================
// SECTION: Resources Endpoints
// ============================================================================

#[test]
fn resources_list_returns_embedded_docs() {
    let config = sample_config();
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state = build_server_state(sample_router(&config), &config.server, metrics, audit, None);
    let context = RequestContext::stdio();
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list",
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::OK);
    let result = response.1.result.expect("result");
    let resources =
        result.get("resources").and_then(|value| value.as_array()).expect("resources array");
    assert!(!resources.is_empty());
    let uri = resources[0].get("uri").and_then(|value| value.as_str()).expect("uri");
    assert!(uri.starts_with(RESOURCE_URI_PREFIX));
}

#[test]
fn resources_read_returns_markdown() {
    let config = sample_config();
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state = build_server_state(sample_router(&config), &config.server, metrics, audit, None);
    let context = RequestContext::stdio();
    let list_payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list",
    });
    let list_bytes = Bytes::from(serde_json::to_vec(&list_payload).expect("payload bytes"));
    let list_response = parse_request_sync(&state, &context, &list_bytes);
    assert_eq!(list_response.0, StatusCode::OK);
    let list_result = list_response.1.result.expect("result");
    let resources =
        list_result.get("resources").and_then(|value| value.as_array()).expect("resources array");
    let uri = resources[0].get("uri").and_then(|value| value.as_str()).expect("uri");

    let read_payload = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/read",
        "params": { "uri": uri },
    });
    let read_bytes = Bytes::from(serde_json::to_vec(&read_payload).expect("payload bytes"));
    let read_response = parse_request_sync(&state, &context, &read_bytes);
    assert_eq!(read_response.0, StatusCode::OK);
    let read_result = read_response.1.result.expect("result");
    let contents =
        read_result.get("contents").and_then(|value| value.as_array()).expect("contents array");
    assert!(!contents.is_empty());
    let resource = &contents[0];
    assert_eq!(resource.get("uri").and_then(|value| value.as_str()), Some(uri));
    let text = resource.get("text").and_then(|value| value.as_str()).unwrap_or_default();
    assert!(!text.is_empty());
}

// ============================================================================
// SECTION: Error Mapping
// ============================================================================

#[test]
fn rate_limited_error_maps_to_json_rpc() {
    let config = sample_config();
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let docs_provider = Arc::new(RateLimitedDocsProvider {
        retry_after_ms: 1500,
    });
    let router = sample_router_with_overrides(&config, Some(docs_provider), None);
    let state = build_server_state(router, &config.server, metrics, audit, None);
    let context = RequestContext::stdio();
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "decision_gate_docs_search",
            "arguments": {
                "query": "rate limit",
                "max_sections": 1
            }
        }
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::OK);
    let error = response.1.error.expect("error");
    assert_eq!(error.code, -32071);
    let data = error.data.expect("error data");
    assert!(data.retryable);
    assert_eq!(data.retry_after_ms, Some(1500));
}

#[test]
fn metrics_recorded_for_unauthenticated_list() {
    let mut config = sample_config();
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state =
        build_server_state(sample_router(&config), &config.server, metrics.clone(), audit, None);
    let context = RequestContext::http_with_correlation(
        ServerTransport::Http,
        Some(std::net::IpAddr::from([127, 0, 0, 1])),
        None,
        None,
        Some("client-123".to_string()),
        Some("srv-456".to_string()),
    );
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::UNAUTHORIZED);

    let events = metrics.events.lock().expect("events lock");
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.method, McpMethod::ToolsList);
    assert_eq!(event.outcome, McpOutcome::Error);
    assert_eq!(event.error_code, Some(-32001));
    assert_eq!(event.error_kind, Some("unauthenticated"));
    assert_eq!(event.unsafe_client_correlation_id.as_deref(), Some("client-123"));
    assert_eq!(event.server_correlation_id.as_deref(), Some("srv-456"));
    drop(events);
}

#[test]
fn unauthorized_response_includes_www_authenticate_header() {
    let mut config = sample_config();
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state = build_server_state(sample_router(&config), &config.server, metrics, audit, None);
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
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::UNAUTHORIZED);

    let headers = build_response_headers(&state, &context, &response.1);
    let challenge =
        headers.get(WWW_AUTHENTICATE).and_then(|value| value.to_str().ok()).unwrap_or("");
    assert!(challenge.starts_with("Bearer "));
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
    let state = build_server_state(sample_router(&config), &config.server, metrics, audit, None);
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
    let first = parse_request_sync(&state, &context, &bytes);
    assert_eq!(first.0, StatusCode::OK);
    let second = parse_request_sync(&state, &context, &bytes);
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
    let state = build_server_state(sample_router(&config), &config.server, metrics, audit, None);
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
    let response = parse_request_sync(&state, &context, &bytes);
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
        build_server_state(sample_router(&config), &config.server, metrics, audit.clone(), None);
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
    let _ = parse_request_sync(&state, &context, &bytes);
    let events = audit.events.lock().expect("events lock");
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.redaction, "evidence");
    drop(events);
}
