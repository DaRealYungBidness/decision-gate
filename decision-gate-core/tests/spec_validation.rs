// decision-gate-core/tests/spec_validation.rs
// ============================================================================
// Module: Scenario Spec Validation Tests
// Description: Tests for spec invariants and validation errors.
// Purpose: Ensure scenario specs fail closed on malformed definitions.
// Dependencies: decision-gate-core, ret-logic
// ============================================================================
//! ## Overview
//! Exercises `ScenarioSpec` validation errors and the success path.
//!
//! Security posture: Spec validation is a trust boundary - must fail closed.
//! Threat model: TM-SPEC-001 - Spec injection or bypass.

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

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketId;
use decision_gate_core::PacketSpec;
use decision_gate_core::PredicateKey;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecError;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TimeoutPolicy;
use serde_json::json;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn base_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: vec![PacketSpec {
                packet_id: PacketId::new("packet-1"),
                schema_id: decision_gate_core::SchemaId::new("schema-1"),
                content_type: "application/json".to_string(),
                visibility_labels: vec!["public".to_string()],
                policy_tags: Vec::new(),
                expiry: None,
                payload: decision_gate_core::PacketPayload::Json {
                    value: json!({"hello": "world"}),
                },
            }],
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::predicate(PredicateKey::from("ready")),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
            predicate: PredicateKey::from("ready"),
            query: EvidenceQuery {
                provider_id: ProviderId::new("time"),
                predicate: "now".to_string(),
                params: Some(json!({})),
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

// ============================================================================
// SECTION: Success Path
// ============================================================================

/// Verifies a well-formed spec validates successfully.
#[test]
fn spec_validate_accepts_valid_spec() {
    let spec = base_spec();
    assert!(spec.validate().is_ok());
}

// ============================================================================
// SECTION: Structural Validation
// ============================================================================

/// Verifies missing stages are rejected.
#[test]
fn spec_validate_rejects_missing_stages() {
    let mut spec = base_spec();
    spec.stages.clear();
    assert!(matches!(spec.validate(), Err(SpecError::MissingStages)));
}

/// Verifies duplicate stage IDs are rejected.
#[test]
fn spec_validate_rejects_duplicate_stage_ids() {
    let mut spec = base_spec();
    spec.stages.push(spec.stages[0].clone());
    assert!(matches!(spec.validate(), Err(SpecError::DuplicateStageId(_))));
}

/// Verifies duplicate gate IDs are rejected across stages.
#[test]
fn spec_validate_rejects_duplicate_gate_ids() {
    let mut spec = base_spec();
    let mut stage = spec.stages[0].clone();
    stage.stage_id = StageId::new("stage-2");
    stage.gates[0].gate_id = GateId::new("gate-1");
    spec.stages.push(stage);
    assert!(matches!(spec.validate(), Err(SpecError::DuplicateGateId(_))));
}

/// Verifies duplicate packet IDs are rejected across stages.
#[test]
fn spec_validate_rejects_duplicate_packet_ids() {
    let mut spec = base_spec();
    let mut stage = spec.stages[0].clone();
    stage.stage_id = StageId::new("stage-2");
    stage.entry_packets[0].packet_id = PacketId::new("packet-1");
    stage.gates.clear();
    spec.stages.push(stage);
    assert!(matches!(spec.validate(), Err(SpecError::DuplicatePacketId(_))));
}

/// Verifies duplicate predicate keys are rejected.
#[test]
fn spec_validate_rejects_duplicate_predicates() {
    let mut spec = base_spec();
    spec.predicates.push(spec.predicates[0].clone());
    assert!(matches!(spec.validate(), Err(SpecError::DuplicatePredicate(_))));
}

// ============================================================================
// SECTION: Predicate Validation
// ============================================================================

/// Verifies missing predicate definitions are rejected.
#[test]
fn spec_validate_rejects_missing_predicates() {
    let mut spec = base_spec();
    spec.predicates.clear();
    assert!(matches!(spec.validate(), Err(SpecError::MissingPredicate(_))));
}

/// Verifies empty provider IDs are rejected.
#[test]
fn spec_validate_rejects_empty_provider_id() {
    let mut spec = base_spec();
    spec.predicates[0].query.provider_id = ProviderId::new(" ");
    assert!(matches!(spec.validate(), Err(SpecError::InvalidEvidenceQuery(_, _))));
}

/// Verifies empty predicate names are rejected.
#[test]
fn spec_validate_rejects_empty_query_predicate() {
    let mut spec = base_spec();
    spec.predicates[0].query.predicate = "   ".to_string();
    assert!(matches!(spec.validate(), Err(SpecError::InvalidEvidenceQuery(_, _))));
}

// ============================================================================
// SECTION: Branch Validation
// ============================================================================

/// Verifies fixed branch targets must exist.
#[test]
fn spec_validate_rejects_missing_fixed_branch_target() {
    let mut spec = base_spec();
    spec.stages[0].advance_to = AdvanceTo::Fixed {
        stage_id: StageId::new("missing-stage"),
    };
    assert!(matches!(spec.validate(), Err(SpecError::MissingBranchTarget(_))));
}

/// Verifies branch targets must exist.
#[test]
fn spec_validate_rejects_missing_branch_target() {
    let mut spec = base_spec();
    spec.stages[0].advance_to = AdvanceTo::Branch {
        branches: vec![decision_gate_core::BranchRule {
            gate_id: GateId::new("gate-1"),
            outcome: decision_gate_core::GateOutcome::True,
            next_stage_id: StageId::new("missing-stage"),
        }],
        default: None,
    };
    assert!(matches!(spec.validate(), Err(SpecError::MissingBranchTarget(_))));
}
