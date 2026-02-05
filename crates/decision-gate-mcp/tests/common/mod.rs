// crates/decision-gate-mcp/tests/common/mod.rs
// ============================================================================
// Module: Common Test Fixtures
// Description: Shared test utilities and fixtures for MCP tests.
// Purpose: Provide reusable test infrastructure for deterministic testing.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! This module provides shared test fixtures, helper functions, and sample
//! specifications for use across all MCP test files.
//!
//! Security posture: Test fixtures are designed to exercise trust boundaries
//! and validate fail-closed behavior under adversarial conditions.

#![allow(dead_code, reason = "Shared test helpers may be unused in some cases.")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unwrap_in_result,
    reason = "Test fixtures favor direct unwraps for setup clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::FederatedEvidenceProvider;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::NoopTenantAuthorizer;
use decision_gate_mcp::NoopUsageMeter;
use decision_gate_mcp::RunpackStorage;
use decision_gate_mcp::SchemaRegistryConfig;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::ToolRouter;
use decision_gate_mcp::UsageMeter;
use decision_gate_mcp::auth::DefaultToolAuthz;
use decision_gate_mcp::auth::NoopAuditSink;
use decision_gate_mcp::auth::RequestContext;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::AnchorPolicyConfig;
use decision_gate_mcp::config::DocsConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::NamespaceConfig;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::ServerToolsConfig;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::config::ValidationConfig;
use decision_gate_mcp::docs::DocsCatalog;
use decision_gate_mcp::namespace_authority::NoopNamespaceAuthority;
use decision_gate_mcp::registry_acl::PrincipalResolver;
use decision_gate_mcp::registry_acl::RegistryAcl;
use decision_gate_mcp::tools::ProviderTransport;
use decision_gate_mcp::tools::SchemaRegistryLimits;
use decision_gate_mcp::tools::ToolDefinition;
use decision_gate_mcp::tools::ToolError;
use decision_gate_mcp::tools::ToolRouterConfig;
use serde_json::Value;
use serde_json::json;
use toml::Value as TomlValue;
use toml::value::Table;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

/// Creates a default Decision Gate config for testing.
#[must_use]
pub fn sample_config() -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig {
            auth: Some(ServerAuthConfig {
                mode: ServerAuthMode::LocalOnly,
                bearer_tokens: Vec::new(),
                mtls_subjects: Vec::new(),
                allowed_tools: Vec::new(),
                principals: vec![
                    PrincipalConfig {
                        subject: "stdio".to_string(),
                        policy_class: Some("prod".to_string()),
                        roles: vec![PrincipalRoleConfig {
                            name: "TenantAdmin".to_string(),
                            tenant_id: None,
                            namespace_id: None,
                        }],
                    },
                    PrincipalConfig {
                        subject: "loopback".to_string(),
                        policy_class: Some("prod".to_string()),
                        roles: vec![PrincipalRoleConfig {
                            name: "TenantAdmin".to_string(),
                            tenant_id: None,
                            namespace_id: None,
                        }],
                    },
                ],
            }),
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![
                TenantId::from_raw(100).expect("nonzero tenantid"),
                TenantId::from_raw(1).expect("nonzero tenantid"),
                TenantId::from_raw(1).expect("nonzero tenantid"),
                TenantId::from_raw(2).expect("nonzero tenantid"),
                TenantId::from_raw(1).expect("nonzero tenantid"),
            ],
            ..NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: builtin_providers(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    }
}

/// Creates a federated evidence provider from the sample config.
#[must_use]
pub fn sample_evidence() -> FederatedEvidenceProvider {
    FederatedEvidenceProvider::from_config(&sample_config()).unwrap()
}

/// Creates a tool router using sample configuration.
#[must_use]
pub fn sample_router() -> ToolRouter {
    let config = sample_config();
    router_with_config(&config)
}

/// Creates a tool router using a custom configuration.
#[must_use]
pub fn router_with_config(config: &DecisionGateConfig) -> ToolRouter {
    router_with_authorizer(config, Arc::new(NoopTenantAuthorizer))
}

/// Creates a tool router using a custom tenant authorizer.
#[must_use]
pub fn router_with_authorizer(
    config: &DecisionGateConfig,
    tenant_authorizer: Arc<dyn TenantAuthorizer>,
) -> ToolRouter {
    router_with_authorizer_and_usage(config, tenant_authorizer, Arc::new(NoopUsageMeter))
}

/// Creates a tool router using custom tenant authorizer and usage meter.
#[must_use]
pub fn router_with_authorizer_and_usage(
    config: &DecisionGateConfig,
    tenant_authorizer: Arc<dyn TenantAuthorizer>,
    usage_meter: Arc<dyn UsageMeter>,
) -> ToolRouter {
    router_with_authorizer_usage_and_runpack_storage(config, tenant_authorizer, usage_meter, None)
}

/// Creates a tool router using custom auth, usage, and runpack storage.
#[must_use]
pub fn router_with_authorizer_usage_and_runpack_storage(
    config: &DecisionGateConfig,
    tenant_authorizer: Arc<dyn TenantAuthorizer>,
    usage_meter: Arc<dyn UsageMeter>,
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
) -> ToolRouter {
    let evidence = FederatedEvidenceProvider::from_config(config).unwrap();
    let capabilities = CapabilityRegistry::from_config(config).unwrap();
    let store = decision_gate_core::SharedRunStateStore::from_store(
        decision_gate_core::InMemoryRunStateStore::new(),
    );
    let schema_registry = decision_gate_core::SharedDataShapeRegistry::from_registry(
        decision_gate_core::InMemoryDataShapeRegistry::new(),
    );
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
    let trust_requirement = config.effective_trust_requirement();
    let allow_default_namespace = config.allow_default_namespace();
    let default_namespace_tenants =
        config.namespace.default_tenants.iter().copied().collect::<BTreeSet<_>>();
    let evidence_policy = config.evidence.clone();
    let validation = config.validation.clone();
    let anchor_policy = config.anchors.to_policy();
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
        BTreeMap::new()
    };
    let runpack_security_context = Some(decision_gate_core::RunpackSecurityContext {
        dev_permissive: config.is_dev_permissive(),
        namespace_authority: "dg_registry".to_string(),
    });
    let precheck_audit_payloads = config.server.audit.log_precheck_payloads;
    let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
    let principal_resolver = PrincipalResolver::from_config(config.server.auth.as_ref());
    let registry_acl = RegistryAcl::new(&config.schema_registry.acl);
    let audit = Arc::new(NoopAuditSink);
    let dispatch_policy = config.policy.dispatch_policy().expect("dispatch policy");
    let docs_catalog = DocsCatalog::from_config(&config.docs).expect("docs catalog");
    ToolRouter::new(ToolRouterConfig {
        evidence,
        evidence_policy,
        validation,
        dispatch_policy,
        store,
        schema_registry,
        provider_transports,
        schema_registry_limits,
        capabilities: Arc::new(capabilities),
        provider_discovery: config.provider_discovery.clone(),
        authz,
        tenant_authorizer,
        usage_meter,
        runpack_storage,
        runpack_object_store: None,
        audit,
        trust_requirement,
        anchor_policy,
        provider_trust_overrides,
        runpack_security_context,
        precheck_audit: Arc::new(McpNoopAuditSink),
        precheck_audit_payloads,
        registry_acl,
        principal_resolver,
        scenario_next_feedback: config.server.feedback.scenario_next.clone(),
        docs_config: config.docs.clone(),
        docs_catalog,
        tools: config.server.tools.clone(),
        docs_provider: None,
        tool_visibility_resolver: None,
        allow_default_namespace,
        default_namespace_tenants,
        namespace_authority: Arc::new(NoopNamespaceAuthority),
    })
}

fn builtin_providers() -> Vec<ProviderConfig> {
    vec![
        builtin_provider("time"),
        builtin_provider("env"),
        json_provider(),
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

fn json_root_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("json_root")
}

fn json_provider() -> ProviderConfig {
    let mut provider = builtin_provider("json");
    provider.config = Some(json_provider_config());
    provider
}

fn json_provider_config() -> TomlValue {
    let mut table = Table::new();
    table.insert(
        "root".to_string(),
        TomlValue::String(json_root_dir().to_string_lossy().to_string()),
    );
    table.insert("root_id".to_string(), TomlValue::String("mcp-tests-root".to_string()));
    table.insert("allow_yaml".to_string(), TomlValue::Boolean(true));
    table.insert("max_bytes".to_string(), TomlValue::Integer(1_048_576));
    TomlValue::Table(table)
}

/// Creates a minimal scenario spec for testing.
#[must_use]
pub fn sample_spec() -> ScenarioSpec {
    sample_spec_with_id("test-scenario")
}

/// Creates a scenario spec with a specified ID.
#[must_use]
pub fn sample_spec_with_id(id: &str) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new(id),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-time"),
                requirement: ret_logic::Requirement::condition("after".into()),
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
        default_tenant_id: Some(TenantId::from_raw(100).expect("nonzero tenantid")),
    }
}

/// Creates a scenario spec with two conditions in a single gate.
#[must_use]
pub fn sample_spec_with_two_conditions(id: &str) -> ScenarioSpec {
    let mut spec = sample_spec_with_id(id);
    spec.conditions.push(ConditionSpec {
        condition_id: "after_alt".into(),
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            check_id: "after".to_string(),
            params: Some(json!({"timestamp": 0})),
        },
        comparator: Comparator::Equals,
        expected: Some(json!(true)),
        policy_tags: Vec::new(),
        trust: None,
    });
    spec.stages[0].gates[0].requirement = ret_logic::Requirement::and(vec![
        ret_logic::Requirement::condition("after".into()),
        ret_logic::Requirement::condition("after_alt".into()),
    ]);
    spec
}

/// Creates a run configuration for testing.
#[must_use]
pub fn sample_run_config() -> RunConfig {
    sample_run_config_with_ids(1, "test-run", "test-scenario")
}

/// Creates a run configuration with specified IDs.
#[must_use]
pub fn sample_run_config_with_ids(tenant_id: u64, run_id: &str, scenario_id: &str) -> RunConfig {
    RunConfig {
        tenant_id: TenantId::from_raw(tenant_id).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new(run_id),
        scenario_id: ScenarioId::new(scenario_id),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    }
}

/// Creates an evidence context for testing.
#[must_use]
pub fn sample_context() -> EvidenceContext {
    sample_context_with_time(Timestamp::Logical(1))
}

/// Creates an evidence context with a specific trigger time.
#[must_use]
pub fn sample_context_with_time(trigger_time: Timestamp) -> EvidenceContext {
    EvidenceContext {
        tenant_id: TenantId::from_raw(100).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("test-run"),
        scenario_id: ScenarioId::new("test-scenario"),
        stage_id: StageId::new("test-stage"),
        trigger_id: TriggerId::new("test-trigger"),
        trigger_time,
        correlation_id: None,
    }
}

/// Returns a local-only request context for tool calls.
#[must_use]
pub fn local_request_context() -> RequestContext {
    RequestContext::stdio().with_server_correlation_id("test-server")
}

// ============================================================================
// SECTION: Test Helper Functions
// ============================================================================

pub trait ToolRouterSyncExt {
    fn handle_tool_call_sync(
        &self,
        context: &RequestContext,
        name: &str,
        payload: Value,
    ) -> Result<Value, ToolError>;

    fn list_tools_sync(&self, context: &RequestContext) -> Result<Vec<ToolDefinition>, ToolError>;
}

impl ToolRouterSyncExt for ToolRouter {
    fn handle_tool_call_sync(
        &self,
        context: &RequestContext,
        name: &str,
        payload: Value,
    ) -> Result<Value, ToolError> {
        tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(self.handle_tool_call(context, name, payload))
    }

    fn list_tools_sync(&self, context: &RequestContext) -> Result<Vec<ToolDefinition>, ToolError> {
        tokio::runtime::Runtime::new().expect("runtime").block_on(self.list_tools(context))
    }
}

/// Defines a scenario using the tool router.
///
/// Returns the scenario ID on success.
pub fn define_scenario(router: &ToolRouter, spec: ScenarioSpec) -> Result<ScenarioId, String> {
    let request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec,
    };
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.handle_tool_call(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        ))
        .map_err(|e| e.to_string())?;
    let response: decision_gate_mcp::tools::ScenarioDefineResponse =
        serde_json::from_value(result).map_err(|e| e.to_string())?;
    Ok(response.scenario_id)
}

/// Starts a scenario run using the tool router.
pub fn start_run(
    router: &ToolRouter,
    scenario_id: &ScenarioId,
    run_config: RunConfig,
    started_at: Timestamp,
) -> Result<decision_gate_core::RunState, String> {
    let request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: scenario_id.clone(),
        run_config,
        started_at,
        issue_entry_packets: false,
    };
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.handle_tool_call(
            &local_request_context(),
            "scenario_start",
            serde_json::to_value(&request).unwrap(),
        ))
        .map_err(|e| e.to_string())?;
    let response: decision_gate_core::RunState =
        serde_json::from_value(result).map_err(|e| e.to_string())?;
    Ok(response)
}

/// Sets up a scenario with a run and returns the router for further testing.
pub fn setup_scenario_with_run() -> (ToolRouter, ScenarioId, RunId) {
    let router = sample_router();
    let spec = sample_spec();
    let scenario_id = define_scenario(&router, spec).unwrap();
    let run_config = sample_run_config_with_ids(1, "test-run", scenario_id.as_str());
    start_run(&router, &scenario_id, run_config, Timestamp::Logical(1)).unwrap();
    (router, scenario_id, RunId::new("test-run"))
}
