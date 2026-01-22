// system-tests/tests/helpers/scenarios.rs
// ============================================================================
// Module: Scenario Fixtures
// Description: ScenarioSpec fixtures for system-tests.
// Purpose: Provide deterministic, reusable scenario definitions.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::PredicateKey;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SchemaId;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use serde_json::json;

/// Fixture bundle for a scenario and related IDs.
#[derive(Debug, Clone)]
pub struct ScenarioFixture {
    pub scenario_id: ScenarioId,
    pub run_id: RunId,
    pub tenant_id: TenantId,
    pub stage_id: StageId,
    pub spec: ScenarioSpec,
}

impl ScenarioFixture {
    /// Creates a simple time-based scenario with a single gate.
    pub fn time_after(scenario_id: &str, run_id: &str, threshold: u64) -> Self {
        let scenario_id = ScenarioId::new(scenario_id);
        let stage_id = StageId::new("stage-1");
        let predicate_key = PredicateKey::new("after");
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            spec_version: SpecVersion::new("1"),
            stages: vec![StageSpec {
                stage_id: stage_id.clone(),
                entry_packets: Vec::new(),
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-time"),
                    requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
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
            }],
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: TenantId::new("tenant-1"),
            stage_id,
            spec,
        }
    }

    /// Creates a scenario that emits an entry packet with visibility metadata.
    pub fn with_visibility_packet(
        scenario_id: &str,
        run_id: &str,
        visibility_labels: Vec<String>,
        policy_tags: Vec<String>,
    ) -> Self {
        let scenario_id = ScenarioId::new(scenario_id);
        let stage_id = StageId::new("stage-1");
        let predicate_key = PredicateKey::new("after");
        let packet = PacketSpec {
            packet_id: PacketId::new("packet-1"),
            schema_id: SchemaId::new("schema-1"),
            content_type: "application/json".to_string(),
            visibility_labels,
            policy_tags,
            expiry: None,
            payload: PacketPayload::Json {
                value: json!({"hello": "world"}),
            },
        };
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            spec_version: SpecVersion::new("1"),
            stages: vec![StageSpec {
                stage_id: stage_id.clone(),
                entry_packets: vec![packet],
                gates: vec![GateSpec {
                    gate_id: GateId::new("gate-time"),
                    requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
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
                    params: Some(json!({"timestamp": 0})),
                },
                comparator: Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
            }],
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: TenantId::new("tenant-1"),
            stage_id,
            spec,
        }
    }

    /// Returns a run config for the fixture.
    pub fn run_config(&self) -> RunConfig {
        RunConfig {
            tenant_id: self.tenant_id.clone(),
            run_id: self.run_id.clone(),
            scenario_id: self.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        }
    }

    /// Builds an evidence context for the fixture.
    pub fn evidence_context(&self, trigger_id: &str, time: Timestamp) -> EvidenceContext {
        EvidenceContext {
            tenant_id: self.tenant_id.clone(),
            run_id: self.run_id.clone(),
            scenario_id: self.scenario_id.clone(),
            stage_id: self.stage_id.clone(),
            trigger_id: TriggerId::new(trigger_id),
            trigger_time: time,
            correlation_id: None,
        }
    }

    /// Builds a trigger event for the fixture.
    pub fn trigger_event(&self, trigger_id: &str, time: Timestamp) -> TriggerEvent {
        TriggerEvent {
            trigger_id: TriggerId::new(trigger_id),
            run_id: self.run_id.clone(),
            kind: TriggerKind::ExternalEvent,
            time,
            source_id: "system-tests".to_string(),
            payload_ref: None,
            correlation_id: None,
        }
    }
}
