// decision-gate-mcp/src/tools/tests.rs
// ============================================================================
// Module: MCP Tool Router Unit Tests
// Description: Unit tests for MCP tool routing and runpack export behavior.
// Purpose: Validate tool flows, auth context usage, and storage integration.
// Dependencies: decision-gate-mcp, decision-gate-core, ret-logic
// ============================================================================

//! ## Overview
//! Exercises tool routing behavior with in-memory stores, schema registries,
//! and runpack storage stubs.
//!
//! Security posture: tests validate fail-closed behavior for tool routing and
//! storage integration boundaries; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions favor direct unwrap/expect for clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::ToolName;
use ret_logic::Requirement;
use serde_json::Value;
use serde_json::json;

use super::*;
use crate::McpNoopAuditSink;
use crate::NoopTenantAuthorizer;
use crate::NoopUsageMeter;
use crate::auth::DefaultToolAuthz;
use crate::auth::NoopAuditSink;
use crate::capabilities::CapabilityRegistry;
use crate::config::AnchorPolicyConfig;
use crate::config::DecisionGateConfig;
use crate::config::DevConfig;
use crate::config::DocsConfig;
use crate::config::EvidencePolicyConfig;
use crate::config::NamespaceConfig;
use crate::config::PolicyConfig;
use crate::config::PrincipalConfig;
use crate::config::PrincipalRoleConfig;
use crate::config::ProviderConfig;
use crate::config::ProviderDiscoveryConfig;
use crate::config::ProviderTimeoutConfig;
use crate::config::ProviderType;
use crate::config::RunStateStoreConfig;
use crate::config::SchemaRegistryConfig;
use crate::config::ServerAuthConfig;
use crate::config::ServerAuthMode;
use crate::config::ServerConfig;
use crate::config::ServerToolsConfig;
use crate::config::TrustConfig;
use crate::config::ValidationConfig;
use crate::docs::DocsCatalog;
use crate::evidence::FederatedEvidenceProvider;
use crate::namespace_authority::NoopNamespaceAuthority;
use crate::policy::DispatchPolicy;
use crate::registry_acl::PrincipalResolver;
use crate::registry_acl::RegistryAcl;
use crate::runpack_object_store::ObjectStoreClient;
use crate::runpack_object_store::ObjectStoreRunpackBackend;
use crate::runpack_storage::RunpackStorage;
use crate::runpack_storage::RunpackStorageError;
use crate::runpack_storage::RunpackStorageKey;
use crate::tools::ProviderTransport;
use crate::tools::SchemaRegistryLimits;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

struct CountingObjectStore {
    objects: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl CountingObjectStore {
    fn new() -> Self {
        Self {
            objects: Mutex::new(std::collections::BTreeMap::new()),
        }
    }

    fn keys(&self) -> Vec<String> {
        self.objects.lock().expect("lock").keys().cloned().collect()
    }
}

impl ObjectStoreClient for CountingObjectStore {
    fn put(
        &self,
        key: &str,
        bytes: Vec<u8>,
        _content_type: Option<&str>,
    ) -> Result<(), crate::runpack_object_store::ObjectStoreError> {
        self.objects
            .lock()
            .map_err(|_| {
                crate::runpack_object_store::ObjectStoreError::Io("lock poisoned".to_string())
            })?
            .insert(key.to_string(), bytes);
        Ok(())
    }

    fn get(
        &self,
        key: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, crate::runpack_object_store::ObjectStoreError> {
        let bytes = self
            .objects
            .lock()
            .map_err(|_| {
                crate::runpack_object_store::ObjectStoreError::Io("lock poisoned".to_string())
            })?
            .get(key)
            .ok_or_else(|| {
                crate::runpack_object_store::ObjectStoreError::Io("object not found".to_string())
            })?
            .clone();
        if bytes.len() > max_bytes {
            return Err(crate::runpack_object_store::ObjectStoreError::TooLarge {
                path: key.to_string(),
                max_bytes,
                actual_bytes: bytes.len(),
            });
        }
        Ok(bytes)
    }
}

struct PanicObjectStore;

impl ObjectStoreClient for PanicObjectStore {
    fn put(
        &self,
        _key: &str,
        _bytes: Vec<u8>,
        _content_type: Option<&str>,
    ) -> Result<(), crate::runpack_object_store::ObjectStoreError> {
        Err(crate::runpack_object_store::ObjectStoreError::Io("panic for coverage".to_string()))
    }

    fn get(
        &self,
        _key: &str,
        _max_bytes: usize,
    ) -> Result<Vec<u8>, crate::runpack_object_store::ObjectStoreError> {
        Err(crate::runpack_object_store::ObjectStoreError::Io("panic for coverage".to_string()))
    }
}

struct StubRunpackStorage {
    calls: Mutex<Vec<RunpackStorageKey>>,
}

impl StubRunpackStorage {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.lock().expect("lock").len()
    }
}

impl RunpackStorage for StubRunpackStorage {
    fn store_runpack(
        &self,
        key: &RunpackStorageKey,
        _source_dir: &Path,
    ) -> Result<Option<String>, RunpackStorageError> {
        self.calls.lock().expect("lock").push(key.clone());
        Ok(Some("memory://runpack".to_string()))
    }
}

struct StubDocsProvider {
    search_enabled: bool,
    resources_enabled: bool,
    resource_uri: String,
    search_calls: Mutex<usize>,
    list_calls: Mutex<usize>,
    read_calls: Mutex<usize>,
}

impl StubDocsProvider {
    fn new(search_enabled: bool, resources_enabled: bool) -> Self {
        Self {
            search_enabled,
            resources_enabled,
            resource_uri: format!("{}/stub-doc", crate::docs::RESOURCE_URI_PREFIX),
            search_calls: Mutex::new(0),
            list_calls: Mutex::new(0),
            read_calls: Mutex::new(0),
        }
    }

    fn search_calls(&self) -> usize {
        *self.search_calls.lock().expect("search calls lock")
    }

    fn list_calls(&self) -> usize {
        *self.list_calls.lock().expect("list calls lock")
    }

    fn read_calls(&self) -> usize {
        *self.read_calls.lock().expect("read calls lock")
    }
}

impl DocsProvider for StubDocsProvider {
    fn is_search_enabled(&self, _context: &RequestContext, _auth: &AuthContext) -> bool {
        self.search_enabled
    }

    fn is_resources_enabled(&self, _context: &RequestContext, _auth: &AuthContext) -> bool {
        self.resources_enabled
    }

    fn search(
        &self,
        _context: &RequestContext,
        _auth: &AuthContext,
        request: DocsSearchRequest,
    ) -> Result<crate::docs::SearchResult, ToolError> {
        let mut calls = self.search_calls.lock().expect("search calls lock");
        *calls += 1;
        Ok(crate::docs::SearchResult {
            sections: Vec::new(),
            docs_covered: Vec::new(),
            suggested_followups: vec![format!("echo: {}", request.query)],
        })
    }

    fn list_resources(
        &self,
        _context: &RequestContext,
        _auth: &AuthContext,
    ) -> Result<Vec<crate::docs::ResourceMetadata>, ToolError> {
        let mut calls = self.list_calls.lock().expect("list calls lock");
        *calls += 1;
        Ok(vec![crate::docs::ResourceMetadata {
            uri: self.resource_uri.clone(),
            name: "Stub Doc".to_string(),
            description: "Stub docs resource".to_string(),
            mime_type: "text/markdown",
        }])
    }

    fn read_resource(
        &self,
        _context: &RequestContext,
        _auth: &AuthContext,
        uri: &str,
    ) -> Result<crate::docs::ResourceContent, ToolError> {
        let mut calls = self.read_calls.lock().expect("read calls lock");
        *calls += 1;
        Ok(crate::docs::ResourceContent {
            uri: uri.to_string(),
            mime_type: "text/markdown",
            text: "stub docs body".to_string(),
        })
    }
}

struct StubToolVisibilityResolver {
    deny_for_list: BTreeSet<ToolName>,
    deny_for_call: BTreeSet<ToolName>,
}

impl StubToolVisibilityResolver {
    fn new(deny_for_list: BTreeSet<ToolName>, deny_for_call: BTreeSet<ToolName>) -> Self {
        Self {
            deny_for_list,
            deny_for_call,
        }
    }
}

impl ToolVisibilityResolver for StubToolVisibilityResolver {
    fn is_visible_for_list(
        &self,
        _context: &RequestContext,
        _auth: &AuthContext,
        tool: ToolName,
    ) -> bool {
        !self.deny_for_list.contains(&tool)
    }

    fn is_allowed_for_call(
        &self,
        _context: &RequestContext,
        _auth: &AuthContext,
        tool: ToolName,
    ) -> bool {
        !self.deny_for_call.contains(&tool)
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
                        tenant_id: None,
                        namespace_id: None,
                    }],
                }],
            }),
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::from_raw(1).expect("nonzero tenantid")],
            ..NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: builtin_providers(),
        dev: DevConfig::default(),
        docs: DocsConfig::default(),
        runpack_storage: None,
        source_modified_at: None,
    }
}

fn builtin_providers() -> Vec<ProviderConfig> {
    vec![builtin_provider("time")]
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

fn sample_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario-1"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-time"),
                requirement: Requirement::condition("after".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: vec![ConditionSpec {
            condition_id: "after".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("time"),
                check_id: "after".to_string(),
                params: Some(json!({"timestamp": 0})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(TenantId::from_raw(1).expect("nonzero tenantid")),
    }
}

fn router_with_config_and_backends(
    config: DecisionGateConfig,
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
) -> ToolRouter {
    router_with_overrides(config, runpack_storage, object_store, None, None)
}

fn router_with_overrides(
    mut config: DecisionGateConfig,
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
    docs_provider: Option<Arc<dyn DocsProvider>>,
    tool_visibility_resolver: Option<Arc<dyn ToolVisibilityResolver>>,
) -> ToolRouter {
    config.validate().expect("config");
    let evidence = FederatedEvidenceProvider::from_config(&config).expect("evidence");
    let capabilities = CapabilityRegistry::from_config(&config).expect("capabilities");
    let store = SharedRunStateStore::from_store(InMemoryRunStateStore::new());
    let schema_registry = SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new());
    let provider_transports = config
        .providers
        .iter()
        .map(|provider| {
            let transport = match provider.provider_type {
                ProviderType::Builtin => ProviderTransport::Builtin,
                ProviderType::Mcp => ProviderTransport::Mcp,
            };
            (provider.name.clone(), transport)
        })
        .collect::<BTreeMap<_, _>>();
    let schema_registry_limits = SchemaRegistryLimits {
        max_schema_bytes: config.schema_registry.max_schema_bytes,
        max_entries: config
            .schema_registry
            .max_entries
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
    };
    let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
    let registry_acl = RegistryAcl::new(&config.schema_registry.acl);
    let principal_resolver = PrincipalResolver::from_config(config.server.auth.as_ref());
    let default_namespace_tenants =
        config.namespace.default_tenants.iter().copied().collect::<BTreeSet<_>>();
    let docs_catalog = DocsCatalog::from_config(&config.docs).expect("docs catalog");
    ToolRouter::new(ToolRouterConfig {
        evidence,
        evidence_policy: config.evidence.clone(),
        validation: config.validation.clone(),
        dispatch_policy: DispatchPolicy::PermitAll,
        store,
        schema_registry,
        provider_transports,
        schema_registry_limits,
        capabilities: Arc::new(capabilities),
        provider_discovery: config.provider_discovery.clone(),
        authz,
        tenant_authorizer: Arc::new(NoopTenantAuthorizer),
        usage_meter: Arc::new(NoopUsageMeter),
        runpack_storage,
        runpack_object_store: object_store,
        audit: Arc::new(NoopAuditSink),
        trust_requirement: config.effective_trust_requirement(),
        anchor_policy: config.anchors.to_policy(),
        provider_trust_overrides: BTreeMap::new(),
        runpack_security_context: None,
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

fn router_with_backends(
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
) -> ToolRouter {
    router_with_config_and_backends(sample_config(), runpack_storage, object_store)
}

fn setup_router_with_run(
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
) -> (ToolRouter, ScenarioSpec, RunConfig) {
    let router = router_with_backends(runpack_storage, object_store);
    let spec = sample_spec();
    let context = RequestContext::stdio();
    router
        .define_scenario(
            &context,
            ScenarioDefineRequest {
                spec: spec.clone(),
            },
        )
        .expect("define scenario");
    let run_config = RunConfig {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: spec.scenario_id.clone(),
        dispatch_targets: Vec::<DispatchTarget>::new(),
        policy_tags: Vec::new(),
    };
    router
        .start_run(
            &context,
            ScenarioStartRequest {
                scenario_id: spec.scenario_id.clone(),
                run_config: run_config.clone(),
                started_at: Timestamp::UnixMillis(0),
                issue_entry_packets: false,
            },
        )
        .expect("start run");
    (router, spec, run_config)
}

#[test]
fn runpack_export_requires_output_dir_without_backend() {
    let (router, spec, run_config) = setup_router_with_run(None, None);
    let request = RunpackExportRequest {
        scenario_id: spec.scenario_id,
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id,
        output_dir: None,
        manifest_name: None,
        generated_at: Timestamp::UnixMillis(0),
        include_verification: false,
    };
    let err = router.export_runpack(&RequestContext::stdio(), &request).expect_err("error");
    assert!(err.to_string().contains("output_dir"));
}

#[test]
fn runpack_export_uses_object_store_backend() {
    let store = Arc::new(CountingObjectStore::new());
    let backend = Arc::new(ObjectStoreRunpackBackend::from_client("bucket", store.clone()));
    let (router, spec, run_config) = setup_router_with_run(None, Some(backend));
    let request = RunpackExportRequest {
        scenario_id: spec.scenario_id,
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id,
        output_dir: None,
        manifest_name: None,
        generated_at: Timestamp::UnixMillis(0),
        include_verification: false,
    };
    let response =
        router.export_runpack(&RequestContext::stdio(), &request).expect("runpack export");
    assert!(response.storage_uri.is_some());
    let keys = store.keys();
    assert!(keys.iter().any(|key| key.ends_with("manifest.json")));
}

#[test]
fn runpack_export_prefers_runpack_storage_over_object_store() {
    let backend =
        Arc::new(ObjectStoreRunpackBackend::from_client("bucket", Arc::new(PanicObjectStore)));
    let runpack_storage = Arc::new(StubRunpackStorage::new());
    let (router, spec, run_config) =
        setup_router_with_run(Some(runpack_storage.clone()), Some(backend));
    let request = RunpackExportRequest {
        scenario_id: spec.scenario_id,
        tenant_id: run_config.tenant_id,
        namespace_id: run_config.namespace_id,
        run_id: run_config.run_id,
        output_dir: None,
        manifest_name: None,
        generated_at: Timestamp::UnixMillis(0),
        include_verification: false,
    };
    let response =
        router.export_runpack(&RequestContext::stdio(), &request).expect("runpack export");
    assert_eq!(runpack_storage.call_count(), 1);
    assert_eq!(response.storage_uri, Some("memory://runpack".to_string()));
}

// ============================================================================
// SECTION: Docs + Tool Visibility Tests
// ============================================================================

#[test]
fn list_tools_includes_docs_search_by_default() {
    let router = router_with_backends(None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");
    assert!(tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch));
}

#[test]
fn list_tools_hides_docs_search_when_disabled() {
    let mut config = sample_config();
    config.docs.enabled = false;
    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");
    assert!(!tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch));
}

#[test]
fn list_tools_hides_docs_search_when_denied() {
    let mut config = sample_config();
    config.server.tools.denylist = vec![ToolName::DecisionGateDocsSearch.as_str().to_string()];
    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");
    assert!(!tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch));
}

// ============================================================================
// SECTION: Tool Visibility Tests (12 tests)
// ============================================================================

#[test]
fn tool_visibility_filter_mode_empty_lists_shows_all() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = Vec::new();
    config.server.tools.denylist = Vec::new();

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // With filter mode and empty lists, all enabled tools should be visible
    assert!(!tools.is_empty(), "should show tools with empty lists in filter mode");
}

#[test]
fn tool_visibility_filter_mode_allowlist_only_filters() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = vec![
        ToolName::ScenarioDefine.as_str().to_string(),
        ToolName::ScenarioStart.as_str().to_string(),
    ];
    config.server.tools.denylist = Vec::new();

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Only allowlisted tools should be visible
    assert!(
        tools.iter().any(|t| t.name == ToolName::ScenarioDefine),
        "allowlisted tool should appear"
    );
    assert!(
        tools.iter().any(|t| t.name == ToolName::ScenarioStart),
        "allowlisted tool should appear"
    );
    assert!(
        tools
            .iter()
            .all(|t| t.name == ToolName::ScenarioDefine || t.name == ToolName::ScenarioStart),
        "only allowlisted tools should appear"
    );
}

#[test]
fn tool_visibility_filter_mode_denylist_only_filters() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = Vec::new();
    config.server.tools.denylist = vec![ToolName::ScenarioDefine.as_str().to_string()];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Denylisted tool should not appear
    assert!(
        !tools.iter().any(|t| t.name == ToolName::ScenarioDefine),
        "denylisted tool should not appear"
    );
    assert!(!tools.is_empty(), "other tools should still appear");
}

#[test]
fn tool_visibility_filter_mode_denylist_overrides_allowlist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = vec![ToolName::ScenarioDefine.as_str().to_string()];
    config.server.tools.denylist = vec![ToolName::ScenarioDefine.as_str().to_string()];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Denylist should take precedence - tool should not appear
    assert!(
        !tools.iter().any(|t| t.name == ToolName::ScenarioDefine),
        "denylist should override allowlist"
    );
}

#[test]
fn tool_visibility_passthrough_mode_ignores_allowlist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Passthrough;
    config.server.tools.allowlist = vec![ToolName::ScenarioDefine.as_str().to_string()];
    config.server.tools.denylist = Vec::new();

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Passthrough mode should ignore allowlist and show all enabled tools
    assert!(tools.len() > 1, "passthrough mode should show all enabled tools");
}

#[test]
fn tool_visibility_passthrough_mode_ignores_denylist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Passthrough;
    config.server.tools.allowlist = Vec::new();
    config.server.tools.denylist = vec![ToolName::ScenarioDefine.as_str().to_string()];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Passthrough mode should ignore denylist - tool should appear
    assert!(
        tools.iter().any(|t| t.name == ToolName::ScenarioDefine),
        "passthrough mode should ignore denylist"
    );
}

#[test]
fn tool_visibility_filter_preserves_order() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = vec![
        ToolName::ScenarioStart.as_str().to_string(),
        ToolName::ScenarioDefine.as_str().to_string(),
    ];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Tools should maintain their canonical order, not allowlist order
    let define_idx = tools.iter().position(|t| t.name == ToolName::ScenarioDefine);
    let start_idx = tools.iter().position(|t| t.name == ToolName::ScenarioStart);

    if let (Some(define_pos), Some(start_pos)) = (define_idx, start_idx) {
        assert!(define_pos < start_pos, "tools should maintain canonical order");
    }
}

#[test]
fn tool_visibility_empty_allowlist_shows_all_in_filter_mode() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = Vec::new();
    config.server.tools.denylist = Vec::new();

    let router = router_with_config_and_backends(config.clone(), None, None);
    let tools1 = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Compare with passthrough mode
    config.server.tools.mode = ToolVisibilityMode::Passthrough;
    let router2 = router_with_config_and_backends(config, None, None);
    let tools2 = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router2.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Both should show same tools when lists are empty
    assert_eq!(tools1.len(), tools2.len(), "empty filter and passthrough should show same tools");
}

#[test]
fn tool_visibility_multiple_tools_in_denylist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.denylist = vec![
        ToolName::ScenarioDefine.as_str().to_string(),
        ToolName::ScenarioStart.as_str().to_string(),
        ToolName::ScenarioNext.as_str().to_string(),
    ];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // All denylisted tools should be hidden
    assert!(!tools.iter().any(|t| t.name == ToolName::ScenarioDefine));
    assert!(!tools.iter().any(|t| t.name == ToolName::ScenarioStart));
    assert!(!tools.iter().any(|t| t.name == ToolName::ScenarioNext));
}

#[test]
fn tool_visibility_list_tools_returns_valid_contracts() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // All returned tools should have valid contracts
    for tool in tools {
        assert!(!tool.name.as_str().is_empty(), "tool name should not be empty");
        assert!(tool.input_schema.is_object(), "input schema should be object");
    }
}

#[test]
fn tool_visibility_disabled_tool_never_appears() {
    let mut config = sample_config();
    config.docs.enabled = false; // Disable docs entirely

    // Even with docs in allowlist, it shouldn't appear if disabled
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = vec![ToolName::DecisionGateDocsSearch.as_str().to_string()];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Disabled tool should not appear even if allowlisted
    assert!(
        !tools.iter().any(|t| t.name == ToolName::DecisionGateDocsSearch),
        "disabled tool should not appear"
    );
}

// ============================================================================
// SECTION: Docs Search Handler Tests (12 tests)
// ============================================================================

fn call_docs_search(router: &ToolRouter, request: Value) -> Value {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.handle_tool_call(
            &RequestContext::stdio(),
            ToolName::DecisionGateDocsSearch.as_str(),
            request,
        ))
        .expect("call tool")
}

#[test]
fn docs_search_handler_returns_sections() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);

    let request = json!({
        "query": "provider evidence",
        "max_sections": 5
    });

    let response = call_docs_search(&router, request);
    assert!(response.get("sections").is_some(), "should have sections field");
    assert!(response["sections"].is_array(), "sections should be array");
}

#[test]
fn docs_search_handler_respects_max_sections() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);

    let request = json!({
        "query": "provider",
        "max_sections": 2
    });

    let response = call_docs_search(&router, request);
    let sections = response["sections"].as_array().expect("sections array");
    assert!(sections.len() <= 2, "should respect max_sections=2");
}

#[test]
fn docs_search_handler_empty_query_returns_overview() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);

    let request = json!({
        "query": "",
        "max_sections": 4
    });

    let response = call_docs_search(&router, request);
    assert!(response.get("sections").is_some(), "should return overview sections");
    assert!(response.get("suggested_followups").is_some(), "should suggest followups");
}

#[test]
fn docs_search_disabled_when_docs_disabled() {
    let mut config = sample_config();
    config.docs.enabled = false;

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Docs search should not appear when docs disabled
    assert!(
        !tools.iter().any(|t| t.name == ToolName::DecisionGateDocsSearch),
        "docs search should be disabled when docs.enabled=false"
    );
}

#[test]
fn docs_search_disabled_when_search_disabled() {
    let mut config = sample_config();
    config.docs.enable_search = false;

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Docs search should not appear when enable_search=false
    assert!(
        !tools.iter().any(|t| t.name == ToolName::DecisionGateDocsSearch),
        "docs search should be disabled when docs.enable_search=false"
    );
}

#[test]
fn docs_search_uses_catalog_from_config() {
    let mut config = sample_config();
    config.docs.max_sections = 5;

    let router = router_with_config_and_backends(config, None, None);
    let request = json!({
        "query": "provider",
        "max_sections": 10 // Request more than config allows
    });

    let response = call_docs_search(&router, request);
    let sections = response["sections"].as_array().expect("sections array");

    // Should be clamped by config.docs.max_sections
    assert!(sections.len() <= 5, "should respect config max_sections");
}

#[test]
fn docs_search_hidden_when_in_denylist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.denylist = vec![ToolName::DecisionGateDocsSearch.as_str().to_string()];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    assert!(
        !tools.iter().any(|t| t.name == ToolName::DecisionGateDocsSearch),
        "docs search should be hidden when denylisted"
    );
}

#[test]
fn docs_search_shown_when_in_allowlist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist = vec![ToolName::DecisionGateDocsSearch.as_str().to_string()];

    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    assert!(
        tools.iter().any(|t| t.name == ToolName::DecisionGateDocsSearch),
        "docs search should appear when allowlisted"
    );
}

#[test]
fn docs_search_shown_by_default_when_enabled() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");

    // Should appear by default when docs.enabled=true and docs.enable_search=true
    assert!(
        tools.iter().any(|t| t.name == ToolName::DecisionGateDocsSearch),
        "docs search should appear by default when enabled"
    );
}

#[test]
fn docs_search_returns_docs_covered() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);

    let request = json!({
        "query": "provider check",
        "max_sections": 5
    });

    let response = call_docs_search(&router, request);
    assert!(response.get("docs_covered").is_some(), "should have docs_covered field");
    assert!(response["docs_covered"].is_array(), "docs_covered should be array");
}

#[test]
fn docs_search_returns_suggested_followups() {
    let config = sample_config();
    let router = router_with_config_and_backends(config, None, None);

    let request = json!({
        "query": "provider",
        "max_sections": 3
    });

    let response = call_docs_search(&router, request);
    assert!(response.get("suggested_followups").is_some(), "should have suggested_followups");
    assert!(response["suggested_followups"].is_array(), "suggested_followups should be array");
}

#[test]
fn docs_search_handles_empty_catalog() {
    let mut config = sample_config();
    config.docs.include_default_docs = false; // No docs in catalog

    let router = router_with_config_and_backends(config, None, None);
    let request = json!({
        "query": "provider",
        "max_sections": 5
    });

    let response = call_docs_search(&router, request);
    let sections = response["sections"].as_array().expect("sections array");
    assert!(sections.is_empty(), "empty catalog should return no results");
}

// ============================================================================
// SECTION: Docs Provider Overrides (6 tests)
// ============================================================================

#[test]
fn docs_provider_disables_search_hides_tool_and_rejects_call() {
    let config = sample_config();
    let docs_provider = Arc::new(StubDocsProvider::new(false, true));
    let docs_provider_trait: Arc<dyn DocsProvider> = docs_provider.clone();
    let router = router_with_overrides(config, None, None, Some(docs_provider_trait), None);
    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");
    assert!(
        !tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch),
        "docs search should be hidden when docs provider disables search",
    );

    let err = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.handle_tool_call(
            &RequestContext::stdio(),
            ToolName::DecisionGateDocsSearch.as_str(),
            json!({ "query": "docs" }),
        ))
        .expect_err("docs search should be blocked");
    assert!(matches!(err, ToolError::UnknownTool));
}

#[test]
fn docs_provider_search_is_invoked_for_docs_tool() {
    let config = sample_config();
    let docs_provider = Arc::new(StubDocsProvider::new(true, true));
    let docs_provider_trait: Arc<dyn DocsProvider> = docs_provider.clone();
    let router = router_with_overrides(config, None, None, Some(docs_provider_trait), None);

    let response = call_docs_search(&router, json!({ "query": "hello docs", "max_sections": 2 }));
    assert_eq!(docs_provider.search_calls(), 1);
    let followups = response["suggested_followups"].as_array().expect("followups array");
    assert!(
        followups.iter().any(|value| value.as_str() == Some("echo: hello docs")),
        "docs provider should shape followup content",
    );
}

#[test]
fn docs_provider_disables_resources_blocks_list_and_read() {
    let config = sample_config();
    let docs_provider = Arc::new(StubDocsProvider::new(true, false));
    let docs_provider_trait: Arc<dyn DocsProvider> = docs_provider.clone();
    let router = router_with_overrides(config, None, None, Some(docs_provider_trait), None);

    let err = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_resources(&RequestContext::stdio()))
        .expect_err("resources list should be blocked");
    assert!(matches!(err, ToolError::UnknownTool));
    assert_eq!(docs_provider.list_calls(), 0);

    let err = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.read_resource(&RequestContext::stdio(), "decision-gate://docs/stub"))
        .expect_err("resources read should be blocked");
    assert!(matches!(err, ToolError::UnknownTool));
    assert_eq!(docs_provider.read_calls(), 0);
}

#[test]
fn docs_provider_resources_enabled_lists_and_reads() {
    let config = sample_config();
    let docs_provider = Arc::new(StubDocsProvider::new(true, true));
    let docs_provider_trait: Arc<dyn DocsProvider> = docs_provider.clone();
    let router = router_with_overrides(config, None, None, Some(docs_provider_trait), None);

    let resources = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_resources(&RequestContext::stdio()))
        .expect("list resources");
    assert_eq!(resources.len(), 1);
    assert_eq!(docs_provider.list_calls(), 1);
    let uri = resources[0].uri.clone();

    let content = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.read_resource(&RequestContext::stdio(), &uri))
        .expect("read resource");
    assert_eq!(content.uri, uri);
    assert_eq!(content.mime_type, "text/markdown");
    assert_eq!(docs_provider.read_calls(), 1);
}

#[test]
fn tool_visibility_resolver_blocks_list_and_call() {
    let config = sample_config();
    let deny_list = vec![ToolName::ScenarioDefine].into_iter().collect();
    let deny_call = vec![ToolName::ScenarioDefine].into_iter().collect();
    let resolver = Arc::new(StubToolVisibilityResolver::new(deny_list, deny_call));
    let router = router_with_overrides(config, None, None, None, Some(resolver));

    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");
    assert!(!tools.iter().any(|tool| tool.name == ToolName::ScenarioDefine));

    let err = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.handle_tool_call(
            &RequestContext::stdio(),
            ToolName::ScenarioDefine.as_str(),
            json!({}),
        ))
        .expect_err("call should be blocked");
    assert!(matches!(err, ToolError::UnknownTool));
}

#[test]
fn tool_visibility_resolver_overrides_config_denylist() {
    let mut config = sample_config();
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.denylist = vec![ToolName::ScenarioDefine.as_str().to_string()];
    let resolver = Arc::new(StubToolVisibilityResolver::new(BTreeSet::new(), BTreeSet::new()));
    let router = router_with_overrides(config, None, None, None, Some(resolver));

    let tools = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.list_tools(&RequestContext::stdio()))
        .expect("list tools");
    assert!(
        tools.iter().any(|tool| tool.name == ToolName::ScenarioDefine),
        "custom resolver should override config denylist",
    );
}
