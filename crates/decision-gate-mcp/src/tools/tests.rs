// crates/decision-gate-mcp/src/tools/tests.rs
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
use std::sync::Condvar;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapePage;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeRegistryError;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::NextRequest;
use decision_gate_core::PacketPayload;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::StatusRequest;
use decision_gate_core::StoreError;
use decision_gate_core::SubmitRequest;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::ToolName;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerKind;
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
        {
            let mut calls = self.search_calls.lock().expect("search calls lock");
            *calls += 1;
        }
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
        {
            let mut calls = self.list_calls.lock().expect("list calls lock");
            *calls += 1;
        }
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
        {
            let mut calls = self.read_calls.lock().expect("read calls lock");
            *calls += 1;
        }
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

#[derive(Default)]
struct BlockingSaveState {
    blocked: bool,
    released: bool,
}

#[derive(Clone)]
struct BlockingRunStateStore {
    inner: InMemoryRunStateStore,
    blocked_run_id: String,
    block_state: Arc<(Mutex<BlockingSaveState>, Condvar)>,
}

impl BlockingRunStateStore {
    fn new(blocked_run_id: &str) -> Self {
        Self {
            inner: InMemoryRunStateStore::new(),
            blocked_run_id: blocked_run_id.to_string(),
            block_state: Arc::new((Mutex::new(BlockingSaveState::default()), Condvar::new())),
        }
    }

    fn wait_until_blocked(&self, timeout: Duration) -> bool {
        let (lock, cv) = &*self.block_state;
        let deadline = Instant::now() + timeout;
        let mut guard = lock.lock().expect("blocking run state mutex");
        while !guard.blocked {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let wait_for = deadline.saturating_duration_since(now);
            let (next_guard, wait_result) =
                cv.wait_timeout(guard, wait_for).expect("blocking run state wait");
            guard = next_guard;
            if wait_result.timed_out() && !guard.blocked {
                return false;
            }
        }
        true
    }

    fn release_block(&self) {
        let (lock, cv) = &*self.block_state;
        {
            let mut guard = lock.lock().expect("blocking run state mutex");
            guard.released = true;
        }
        cv.notify_all();
    }
}

impl RunStateStore for BlockingRunStateStore {
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        self.inner.load(tenant_id, namespace_id, run_id)
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        if state.run_id.as_str() == self.blocked_run_id {
            let (lock, cv) = &*self.block_state;
            let mut guard = lock.lock().expect("blocking run state mutex");
            if !guard.released {
                guard.blocked = true;
                cv.notify_all();
                while !guard.released {
                    guard = cv.wait(guard).expect("blocking run state wait");
                }
                drop(guard);
            }
        }
        self.inner.save(state)
    }

    fn readiness(&self) -> Result<(), StoreError> {
        Ok(())
    }
}

struct OverloadedRunStateStore {
    message: String,
    retry_after_ms: Option<u64>,
}

impl RunStateStore for OverloadedRunStateStore {
    fn load(
        &self,
        _tenant_id: &TenantId,
        _namespace_id: &NamespaceId,
        _run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        Ok(None)
    }

    fn save(&self, _state: &RunState) -> Result<(), StoreError> {
        Err(StoreError::Overloaded {
            message: self.message.clone(),
            retry_after_ms: self.retry_after_ms,
        })
    }

    fn readiness(&self) -> Result<(), StoreError> {
        Ok(())
    }
}

struct OverloadedRegistry {
    message: String,
    retry_after_ms: Option<u64>,
}

impl DataShapeRegistry for OverloadedRegistry {
    fn register(&self, _record: DataShapeRecord) -> Result<(), DataShapeRegistryError> {
        Err(DataShapeRegistryError::Overloaded {
            message: self.message.clone(),
            retry_after_ms: self.retry_after_ms,
        })
    }

    fn get(
        &self,
        _tenant_id: &TenantId,
        _namespace_id: &NamespaceId,
        _schema_id: &DataShapeId,
        _version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
        Ok(None)
    }

    fn list(
        &self,
        _tenant_id: &TenantId,
        _namespace_id: &NamespaceId,
        _cursor: Option<String>,
        _limit: usize,
    ) -> Result<DataShapePage, DataShapeRegistryError> {
        Ok(DataShapePage {
            items: Vec::new(),
            next_token: None,
        })
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
    config: DecisionGateConfig,
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
    docs_provider: Option<Arc<dyn DocsProvider>>,
    tool_visibility_resolver: Option<Arc<dyn ToolVisibilityResolver>>,
) -> ToolRouter {
    router_with_store_registry_overrides(
        config,
        runpack_storage,
        object_store,
        docs_provider,
        tool_visibility_resolver,
        None,
        None,
    )
}

fn router_with_store_registry_overrides(
    mut config: DecisionGateConfig,
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
    docs_provider: Option<Arc<dyn DocsProvider>>,
    tool_visibility_resolver: Option<Arc<dyn ToolVisibilityResolver>>,
    store: Option<SharedRunStateStore>,
    schema_registry: Option<SharedDataShapeRegistry>,
) -> ToolRouter {
    config.validate().expect("config");
    let evidence = FederatedEvidenceProvider::from_config(&config).expect("evidence");
    let capabilities = CapabilityRegistry::from_config(&config).expect("capabilities");
    let store =
        store.unwrap_or_else(|| SharedRunStateStore::from_store(InMemoryRunStateStore::new()));
    let schema_registry = schema_registry.unwrap_or_else(|| {
        SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new())
    });
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

fn scenario_start_payload(
    spec: &ScenarioSpec,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    run_id: &str,
) -> Value {
    serde_json::to_value(ScenarioStartRequest {
        scenario_id: spec.scenario_id.clone(),
        run_config: RunConfig {
            tenant_id,
            namespace_id,
            run_id: RunId::new(run_id),
            scenario_id: spec.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        },
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    })
    .expect("scenario start payload")
}

fn schemas_register_payload(tenant_id: TenantId, namespace_id: NamespaceId) -> Value {
    serde_json::to_value(SchemasRegisterRequest {
        record: DataShapeRecord {
            tenant_id,
            namespace_id,
            schema_id: DataShapeId::new("schema-a"),
            version: DataShapeVersion::new("v1"),
            schema: json!({
                "type": "object",
                "properties": { "ok": { "type": "boolean" } },
                "required": ["ok"],
            }),
            description: Some("test schema".to_string()),
            created_at: Timestamp::Logical(1),
            signing: None,
        },
    })
    .expect("schemas register payload")
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
    let docs_provider_trait: Arc<dyn DocsProvider> = docs_provider;
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

#[test]
fn control_plane_store_overload_maps_to_rate_limited() {
    let error = ControlPlaneError::Store(StoreError::Overloaded {
        message: "sqlite queue full".to_string(),
        retry_after_ms: Some(5),
    });
    let mapped = map_control_plane_error(error);
    assert!(matches!(
        mapped,
        ToolError::RateLimited {
            message,
            retry_after_ms: Some(5)
        } if message == "sqlite queue full"
    ));
}

#[test]
fn registry_overload_maps_to_rate_limited() {
    let mapped: ToolError = DataShapeRegistryError::Overloaded {
        message: "registry queue full".to_string(),
        retry_after_ms: Some(3),
    }
    .into();
    assert!(matches!(
        mapped,
        ToolError::RateLimited {
            message,
            retry_after_ms: Some(3)
        } if message == "registry queue full"
    ));
}

// ============================================================================
// SECTION: Mutation Coordination + Overload Regression Tests
// ============================================================================

#[test]
fn scenario_start_distinct_runs_do_not_block_each_other() {
    let config = sample_config();
    let blocked_store = BlockingRunStateStore::new("run-a");
    let router = router_with_store_registry_overrides(
        config,
        None,
        None,
        None,
        None,
        Some(SharedRunStateStore::new(Arc::new(blocked_store.clone()))),
        None,
    );
    let spec = sample_spec();
    router
        .define_scenario(
            &RequestContext::stdio(),
            ScenarioDefineRequest {
                spec: spec.clone(),
            },
        )
        .expect("define scenario");
    let tenant_id = TenantId::from_raw(1).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let blocked_run_payload = scenario_start_payload(&spec, tenant_id, namespace_id, "run-a");
    let independent_run_payload = scenario_start_payload(&spec, tenant_id, namespace_id, "run-b");

    tokio::runtime::Runtime::new().expect("runtime").block_on(async move {
        let router_for_a = router.clone();
        let run_a_task = tokio::spawn(async move {
            let context = RequestContext::stdio();
            router_for_a
                .handle_tool_call(&context, ToolName::ScenarioStart.as_str(), blocked_run_payload)
                .await
        });
        assert!(
            blocked_store.wait_until_blocked(Duration::from_secs(1)),
            "run-a should block in store save",
        );

        let independent_run_outcome = tokio::time::timeout(Duration::from_millis(300), async {
            let context = RequestContext::stdio();
            router
                .handle_tool_call(
                    &context,
                    ToolName::ScenarioStart.as_str(),
                    independent_run_payload,
                )
                .await
        })
        .await;

        blocked_store.release_block();
        let blocked_run_outcome = tokio::time::timeout(Duration::from_secs(2), run_a_task)
            .await
            .expect("run-a should complete after release")
            .expect("run-a task join");

        let independent_run_response =
            independent_run_outcome.expect("run-b start timed out").expect("run-b start");
        assert_eq!(independent_run_response["run_id"], json!("run-b"));
        let blocked_run_response = blocked_run_outcome.expect("run-a start");
        assert_eq!(blocked_run_response["run_id"], json!("run-a"));
    });
}

#[test]
fn schemas_register_not_blocked_by_inflight_run_mutation() {
    let config = sample_config();
    let blocked_store = BlockingRunStateStore::new("run-a");
    let router = router_with_store_registry_overrides(
        config,
        None,
        None,
        None,
        None,
        Some(SharedRunStateStore::new(Arc::new(blocked_store.clone()))),
        None,
    );
    let spec = sample_spec();
    router
        .define_scenario(
            &RequestContext::stdio(),
            ScenarioDefineRequest {
                spec: spec.clone(),
            },
        )
        .expect("define scenario");
    let tenant_id = TenantId::from_raw(1).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let blocked_run_payload = scenario_start_payload(&spec, tenant_id, namespace_id, "run-a");
    let schema_payload = schemas_register_payload(tenant_id, namespace_id);

    tokio::runtime::Runtime::new().expect("runtime").block_on(async move {
        let router_for_run = router.clone();
        let run_a_task = tokio::spawn(async move {
            let context = RequestContext::stdio();
            router_for_run
                .handle_tool_call(&context, ToolName::ScenarioStart.as_str(), blocked_run_payload)
                .await
        });
        assert!(
            blocked_store.wait_until_blocked(Duration::from_secs(1)),
            "run-a should block in store save",
        );

        let schema_result = tokio::time::timeout(Duration::from_millis(300), async {
            let context = RequestContext::stdio();
            router
                .handle_tool_call(&context, ToolName::SchemasRegister.as_str(), schema_payload)
                .await
        })
        .await;

        blocked_store.release_block();
        let run_a_result = tokio::time::timeout(Duration::from_secs(2), run_a_task)
            .await
            .expect("run-a should complete after release")
            .expect("run-a task join");
        run_a_result.expect("run-a start");

        let schema_response = schema_result
            .expect("schemas_register timed out while run mutation was blocked")
            .expect("schemas_register response");
        assert_eq!(schema_response["record"]["schema_id"], json!("schema-a"));
    });
}

#[test]
fn run_mutation_coordinator_same_key_waits_and_updates_stats() {
    tokio::runtime::Runtime::new().expect("runtime").block_on(async move {
        let coordinator = Arc::new(RunMutationCoordinator::default());
        let key = RunMutationKey {
            tenant: TenantId::from_raw(1).expect("nonzero tenantid"),
            namespace: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            run: RunId::new("run-1"),
        };

        let permit = coordinator.acquire(&key).await;
        let waiter = {
            let coordinator = Arc::clone(&coordinator);
            let key = key.clone();
            tokio::spawn(async move {
                let _permit = coordinator.acquire(&key).await;
            })
        };

        let wait_started = Instant::now();
        loop {
            let snapshot = coordinator.snapshot();
            if snapshot.pending_waiters > 0 {
                assert_eq!(snapshot.active_holders, 1);
                break;
            }
            assert!(
                wait_started.elapsed() < Duration::from_millis(300),
                "waiter never observed as pending",
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        drop(permit);
        waiter.await.expect("waiter join");

        let final_stats = coordinator.snapshot();
        assert_eq!(final_stats.lock_acquisitions, 2);
        assert_eq!(final_stats.active_holders, 0);
        assert_eq!(final_stats.pending_waiters, 0);
        assert_eq!(final_stats.lock_wait_histogram.iter().sum::<u64>(), 2);
        assert_eq!(final_stats.queue_depth_histogram.iter().sum::<u64>(), 2);
    });
}

#[test]
fn run_mutation_key_from_all_request_types_is_consistent_for_same_run() {
    let scenario_id = ScenarioId::new("scenario-key");
    let run_id = RunId::new("run-key");
    let tenant_id = TenantId::from_raw(42).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(7).expect("nonzero namespaceid");
    let run_config = RunConfig {
        tenant_id,
        namespace_id,
        run_id: run_id.clone(),
        scenario_id: scenario_id.clone(),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    let start_request = ScenarioStartRequest {
        scenario_id: scenario_id.clone(),
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let status_request = ScenarioStatusRequest {
        scenario_id: scenario_id.clone(),
        request: StatusRequest {
            run_id: run_id.clone(),
            tenant_id,
            namespace_id,
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let next_request = ScenarioNextRequest {
        scenario_id: scenario_id.clone(),
        request: NextRequest {
            run_id: run_id.clone(),
            tenant_id,
            namespace_id,
            trigger_id: decision_gate_core::TriggerId::new("trigger-key"),
            agent_id: "agent-key".to_string(),
            time: Timestamp::Logical(3),
            correlation_id: None,
        },
        feedback: None,
    };
    let submit_request = ScenarioSubmitRequest {
        scenario_id: scenario_id.clone(),
        request: SubmitRequest {
            run_id: run_id.clone(),
            tenant_id,
            namespace_id,
            submission_id: "submission-key".to_string(),
            payload: PacketPayload::Json {
                value: json!({
                    "ok": true
                }),
            },
            content_type: "application/json".to_string(),
            submitted_at: Timestamp::Logical(4),
            correlation_id: None,
        },
    };
    let trigger_request = ScenarioTriggerRequest {
        scenario_id,
        trigger: TriggerEvent {
            trigger_id: decision_gate_core::TriggerId::new("trigger-key"),
            tenant_id,
            namespace_id,
            run_id: run_id.clone(),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(5),
            source_id: "test".to_string(),
            payload: None,
            correlation_id: None,
        },
    };

    let key_start = RunMutationKey::from_start(&start_request);
    let key_status = RunMutationKey::from_status(&status_request);
    let key_next = RunMutationKey::from_next(&next_request);
    let key_submit = RunMutationKey::from_submit(&submit_request);
    let key_trigger = RunMutationKey::from_trigger(&trigger_request);

    assert_eq!(key_start, key_status);
    assert_eq!(key_start, key_next);
    assert_eq!(key_start, key_submit);
    assert_eq!(key_start, key_trigger);
    assert_eq!(key_start.map_key(), key_status.map_key());
    assert_eq!(key_start.map_key(), key_next.map_key());
    assert_eq!(key_start.map_key(), key_submit.map_key());
    assert_eq!(key_start.map_key(), key_trigger.map_key());
}

#[test]
fn mutation_coordinator_snapshot_percentiles_empty_histograms_return_zero() {
    let coordinator = RunMutationCoordinator::default();
    let snapshot = coordinator.snapshot();
    assert_eq!(snapshot.lock_acquisitions, 0);
    assert_eq!(snapshot.active_holders, 0);
    assert_eq!(snapshot.pending_waiters, 0);
    assert_eq!(snapshot.lock_wait_p50_us, 0);
    assert_eq!(snapshot.lock_wait_p95_us, 0);
    assert_eq!(snapshot.queue_depth_p50, 0);
    assert_eq!(snapshot.queue_depth_p95, 0);
    assert!(snapshot.lock_wait_histogram.iter().all(|count| *count == 0));
    assert!(snapshot.queue_depth_histogram.iter().all(|count| *count == 0));
}

#[test]
fn histogram_percentile_rejects_out_of_range_percentiles() {
    let bounds = [10_u64, 20, 30];
    let counts = [1_u64, 2, 3, 0];
    assert_eq!(histogram_percentile(&bounds, &counts, 0), 0);
    assert_eq!(histogram_percentile(&bounds, &counts, 101), 0);
    assert_eq!(histogram_percentile(&bounds, &counts, 50), 20);
}

#[test]
fn scenario_start_overloaded_store_returns_rate_limited() {
    let config = sample_config();
    let store = SharedRunStateStore::new(Arc::new(OverloadedRunStateStore {
        message: "sqlite writer queue full".to_string(),
        retry_after_ms: Some(27),
    }));
    let router =
        router_with_store_registry_overrides(config, None, None, None, None, Some(store), None);
    let spec = sample_spec();
    router
        .define_scenario(
            &RequestContext::stdio(),
            ScenarioDefineRequest {
                spec: spec.clone(),
            },
        )
        .expect("define scenario");
    let payload = scenario_start_payload(
        &spec,
        TenantId::from_raw(1).expect("nonzero tenantid"),
        NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        "run-overloaded",
    );

    let err = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(async {
            let context = RequestContext::stdio();
            router.handle_tool_call(&context, ToolName::ScenarioStart.as_str(), payload).await
        })
        .expect_err("scenario_start should fail with rate-limited");
    assert!(matches!(
        err,
        ToolError::RateLimited {
            retry_after_ms: Some(27),
            ..
        }
    ));
}

#[test]
fn schemas_register_overloaded_registry_returns_rate_limited() {
    let config = sample_config();
    let registry = SharedDataShapeRegistry::new(Arc::new(OverloadedRegistry {
        message: "registry writer queue full".to_string(),
        retry_after_ms: Some(19),
    }));
    let router =
        router_with_store_registry_overrides(config, None, None, None, None, None, Some(registry));
    let payload = schemas_register_payload(
        TenantId::from_raw(1).expect("nonzero tenantid"),
        NamespaceId::from_raw(1).expect("nonzero namespaceid"),
    );

    let err = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(async {
            let context = RequestContext::stdio();
            router.handle_tool_call(&context, ToolName::SchemasRegister.as_str(), payload).await
        })
        .expect_err("schemas_register should fail with rate-limited");
    assert!(matches!(
        err,
        ToolError::RateLimited {
            retry_after_ms: Some(19),
            ..
        }
    ));
}
