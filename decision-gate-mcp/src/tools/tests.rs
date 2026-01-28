#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions favor direct unwrap/expect for clarity."
)]

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::DispatchTarget;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::PredicateSpec;
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
use ret_logic::Requirement;
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
use crate::config::TrustConfig;
use crate::config::ValidationConfig;
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

struct CountingObjectStore {
    objects: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl CountingObjectStore {
    fn new() -> Self {
        Self { objects: Mutex::new(std::collections::BTreeMap::new()) }
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
        Self { calls: Mutex::new(Vec::new()) }
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
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::new("tenant-1")],
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
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-time"),
                requirement: Requirement::predicate("after".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
            predicate: "after".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("time"),
                predicate: "after".to_string(),
                params: Some(json!({"timestamp": 0})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: Some(TenantId::new("tenant-1")),
    }
}

fn router_with_backends(
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
) -> ToolRouter {
    let mut config = sample_config();
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
        config.namespace.default_tenants.iter().map(ToString::to_string).collect::<BTreeSet<_>>();
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
        allow_default_namespace: config.allow_default_namespace(),
        default_namespace_tenants,
        namespace_authority: Arc::new(NoopNamespaceAuthority),
    })
}

fn setup_router_with_run(
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    object_store: Option<Arc<ObjectStoreRunpackBackend>>,
) -> (ToolRouter, ScenarioSpec, RunConfig) {
    let router = router_with_backends(runpack_storage, object_store);
    let spec = sample_spec();
    let context = RequestContext::stdio();
    router
        .define_scenario(&context, ScenarioDefineRequest { spec: spec.clone() })
        .expect("define scenario");
    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
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
        scenario_id: spec.scenario_id.clone(),
        tenant_id: run_config.tenant_id.clone(),
        namespace_id: run_config.namespace_id.clone(),
        run_id: run_config.run_id.clone(),
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
        scenario_id: spec.scenario_id.clone(),
        tenant_id: run_config.tenant_id.clone(),
        namespace_id: run_config.namespace_id.clone(),
        run_id: run_config.run_id.clone(),
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
        scenario_id: spec.scenario_id.clone(),
        tenant_id: run_config.tenant_id.clone(),
        namespace_id: run_config.namespace_id.clone(),
        run_id: run_config.run_id.clone(),
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
