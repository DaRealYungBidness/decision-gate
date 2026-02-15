// crates/decision-gate-mcp/src/server/tests.rs
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
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use axum::body::Bytes;
use axum::body::to_bytes;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::http::header::WWW_AUTHENTICATE;
use axum::response::IntoResponse;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::StageId;
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use serde_json::json;

use super::JsonRpcResponse;
use super::McpServer;
use super::ReadinessState;
use super::ServerState;
use super::build_provider_transports;
use super::build_response_headers;
use super::build_run_state_store;
use super::build_schema_registry;
use super::build_schema_registry_limits;
use super::build_server_state;
use super::handle_health;
use super::handle_mutation_stats;
use super::handle_ready;
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
use crate::config::RunStateStoreType;
use crate::config::SchemaRegistryConfig;
use crate::config::SchemaRegistryType;
use crate::config::ServerAuthConfig;
use crate::config::ServerAuthMode;
use crate::config::ServerConfig;
use crate::config::ServerToolsConfig;
use crate::config::ServerTransport;
use crate::config::TrustConfig;
use crate::config::ValidationConfig;
use crate::correlation::CLIENT_CORRELATION_HEADER;
use crate::correlation::SERVER_CORRELATION_HEADER;
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
use crate::tools::EvidenceQueryRequest;
use crate::tools::EvidenceQueryResponse;
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

fn readiness_for_tests() -> Arc<ReadinessState> {
    Arc::new(ReadinessState::new(
        SharedRunStateStore::from_store(InMemoryRunStateStore::new()),
        SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new()),
    ))
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

// ============================================================================
// SECTION: Health Checks
// ============================================================================//

#[tokio::test]
async fn health_endpoint_ok() {
    let response = handle_health().await.into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get(CONTENT_TYPE).expect("content type");
    assert_eq!(content_type, "application/json");
}

#[test]
fn ready_endpoint_ok() {
    let state = Arc::new(sample_server_state());
    let response = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(handle_ready(State(state)))
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get(CONTENT_TYPE).expect("content type");
    assert_eq!(content_type, "application/json");
}

#[test]
fn ready_endpoint_not_ready_when_store_unavailable() {
    let store = SharedRunStateStore::from_store(FailingRunStateStore);
    let registry = SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new());
    let readiness = Arc::new(ReadinessState::new(store, registry));
    let config = sample_config();
    let router = sample_router(&config);
    let state = Arc::new(build_server_state(
        router,
        &config.server,
        Arc::new(TestMetrics::default()),
        Arc::new(McpNoopAuditSink),
        None,
        readiness,
    ));
    let response = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(handle_ready(State(state)))
        .into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let content_type = response.headers().get(CONTENT_TYPE).expect("content type");
    assert_eq!(content_type, "application/json");
}

fn sample_server_state() -> ServerState {
    let config = sample_config();
    let router = sample_router(&config);
    let readiness = Arc::new(ReadinessState::new(
        SharedRunStateStore::from_store(InMemoryRunStateStore::new()),
        SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new()),
    ));
    build_server_state(
        router,
        &config.server,
        Arc::new(TestMetrics::default()),
        Arc::new(McpNoopAuditSink),
        None,
        readiness,
    )
}

struct FailingRunStateStore;

impl decision_gate_core::RunStateStore for FailingRunStateStore {
    fn load(
        &self,
        _tenant_id: &TenantId,
        _namespace_id: &NamespaceId,
        _run_id: &decision_gate_core::RunId,
    ) -> Result<Option<decision_gate_core::RunState>, StoreError> {
        Ok(None)
    }

    fn save(&self, _state: &decision_gate_core::RunState) -> Result<(), StoreError> {
        Ok(())
    }

    fn readiness(&self) -> Result<(), StoreError> {
        Err(StoreError::Store("store unavailable".to_string()))
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
    let config = match name {
        "json" => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let mut table = toml::value::Table::new();
            table.insert(
                "root".to_string(),
                toml::Value::String(root.to_string_lossy().to_string()),
            );
            table.insert("root_id".to_string(), toml::Value::String("mcp-tests-root".to_string()));
            table.insert("allow_yaml".to_string(), toml::Value::Boolean(true));
            table.insert("max_bytes".to_string(), toml::Value::Integer(1024 * 1024));
            Some(toml::Value::Table(table))
        }
        _ => None,
    };
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
        config,
    }
}

fn parse_request_sync(
    state: &ServerState,
    context: &RequestContext,
    bytes: &Bytes,
) -> (StatusCode, JsonRpcResponse) {
    tokio::runtime::Runtime::new().expect("runtime").block_on(parse_request(state, context, bytes))
}

fn parse_json_body_sync(response: axum::response::Response) -> serde_json::Value {
    let bytes = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(to_bytes(response.into_body(), usize::MAX))
        .expect("response body bytes");
    serde_json::from_slice(&bytes).expect("response body json")
}

fn server_state_from_config(config: DecisionGateConfig) -> ServerState {
    let McpServer {
        config,
        router,
        metrics,
        audit,
        auth_challenge,
        readiness,
    } = McpServer::from_config(config).expect("server");
    build_server_state(router, &config.server, metrics, audit, auth_challenge, readiness)
}

fn evidence_context_for_tests() -> EvidenceContext {
    EvidenceContext {
        tenant_id: TenantId::from_raw(100).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run"),
        scenario_id: ScenarioId::new("scenario"),
        stage_id: StageId::new("stage"),
        trigger_id: TriggerId::new("trigger"),
        trigger_time: Timestamp::Logical(1),
        correlation_id: None,
    }
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
fn read_framed_rejects_oversized_headers() {
    let oversized = "x".repeat(9_000);
    let framed = format!("{oversized}\r\n\r\n");
    let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
    let result = read_framed(&mut reader, 1024);
    assert!(result.is_err());
}

#[test]
fn read_framed_rejects_duplicate_content_length_headers() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let framed = format!(
        "Content-Length: {}\r\nContent-Length: {}\r\n\r\n{}",
        payload.len(),
        payload.len(),
        String::from_utf8_lossy(payload)
    );
    let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
    let result = read_framed(&mut reader, payload.len());
    assert!(result.is_err());
}

#[test]
fn parse_request_rejects_payload_over_limit() {
    let mut config = sample_config();
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/list",
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    config.server.max_body_bytes = bytes.len() - 1;
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit,
        None,
        readiness_for_tests(),
    );
    let context = RequestContext::stdio();
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::PAYLOAD_TOO_LARGE);
    let error = response.1.error.expect("error");
    assert_eq!(error.code, -32070);
}

#[test]
fn metrics_recorded_for_tools_list() {
    let mut config = sample_config();
    config.server.limits.max_inflight = 1;
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics.clone(),
        audit,
        None,
        readiness_for_tests(),
    );
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

#[test]
fn from_config_tools_list_round_trip() {
    let config = sample_config();
    let state = server_state_from_config(config);
    let context = RequestContext::stdio();
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::OK);
    let result = response.1.result.expect("result");
    let tools = result.get("tools").and_then(|value| value.as_array()).expect("tools array");
    assert!(!tools.is_empty());
}

#[test]
fn from_config_tools_call_evidence_query_round_trip() {
    let config = sample_config();
    let state = server_state_from_config(config);
    let context = RequestContext::stdio();
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            check_id: "now".to_string(),
            params: None,
        },
        context: evidence_context_for_tests(),
    };
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "evidence_query",
            "arguments": request,
        }
    });
    let bytes = Bytes::from(serde_json::to_vec(&payload).expect("payload bytes"));
    let response = parse_request_sync(&state, &context, &bytes);
    assert_eq!(response.0, StatusCode::OK);
    let result = response.1.result.expect("result");
    let content_entry = result
        .get("content")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .expect("content entry");
    let json_value = content_entry.get("json").expect("json payload");
    let response: EvidenceQueryResponse =
        serde_json::from_value(json_value.clone()).expect("evidence response");
    assert!(response.result.evidence_hash.is_some());
}

// ============================================================================
// SECTION: Resources Endpoints
// ============================================================================

#[test]
fn resources_list_returns_embedded_docs() {
    let config = sample_config();
    let metrics = Arc::new(TestMetrics::default());
    let audit = Arc::new(TestAudit::default());
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit,
        None,
        readiness_for_tests(),
    );
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
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit,
        None,
        readiness_for_tests(),
    );
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
    let state =
        build_server_state(router, &config.server, metrics, audit, None, readiness_for_tests());
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
fn mutation_stats_requires_auth_in_bearer_mode() {
    let mut config = sample_config();
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:0".to_string());
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    let state = Arc::new(server_state_from_config(config));
    let response =
        tokio::runtime::Runtime::new().expect("runtime").block_on(handle_mutation_stats(
            State(state),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 3456))),
            HeaderMap::new(),
        ));
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn mutation_stats_rejects_invalid_correlation_header() {
    let state = Arc::new(sample_server_state());
    let mut headers = HeaderMap::new();
    let invalid_correlation = "a".repeat(256);
    headers.insert(
        CLIENT_CORRELATION_HEADER,
        HeaderValue::from_str(&invalid_correlation).expect("header value"),
    );

    let response =
        tokio::runtime::Runtime::new().expect("runtime").block_on(handle_mutation_stats(
            State(state),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4321))),
            headers,
        ));
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        response.headers().contains_key(SERVER_CORRELATION_HEADER),
        "invalid correlation response should include server correlation header"
    );
    assert!(
        !response.headers().contains_key(CLIENT_CORRELATION_HEADER),
        "rejected client correlation id should not be echoed back"
    );
    let body = parse_json_body_sync(response);
    let error = body.get("error").and_then(serde_json::Value::as_object).expect("error object");
    assert_eq!(error.get("code").and_then(serde_json::Value::as_i64), Some(-32073));
}

#[test]
fn build_run_state_store_with_custom_sqlite_batch_settings_succeeds() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut config = sample_config();
    config.run_state_store.store_type = RunStateStoreType::Sqlite;
    config.run_state_store.path = Some(temp.path().join("run_state.sqlite"));
    config.run_state_store.writer_queue_capacity = 8;
    config.run_state_store.batch_max_ops = 5;
    config.run_state_store.batch_max_bytes = 4 * 1024;
    config.run_state_store.batch_max_wait_ms = 3;
    config.run_state_store.read_pool_size = 2;

    let store = build_run_state_store(&config).expect("build run state store");
    store.readiness().expect("run state store readiness");
}

#[test]
fn build_schema_registry_with_custom_sqlite_batch_settings_succeeds() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut config = sample_config();
    config.schema_registry.registry_type = SchemaRegistryType::Sqlite;
    config.schema_registry.path = Some(temp.path().join("registry.sqlite"));
    config.schema_registry.writer_queue_capacity = 9;
    config.schema_registry.batch_max_ops = 6;
    config.schema_registry.batch_max_bytes = 8 * 1024;
    config.schema_registry.batch_max_wait_ms = 4;
    config.schema_registry.read_pool_size = 3;

    let registry = build_schema_registry(&config).expect("build schema registry");
    registry.readiness().expect("schema registry readiness");
}

#[test]
fn mutation_stats_allows_authorized_bearer() {
    let mut config = sample_config();
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:0".to_string());
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    let state = Arc::new(server_state_from_config(config));
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer token"));
    let response =
        tokio::runtime::Runtime::new().expect("runtime").block_on(handle_mutation_stats(
            State(state),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4567))),
            headers,
        ));
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn mutation_stats_response_shape_stable() {
    let state = Arc::new(sample_server_state());
    let response =
        tokio::runtime::Runtime::new().expect("runtime").block_on(handle_mutation_stats(
            State(state),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 5678))),
            HeaderMap::new(),
        ));
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json_body_sync(response);
    let lock_wait_buckets = body
        .get("lock_wait_buckets_us")
        .and_then(serde_json::Value::as_array)
        .expect("lock wait buckets");
    let lock_wait_histogram = body
        .get("lock_wait_histogram")
        .and_then(serde_json::Value::as_array)
        .expect("lock wait histogram");
    let queue_depth_buckets = body
        .get("queue_depth_buckets")
        .and_then(serde_json::Value::as_array)
        .expect("queue depth buckets");
    let queue_depth_histogram = body
        .get("queue_depth_histogram")
        .and_then(serde_json::Value::as_array)
        .expect("queue depth histogram");
    assert_eq!(lock_wait_histogram.len(), lock_wait_buckets.len().saturating_add(1));
    assert_eq!(queue_depth_histogram.len(), queue_depth_buckets.len().saturating_add(1));
    assert!(body.get("lock_acquisitions").is_some());
    assert!(body.get("active_holders").is_some());
    assert!(body.get("pending_waiters").is_some());
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
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics.clone(),
        audit,
        None,
        readiness_for_tests(),
    );
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
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit,
        None,
        readiness_for_tests(),
    );
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
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit,
        None,
        readiness_for_tests(),
    );
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
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit,
        None,
        readiness_for_tests(),
    );
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
    let state = build_server_state(
        sample_router(&config),
        &config.server,
        metrics,
        audit.clone(),
        None,
        readiness_for_tests(),
    );
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
