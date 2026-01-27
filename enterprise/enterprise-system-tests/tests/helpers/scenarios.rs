// enterprise-system-tests/tests/helpers/scenarios.rs
// ============================================================================
// Module: Scenario Fixtures (Enterprise)
// Description: ScenarioSpec fixtures for enterprise system-tests.
// Purpose: Provide deterministic, reusable scenario definitions.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PredicateKey;
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
use serde_json::json;

/// Fixture bundle for a scenario and related IDs.
#[derive(Debug, Clone)]
pub struct ScenarioFixture {
    pub scenario_id: ScenarioId,
    pub run_id: RunId,
    pub tenant_id: TenantId,
    pub namespace_id: NamespaceId,
    pub stage_id: StageId,
    pub spec: ScenarioSpec,
}

impl ScenarioFixture {
    /// Creates a simple time-based scenario with a single gate.
    pub fn time_after(scenario_id: &str, run_id: &str, threshold: u64) -> Self {
        let scenario_id = ScenarioId::new(scenario_id);
        let namespace_id = NamespaceId::new("default");
        let stage_id = StageId::new("stage-1");
        let predicate_key = PredicateKey::new("after");
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            namespace_id: namespace_id.clone(),
            spec_version: SpecVersion::new("1"),
            stages: vec![StageSpec {
                stage_id: stage_id.clone(),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-time"),
                    requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
                    trust: None,
                }],
                advance_to: AdvanceTo::Terminal,
                timeout: None,
                on_timeout: TimeoutPolicy::Fail,
            }],
            predicates: vec![PredicateSpec {
                predicate: predicate_key,
                query: EvidenceQuery {
                    provider_id: ProviderId::new("time"),
                    predicate: "after".to_string(),
                    params: Some(json!({"timestamp": threshold})),
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            }],
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: TenantId::new("tenant-1"),
            namespace_id,
            stage_id,
            spec,
        }
    }

    /// Returns a RunConfig for the fixture.
    pub fn run_config(&self) -> RunConfig {
        RunConfig {
            tenant_id: self.tenant_id.clone(),
            namespace_id: self.namespace_id.clone(),
            run_id: self.run_id.clone(),
            scenario_id: self.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        }
    }
}
