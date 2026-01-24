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
use decision_gate_core::TrustRequirement;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::NextResult;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::DefaultToolAuthz;
use decision_gate_mcp::FederatedEvidenceProvider;
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
        dispatch_policy: config.policy.dispatch.clone(),
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

/// Tests mcp tools match core control plane.
#[test]
fn mcp_tools_match_core_control_plane() {
    let config = DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: vec![builtin_provider("time")],
    };
    let evidence = FederatedEvidenceProvider::from_config(&config).unwrap();
    let router = build_router(&config);
    let context = RequestContext::stdio();

    let define = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec: sample_spec(),
    };
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
