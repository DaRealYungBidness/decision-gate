// decision-gate-mcp/tests/tool_adapter.rs
// ============================================================================
// Module: MCP Tool Adapter Tests
// Description: Ensure MCP tool routing matches core control plane behavior.
// Purpose: Verify tools are thin wrappers over Decision Gate core methods.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================
//! ## Overview
//! Compares MCP tool responses against direct core invocations for identical inputs.

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
    reason = "Test-only output and panic-based assertions are permitted."
)]

use std::sync::Arc;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::NextResult;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::DefaultToolAuthz;
use decision_gate_mcp::FederatedEvidenceProvider;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::NoopAuditSink;
use decision_gate_mcp::RequestContext;
use decision_gate_mcp::SchemaRegistryConfig;
use decision_gate_mcp::ToolRouter;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::config::ValidationConfig;
use decision_gate_mcp::tools::ToolRouterConfig;
use serde_json::json;

struct NoopDispatcher;

impl Dispatcher for NoopDispatcher {
    fn dispatch(
        &self,
        _target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<DispatchReceipt, decision_gate_core::DispatchError> {
        Err(decision_gate_core::DispatchError::DispatchFailed(
            "dispatch should not be called".to_string(),
        ))
    }
}

struct PermitAll;

impl PolicyDecider for PermitAll {
    fn authorize(
        &self,
        _target: &DispatchTarget,
        _envelope: &decision_gate_core::PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<PolicyDecision, decision_gate_core::PolicyError> {
        Ok(PolicyDecision::Permit)
    }
}

fn sample_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::new("default"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-time"),
                requirement: ret_logic::Requirement::predicate("after".into()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
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
        default_tenant_id: None,
    }
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

/// Builds a tool router for MCP/core parity tests.
fn build_router(config: &DecisionGateConfig) -> ToolRouter {
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
                ProviderType::Builtin => decision_gate_mcp::tools::ProviderTransport::Builtin,
                ProviderType::Mcp => decision_gate_mcp::tools::ProviderTransport::Mcp,
            };
            (provider.name.clone(), transport)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let schema_registry_limits = decision_gate_mcp::tools::SchemaRegistryLimits {
        max_schema_bytes: config.schema_registry.max_schema_bytes,
        max_entries: config
            .schema_registry
            .max_entries
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
    };
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
        trust_requirement: config.effective_trust_requirement(),
        precheck_audit: Arc::new(McpNoopAuditSink),
        precheck_audit_payloads: config.server.audit.log_precheck_payloads,
        allow_default_namespace: config.allow_default_namespace(),
    })
}

/// Tests mcp tools match core control plane.
#[test]
fn mcp_tools_match_core_control_plane() {
    let config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: decision_gate_mcp::config::NamespaceConfig { allow_default: true },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: vec![builtin_provider("time")],
    };
    let evidence = FederatedEvidenceProvider::from_config(&config).unwrap();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    let define = decision_gate_mcp::tools::ScenarioDefineRequest { spec: sample_spec() };
    let _ = router
        .handle_tool_call(&context, "scenario_define", serde_json::to_value(&define).unwrap())
        .unwrap();

    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: decision_gate_core::RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    let start_request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: ScenarioId::new("scenario"),
        run_config: run_config.clone(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let _ = router
        .handle_tool_call(&context, "scenario_start", serde_json::to_value(&start_request).unwrap())
        .unwrap();

    let next_request = NextRequest {
        run_id: decision_gate_core::RunId::new("run-1"),
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        trigger_id: TriggerId::new("trigger-1"),
        agent_id: "agent-1".to_string(),
        time: Timestamp::Logical(2),
        correlation_id: None,
    };
    let tool_request = decision_gate_mcp::tools::ScenarioNextRequest {
        scenario_id: ScenarioId::new("scenario"),
        request: next_request.clone(),
    };
    let tool_result = router
        .handle_tool_call(&context, "scenario_next", serde_json::to_value(&tool_request).unwrap())
        .unwrap();
    let mcp_result: NextResult = serde_json::from_value(tool_result).unwrap();

    let store = InMemoryRunStateStore::new();
    let core = ControlPlane::new(
        sample_spec(),
        evidence,
        NoopDispatcher,
        store,
        Some(PermitAll),
        ControlPlaneConfig::default(),
    )
    .unwrap();
    core.start_run(run_config, Timestamp::Logical(1), false).unwrap();
    let core_result = core.scenario_next(&next_request).unwrap();

    assert_eq!(mcp_result.decision, core_result.decision);
    assert_eq!(mcp_result.packets, core_result.packets);
}

/// Builds a default config for parity tests.
fn default_config() -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: decision_gate_mcp::config::NamespaceConfig { allow_default: true },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: vec![builtin_provider("time")],
    }
}

/// Tests scenario_status tool returns matching status.
#[test]
fn parity_scenario_status() {
    use decision_gate_core::runtime::ScenarioStatus;
    use decision_gate_core::runtime::StatusRequest;
    use decision_gate_mcp::tools::ScenarioStatusRequest;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    // Define and start a scenario
    let define = decision_gate_mcp::tools::ScenarioDefineRequest { spec: sample_spec() };
    router
        .handle_tool_call(&context, "scenario_define", serde_json::to_value(&define).unwrap())
        .unwrap();

    let run_config = RunConfig {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: decision_gate_core::RunId::new("run-status"),
        scenario_id: ScenarioId::new("scenario"),
        dispatch_targets: Vec::new(),
        policy_tags: Vec::new(),
    };
    let start_request = decision_gate_mcp::tools::ScenarioStartRequest {
        scenario_id: ScenarioId::new("scenario"),
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    router
        .handle_tool_call(&context, "scenario_start", serde_json::to_value(&start_request).unwrap())
        .unwrap();

    // Query status via MCP
    let status_request = ScenarioStatusRequest {
        scenario_id: ScenarioId::new("scenario"),
        request: StatusRequest {
            run_id: decision_gate_core::RunId::new("run-status"),
            tenant_id: TenantId::new("tenant"),
            namespace_id: NamespaceId::new("default"),
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let mcp_result = router
        .handle_tool_call(
            &context,
            "scenario_status",
            serde_json::to_value(&status_request).unwrap(),
        )
        .unwrap();
    let mcp_status: ScenarioStatus = serde_json::from_value(mcp_result).unwrap();

    // Verify status is returned (active run should have a stage ID)
    assert!(
        !mcp_status.current_stage_id.as_str().is_empty(),
        "status should have current stage id"
    );
}

/// Tests providers_list tool returns configured providers.
#[test]
fn parity_providers_list() {
    use decision_gate_mcp::tools::ProvidersListRequest;
    use decision_gate_mcp::tools::ProvidersListResponse;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    let request = ProvidersListRequest {};
    let mcp_result = router
        .handle_tool_call(&context, "providers_list", serde_json::to_value(&request).unwrap())
        .unwrap();
    let response: ProvidersListResponse = serde_json::from_value(mcp_result).unwrap();

    // Should include the "time" provider from config
    let provider_ids: Vec<_> = response.providers.iter().map(|p| p.provider_id.as_str()).collect();
    assert!(provider_ids.contains(&"time"), "providers should include 'time': {provider_ids:?}");
}

/// Tests scenarios_list tool returns defined scenarios.
#[test]
fn parity_scenarios_list() {
    use decision_gate_mcp::tools::ScenariosListRequest;
    use decision_gate_mcp::tools::ScenariosListResponse;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    // Define a scenario
    let define = decision_gate_mcp::tools::ScenarioDefineRequest { spec: sample_spec() };
    router
        .handle_tool_call(&context, "scenario_define", serde_json::to_value(&define).unwrap())
        .unwrap();

    // List scenarios
    let request = ScenariosListRequest {
        tenant_id: TenantId::new("any"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let mcp_result = router
        .handle_tool_call(&context, "scenarios_list", serde_json::to_value(&request).unwrap())
        .unwrap();
    let response: ScenariosListResponse = serde_json::from_value(mcp_result).unwrap();

    // Should include the defined scenario
    let scenario_ids: Vec<_> = response.items.iter().map(|s| s.scenario_id.as_str()).collect();
    assert!(
        scenario_ids.contains(&"scenario"),
        "scenarios should include 'scenario': {scenario_ids:?}"
    );
}

/// Tests schemas_register/get roundtrip.
#[test]
fn parity_schemas_register_get() {
    use decision_gate_core::DataShapeId;
    use decision_gate_core::DataShapeRecord;
    use decision_gate_core::DataShapeVersion;
    use decision_gate_mcp::tools::SchemasGetRequest;
    use decision_gate_mcp::tools::SchemasGetResponse;
    use decision_gate_mcp::tools::SchemasRegisterRequest;
    use decision_gate_mcp::tools::SchemasRegisterResponse;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    let schema = json!({"type": "object", "properties": {"value": {"type": "integer"}}});
    let record = DataShapeRecord {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("test-schema"),
        version: DataShapeVersion::new("1"),
        schema: schema.clone(),
        description: None,
        created_at: Timestamp::Logical(1),
    };

    // Register schema
    let register_request = SchemasRegisterRequest { record: record.clone() };
    let register_result = router
        .handle_tool_call(
            &context,
            "schemas_register",
            serde_json::to_value(&register_request).unwrap(),
        )
        .unwrap();
    let register_response: SchemasRegisterResponse =
        serde_json::from_value(register_result).unwrap();
    assert_eq!(register_response.record.schema_id, record.schema_id);

    // Get schema back
    let get_request = SchemasGetRequest {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("test-schema"),
        version: DataShapeVersion::new("1"),
    };
    let get_result = router
        .handle_tool_call(&context, "schemas_get", serde_json::to_value(&get_request).unwrap())
        .unwrap();
    let get_response: SchemasGetResponse = serde_json::from_value(get_result).unwrap();
    assert_eq!(get_response.record.schema, schema);
}

/// Tests schemas_list returns registered schemas.
#[test]
fn parity_schemas_list() {
    use decision_gate_core::DataShapeId;
    use decision_gate_core::DataShapeRecord;
    use decision_gate_core::DataShapeVersion;
    use decision_gate_mcp::tools::SchemasListRequest;
    use decision_gate_mcp::tools::SchemasListResponse;
    use decision_gate_mcp::tools::SchemasRegisterRequest;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    let schema = json!({"type": "string"});
    let record = DataShapeRecord {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("list-schema"),
        version: DataShapeVersion::new("1"),
        schema,
        description: None,
        created_at: Timestamp::Logical(1),
    };

    // Register schema
    let register_request = SchemasRegisterRequest { record };
    router
        .handle_tool_call(
            &context,
            "schemas_register",
            serde_json::to_value(&register_request).unwrap(),
        )
        .unwrap();

    // List schemas
    let list_request = SchemasListRequest {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let list_result = router
        .handle_tool_call(&context, "schemas_list", serde_json::to_value(&list_request).unwrap())
        .unwrap();
    let list_response: SchemasListResponse = serde_json::from_value(list_result).unwrap();

    let schema_ids: Vec<_> = list_response.items.iter().map(|s| s.schema_id.as_str()).collect();
    assert!(
        schema_ids.contains(&"list-schema"),
        "schemas should include 'list-schema': {schema_ids:?}"
    );
}

/// Tests evidence_query tool returns evidence from provider.
#[test]
fn parity_evidence_query() {
    use decision_gate_core::EvidenceContext;
    use decision_gate_core::EvidenceQuery;
    use decision_gate_core::RunId;
    use decision_gate_mcp::tools::EvidenceQueryRequest;
    use decision_gate_mcp::tools::EvidenceQueryResponse;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    // Query time provider
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        predicate: "after".to_string(),
        params: Some(json!({"timestamp": 0})),
    };
    let evidence_context = EvidenceContext {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run"),
        scenario_id: ScenarioId::new("scenario"),
        stage_id: StageId::new("stage"),
        trigger_id: TriggerId::new("trigger"),
        trigger_time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let request = EvidenceQueryRequest { query, context: evidence_context };
    let mcp_result = router
        .handle_tool_call(&context, "evidence_query", serde_json::to_value(&request).unwrap())
        .unwrap();
    let response: EvidenceQueryResponse = serde_json::from_value(mcp_result).unwrap();

    // Time provider should return a boolean result
    assert!(
        response.result.value.is_some() || response.result.evidence_hash.is_some(),
        "evidence query should return value or hash"
    );
}

/// Tests precheck tool evaluates predicates and returns a decision.
#[test]
fn parity_precheck() {
    use decision_gate_core::DataShapeId;
    use decision_gate_core::DataShapeRecord;
    use decision_gate_core::DataShapeRef;
    use decision_gate_core::DataShapeVersion;
    use decision_gate_mcp::tools::PrecheckToolRequest;
    use decision_gate_mcp::tools::PrecheckToolResponse;
    use decision_gate_mcp::tools::SchemasRegisterRequest;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    // Create a spec
    let spec = sample_spec();

    // Define the scenario
    let define = decision_gate_mcp::tools::ScenarioDefineRequest { spec: spec.clone() };
    router
        .handle_tool_call(&context, "scenario_define", serde_json::to_value(&define).unwrap())
        .unwrap();

    // Register a schema for the data shape
    let schema = json!({"type": "object", "properties": {"after": {"type": "boolean"}}});
    let record = DataShapeRecord {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("precheck-shape"),
        version: DataShapeVersion::new("1"),
        schema,
        description: None,
        created_at: Timestamp::Logical(1),
    };
    let register_request = SchemasRegisterRequest { record };
    router
        .handle_tool_call(
            &context,
            "schemas_register",
            serde_json::to_value(&register_request).unwrap(),
        )
        .unwrap();

    // Precheck with asserted evidence
    let precheck_request = PrecheckToolRequest {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        scenario_id: Some(ScenarioId::new("scenario")),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: DataShapeId::new("precheck-shape"),
            version: DataShapeVersion::new("1"),
        },
        payload: json!({"after": true}),
    };
    let mcp_result = router
        .handle_tool_call(&context, "precheck", serde_json::to_value(&precheck_request).unwrap())
        .unwrap();
    let response: PrecheckToolResponse = serde_json::from_value(mcp_result).unwrap();

    // Precheck should return a decision (even if Hold) and gate evaluations
    // The important thing is that precheck executes without error and returns structured data
    assert!(!response.gate_evaluations.is_empty(), "precheck should return gate evaluations");
}

/// Tests scenario_status fails for non-existent run.
#[test]
fn parity_scenario_status_not_found() {
    use decision_gate_core::runtime::StatusRequest;
    use decision_gate_mcp::tools::ScenarioStatusRequest;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    // Define scenario but don't start a run
    let define = decision_gate_mcp::tools::ScenarioDefineRequest { spec: sample_spec() };
    router
        .handle_tool_call(&context, "scenario_define", serde_json::to_value(&define).unwrap())
        .unwrap();

    // Query status for non-existent run
    let status_request = ScenarioStatusRequest {
        scenario_id: ScenarioId::new("scenario"),
        request: StatusRequest {
            run_id: decision_gate_core::RunId::new("non-existent"),
            tenant_id: TenantId::new("tenant"),
            namespace_id: NamespaceId::new("default"),
            requested_at: Timestamp::Logical(1),
            correlation_id: None,
        },
    };
    let result = router.handle_tool_call(
        &context,
        "scenario_status",
        serde_json::to_value(&status_request).unwrap(),
    );

    assert!(result.is_err(), "status for non-existent run should fail");
}

/// Tests evidence_query fails for unknown provider.
#[test]
fn parity_evidence_query_unknown_provider() {
    use decision_gate_core::EvidenceContext;
    use decision_gate_core::EvidenceQuery;
    use decision_gate_core::RunId;
    use decision_gate_mcp::tools::EvidenceQueryRequest;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    // Query non-existent provider
    let query = EvidenceQuery {
        provider_id: ProviderId::new("unknown-provider"),
        predicate: "test".to_string(),
        params: None,
    };
    let evidence_context = EvidenceContext {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run"),
        scenario_id: ScenarioId::new("scenario"),
        stage_id: StageId::new("stage"),
        trigger_id: TriggerId::new("trigger"),
        trigger_time: Timestamp::Logical(1),
        correlation_id: None,
    };

    let request = EvidenceQueryRequest { query, context: evidence_context };
    let result = router.handle_tool_call(
        &context,
        "evidence_query",
        serde_json::to_value(&request).unwrap(),
    );

    assert!(result.is_err(), "evidence query for unknown provider should fail");
}

/// Tests schemas_get fails for non-existent schema.
#[test]
fn parity_schemas_get_not_found() {
    use decision_gate_core::DataShapeId;
    use decision_gate_core::DataShapeVersion;
    use decision_gate_mcp::tools::SchemasGetRequest;

    let config = default_config();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    let get_request = SchemasGetRequest {
        tenant_id: TenantId::new("tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("non-existent-schema"),
        version: DataShapeVersion::new("1"),
    };
    let result = router.handle_tool_call(
        &context,
        "schemas_get",
        serde_json::to_value(&get_request).unwrap(),
    );

    assert!(result.is_err(), "schemas_get for non-existent should fail");
}
