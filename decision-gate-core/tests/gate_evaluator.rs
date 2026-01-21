// decision-gate-core/tests/gate_evaluator.rs
// ============================================================================
// Module: Gate Evaluator Tests
// Description: Tests for gate evaluation using evidence snapshots.
// ============================================================================
//! ## Overview
//! Validates deterministic gate evaluation and trace output.

#![allow(clippy::unwrap_used, reason = "Tests use unwrap on deterministic evidence fixtures.")]
#![allow(clippy::expect_used, reason = "Tests use expect for explicit failure messages.")]

use decision_gate_core::EvidenceRecord;
use decision_gate_core::EvidenceResult;
use decision_gate_core::GateEvaluator;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::PredicateKey;
use decision_gate_core::runtime::gate::EvidenceSnapshot;
use ret_logic::LogicMode;
use ret_logic::Requirement;
use ret_logic::TriState;

// ============================================================================
// SECTION: Snapshot Evaluation
// ============================================================================

#[test]
fn test_gate_evaluation_with_snapshot() {
    let gate = GateSpec {
        gate_id: GateId::new("gate-1"),
        requirement: Requirement::and(vec![
            Requirement::predicate(PredicateKey::from("a")),
            Requirement::predicate(PredicateKey::from("b")),
        ]),
    };

    let snapshot = EvidenceSnapshot::new(vec![
        EvidenceRecord {
            predicate: PredicateKey::from("a"),
            status: TriState::True,
            result: EvidenceResult {
                value: None,
                evidence_hash: None,
                evidence_ref: None,
                evidence_anchor: None,
                signature: None,
                content_type: None,
            },
        },
        EvidenceRecord {
            predicate: PredicateKey::from("b"),
            status: TriState::Unknown,
            result: EvidenceResult {
                value: None,
                evidence_hash: None,
                evidence_ref: None,
                evidence_anchor: None,
                signature: None,
                content_type: None,
            },
        },
    ]);

    let evaluator = GateEvaluator::new(LogicMode::Kleene);
    let result = evaluator.evaluate_gate(&gate, &snapshot);

    assert_eq!(result.status, TriState::Unknown);
    assert_eq!(result.trace.len(), 2);
}
