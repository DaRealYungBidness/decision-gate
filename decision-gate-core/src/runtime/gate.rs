// decision-gate-core/src/runtime/gate.rs
// ============================================================================
// Module: Decision Gate Gate Evaluation
// Description: Gate evaluation helpers and trace collection.
// Purpose: Evaluate requirement trees against evidence snapshots deterministically.
// Dependencies: crate::core, ret-logic
// ============================================================================

//! ## Overview
//! Gate evaluation bridges the requirement algebra with evidence snapshots to
//! produce deterministic tri-state outcomes and trace logs.

// ============================================================================
// SECTION: Imports
// ============================================================================

use ret_logic::LogicMode;
use ret_logic::Requirement;
use ret_logic::RequirementTrace;
use ret_logic::TriState;
use ret_logic::TriStateConditionEval;

use crate::core::ConditionId;
use crate::core::GateEvaluation;
use crate::core::GateSpec;
use crate::core::GateTraceEntry;
use crate::core::state::EvidenceRecord;

// ============================================================================
// SECTION: Gate Evaluator
// ============================================================================

/// Evaluates gates against evidence snapshots using tri-state logic.
pub struct GateEvaluator {
    /// Logic mode used for tri-state evaluation.
    logic: LogicMode,
}

impl GateEvaluator {
    /// Creates a new gate evaluator with the provided logic mode.
    #[must_use]
    pub const fn new(logic: LogicMode) -> Self {
        Self {
            logic,
        }
    }

    /// Returns the active logic mode.
    #[must_use]
    pub const fn logic(&self) -> LogicMode {
        self.logic
    }

    /// Evaluates a gate against a precomputed evidence snapshot.
    #[must_use]
    pub fn evaluate_gate(&self, gate: &GateSpec, snapshot: &EvidenceSnapshot) -> GateEvaluation {
        let reader = EvidenceReader {
            snapshot,
        };
        let mut trace = GateTrace::default();
        let status = gate.requirement.eval_tristate_with_trace(&reader, 0, &self.logic, &mut trace);

        GateEvaluation {
            gate_id: gate.gate_id.clone(),
            status,
            trace: trace.entries,
        }
    }
}

// ============================================================================
// SECTION: Evidence Snapshot
// ============================================================================

/// Evidence snapshot keyed by condition identifier.
#[derive(Debug, Clone, Default)]
pub struct EvidenceSnapshot {
    /// Evidence records keyed by condition.
    records: Vec<EvidenceRecord>,
}

impl EvidenceSnapshot {
    /// Creates a new evidence snapshot.
    #[must_use]
    pub const fn new(records: Vec<EvidenceRecord>) -> Self {
        Self {
            records,
        }
    }

    /// Returns the status for a condition, or `Unknown` if missing.
    #[must_use]
    pub fn status_for(&self, condition_id: &ConditionId) -> TriState {
        self.records
            .iter()
            .find(|record| &record.condition_id == condition_id)
            .map_or(TriState::Unknown, |record| record.status)
    }

    /// Returns evidence records.
    #[must_use]
    pub fn records(&self) -> &[EvidenceRecord] {
        &self.records
    }
}

#[doc(hidden)]
pub struct EvidenceReader<'a> {
    snapshot: &'a EvidenceSnapshot,
}

impl TriStateConditionEval for ConditionId {
    type Reader<'a> = EvidenceReader<'a>;

    fn eval_row_tristate(&self, reader: &Self::Reader<'_>, _row: usize) -> TriState {
        reader.snapshot.status_for(self)
    }
}

// ============================================================================
// SECTION: Gate Trace
// ============================================================================

/// Gate evaluation trace collector.
#[derive(Default)]
struct GateTrace {
    /// Trace entries captured during evaluation.
    entries: Vec<GateTraceEntry>,
}

impl RequirementTrace<ConditionId> for GateTrace {
    fn on_condition_evaluated(&mut self, condition_id: &ConditionId, result: TriState) {
        self.entries.push(GateTraceEntry {
            condition_id: condition_id.clone(),
            status: result,
        });
    }
}

// ============================================================================
// SECTION: Condition Collection
// ============================================================================

/// Collects unique condition ids in a requirement tree.
#[must_use]
pub fn collect_conditions(requirement: &Requirement<ConditionId>) -> Vec<ConditionId> {
    let mut out = Vec::new();
    collect_conditions_inner(requirement, &mut out);
    out
}

/// Walks a requirement tree and appends condition ids.
fn collect_conditions_inner(requirement: &Requirement<ConditionId>, out: &mut Vec<ConditionId>) {
    match requirement {
        Requirement::Condition(condition_id) => {
            if !out.contains(condition_id) {
                out.push(condition_id.clone());
            }
        }
        Requirement::Not(inner) => collect_conditions_inner(inner, out),
        Requirement::And(reqs) | Requirement::Or(reqs) => {
            for req in reqs {
                collect_conditions_inner(req, out);
            }
        }
        Requirement::RequireGroup {
            reqs, ..
        } => {
            for req in reqs {
                collect_conditions_inner(req, out);
            }
        }
    }
}
