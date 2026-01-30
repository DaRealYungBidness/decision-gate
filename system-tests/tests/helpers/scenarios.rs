// system-tests/tests/helpers/scenarios.rs
// ============================================================================
// Module: Scenario Fixtures
// Description: ScenarioSpec fixtures for system-tests.
// Purpose: Provide deterministic, reusable scenario definitions.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================

use std::num::NonZeroU64;

use decision_gate_core::AdvanceTo;
use decision_gate_core::BranchRule;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateOutcome;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
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
use decision_gate_core::TimeoutSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use serde_json::json;

const fn default_tenant_id() -> TenantId {
    TenantId::new(NonZeroU64::MIN)
}

const fn default_namespace_id() -> NamespaceId {
    NamespaceId::new(NonZeroU64::MIN)
}

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
        let namespace_id = default_namespace_id();
        let stage_id = StageId::new("stage-1");
        let predicate_key = PredicateKey::new("after");
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            namespace_id,
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
            tenant_id: default_tenant_id(),
            namespace_id,
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
        let namespace_id = default_namespace_id();
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
            namespace_id,
            spec_version: SpecVersion::new("1"),
            stages: vec![StageSpec {
                stage_id: stage_id.clone(),
                entry_packets: vec![packet],
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
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: default_tenant_id(),
            namespace_id,
            stage_id,
            spec,
        }
    }

    /// Creates a single-stage scenario that fails on timeout.
    pub fn timeout_fail(scenario_id: &str, run_id: &str, timeout_ms: u64) -> Self {
        let scenario_id = ScenarioId::new(scenario_id);
        let namespace_id = default_namespace_id();
        let stage_id = StageId::new("stage-1");
        let predicate_key = PredicateKey::new("after");
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            namespace_id,
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
                timeout: Some(TimeoutSpec {
                    timeout_ms,
                    policy_tags: Vec::new(),
                }),
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
                trust: None,
            }],
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: default_tenant_id(),
            namespace_id,
            stage_id,
            spec,
        }
    }

    /// Creates a two-stage scenario that advances on timeout with a timeout flag.
    pub fn timeout_advance(scenario_id: &str, run_id: &str, timeout_ms: u64) -> Self {
        let scenario_id = ScenarioId::new(scenario_id);
        let namespace_id = default_namespace_id();
        let stage1_id = StageId::new("stage-1");
        let stage2_id = StageId::new("stage-2");
        let predicate_key = PredicateKey::new("after");
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            namespace_id,
            spec_version: SpecVersion::new("1"),
            stages: vec![
                StageSpec {
                    stage_id: stage1_id.clone(),
                    entry_packets: Vec::new(),
                    gates: vec![GateSpec {
                        gate_id: GateId::new("gate-time"),
                        requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
                        trust: None,
                    }],
                    advance_to: AdvanceTo::Linear,
                    timeout: Some(TimeoutSpec {
                        timeout_ms,
                        policy_tags: Vec::new(),
                    }),
                    on_timeout: TimeoutPolicy::AdvanceWithFlag,
                },
                StageSpec {
                    stage_id: stage2_id,
                    entry_packets: Vec::new(),
                    gates: Vec::new(),
                    advance_to: AdvanceTo::Terminal,
                    timeout: None,
                    on_timeout: TimeoutPolicy::Fail,
                },
            ],
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
                trust: None,
            }],
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: default_tenant_id(),
            namespace_id,
            stage_id: stage1_id,
            spec,
        }
    }

    /// Creates a scenario that routes to an alternate branch on timeout.
    pub fn timeout_alternate_branch(scenario_id: &str, run_id: &str, timeout_ms: u64) -> Self {
        let scenario_id = ScenarioId::new(scenario_id);
        let namespace_id = default_namespace_id();
        let stage1_id = StageId::new("stage-1");
        let stage2_id = StageId::new("stage-alt");
        let predicate_key = PredicateKey::new("after");
        let spec = ScenarioSpec {
            scenario_id: scenario_id.clone(),
            namespace_id,
            spec_version: SpecVersion::new("1"),
            stages: vec![
                StageSpec {
                    stage_id: stage1_id.clone(),
                    entry_packets: Vec::new(),
                    gates: vec![GateSpec {
                        gate_id: GateId::new("gate-time"),
                        requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
                        trust: None,
                    }],
                    advance_to: AdvanceTo::Branch {
                        branches: vec![BranchRule {
                            gate_id: GateId::new("gate-time"),
                            outcome: GateOutcome::Unknown,
                            next_stage_id: stage2_id.clone(),
                        }],
                        default: None,
                    },
                    timeout: Some(TimeoutSpec {
                        timeout_ms,
                        policy_tags: Vec::new(),
                    }),
                    on_timeout: TimeoutPolicy::AlternateBranch,
                },
                StageSpec {
                    stage_id: stage2_id,
                    entry_packets: Vec::new(),
                    gates: Vec::new(),
                    advance_to: AdvanceTo::Terminal,
                    timeout: None,
                    on_timeout: TimeoutPolicy::Fail,
                },
            ],
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
                trust: None,
            }],
            policies: Vec::new(),
            schemas: Vec::new(),
            default_tenant_id: None,
        };
        Self {
            scenario_id,
            run_id: RunId::new(run_id),
            tenant_id: default_tenant_id(),
            namespace_id,
            stage_id: stage1_id,
            spec,
        }
    }

    /// Returns a run config for the fixture.
    pub fn run_config(&self) -> RunConfig {
        RunConfig {
            tenant_id: self.tenant_id,
            namespace_id: self.namespace_id,
            run_id: self.run_id.clone(),
            scenario_id: self.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        }
    }

    /// Builds an evidence context for the fixture.
    pub fn evidence_context(&self, trigger_id: &str, time: Timestamp) -> EvidenceContext {
        EvidenceContext {
            tenant_id: self.tenant_id,
            namespace_id: self.namespace_id,
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
            tenant_id: self.tenant_id,
            namespace_id: self.namespace_id,
            run_id: self.run_id.clone(),
            kind: TriggerKind::ExternalEvent,
            time,
            source_id: "system-tests".to_string(),
            payload: None,
            correlation_id: None,
        }
    }
}
