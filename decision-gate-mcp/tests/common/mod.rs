// decision-gate-mcp/tests/common/mod.rs
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

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::sync::Arc;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::PredicateSpec;
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
use decision_gate_mcp::ToolRouter;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::TrustConfig;
use serde_json::json;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

/// Creates a default Decision Gate config for testing.
#[must_use]
pub fn sample_config() -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        providers: builtin_providers(),
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
    let evidence = FederatedEvidenceProvider::from_config(&config).unwrap();
    let capabilities = CapabilityRegistry::from_config(&config).unwrap();
    ToolRouter::new(evidence, config.evidence, Arc::new(capabilities))
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
        config: None,
    }
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
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-time"),
                requirement: ret_logic::Requirement::predicate("after".into()),
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
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

/// Creates a run configuration for testing.
#[must_use]
pub fn sample_run_config() -> RunConfig {
    sample_run_config_with_ids("test-tenant", "test-run", "test-scenario")
}

/// Creates a run configuration with specified IDs.
#[must_use]
pub fn sample_run_config_with_ids(tenant_id: &str, run_id: &str, scenario_id: &str) -> RunConfig {
    RunConfig {
        tenant_id: TenantId::new(tenant_id),
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
        tenant_id: TenantId::new("test-tenant"),
        run_id: RunId::new("test-run"),
        scenario_id: ScenarioId::new("test-scenario"),
        stage_id: StageId::new("test-stage"),
        trigger_id: TriggerId::new("test-trigger"),
        trigger_time,
        correlation_id: None,
    }
}

// ============================================================================
// SECTION: Test Helper Functions
// ============================================================================

/// Defines a scenario using the tool router.
///
/// Returns the scenario ID on success.
pub fn define_scenario(router: &ToolRouter, spec: ScenarioSpec) -> Result<ScenarioId, String> {
    let request = decision_gate_mcp::tools::ScenarioDefineRequest {
        spec,
    };
    let result = router
        .handle_tool_call("scenario_define", serde_json::to_value(&request).unwrap())
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
    let result = router
        .handle_tool_call("scenario_start", serde_json::to_value(&request).unwrap())
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
    let run_config = sample_run_config_with_ids("test-tenant", "test-run", scenario_id.as_str());
    start_run(&router, &scenario_id, run_config, Timestamp::Logical(1)).unwrap();
    (router, scenario_id, RunId::new("test-run"))
}
