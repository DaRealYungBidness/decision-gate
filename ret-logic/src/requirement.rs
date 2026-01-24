// ret-logic/src/requirement.rs
// ============================================================================
// Module: Requirement Core Types
// Description: Universal Boolean algebra over typed predicates.
// Purpose: Define `Requirement`, `RequirementId`, and `RequirementGroup` structures along with
// helpers. Dependencies: serde::{Deserialize, Serialize}, smallvec::SmallVec
// ============================================================================

//! ## Overview
//! This module defines the core requirement structure, its identity, and the
//! grouped logical operators that power the universal predicate algebra while
//! preserving short-circuit evaluation guarantees.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fmt;
use std::num::NonZeroU64;

use serde::Deserialize;
use serde::Serialize;
use smallvec::SmallVec;

use crate::traits::TriStatePredicateEval;
use crate::tristate::GroupCounts;
use crate::tristate::NoopTrace;
use crate::tristate::RequirementTrace;
use crate::tristate::TriLogic;
use crate::tristate::TriState;

// ============================================================================
// SECTION: Requirement Id
// ============================================================================

/// A unique identifier for requirements
///
/// This opaque identifier allows requirements to be referenced by ID
/// rather than storing the full requirement structure inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct RequirementId(pub NonZeroU64);

/// Errors that can occur while constructing a [`RequirementId`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementIdError {
    /// The provided raw ID was zero, which is not allowed
    Zero,
}

impl fmt::Display for RequirementIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "RequirementId cannot be zero"),
        }
    }
}

impl std::error::Error for RequirementIdError {}

impl RequirementId {
    /// Creates a new requirement ID from a known non-zero value.
    #[must_use]
    pub const fn new(id: NonZeroU64) -> Self {
        Self(id)
    }

    /// Attempts to create a requirement ID, returning `None` when the raw value is zero.
    #[must_use]
    pub fn from_raw(id: u64) -> Option<Self> {
        NonZeroU64::new(id).map(Self::new)
    }

    /// Returns the raw ID value
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.0.get()
    }
}

impl From<RequirementId> for u64 {
    fn from(id: RequirementId) -> Self {
        id.value()
    }
}

impl TryFrom<u64> for RequirementId {
    type Error = RequirementIdError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::from_raw(value).ok_or(RequirementIdError::Zero)
    }
}

// ============================================================================
// SECTION: Requirement Definition
// ============================================================================

/// Universal requirement tree with domain-specific leaves
///
/// This enum represents the core of the requirement system - a composable
/// Boolean algebra that works over any domain-specific predicate type.
/// The logical operators (And, Or, Not, `RequireGroup`) are universal and
/// domain-agnostic, while the Predicate variant serves as the boundary
/// where domain-specific semantics are injected.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub enum Requirement<P> {
    /// Logical AND: All sub-requirements must be satisfied
    ///
    /// Evaluation short-circuits on the first failure. Empty And is
    /// trivially satisfied (mathematical identity).
    And(SmallVec<[Box<Self>; 4]>),

    /// Logical OR: At least one sub-requirement must be satisfied
    ///
    /// Evaluation short-circuits on the first success. Empty Or is
    /// trivially unsatisfiable (no options available).
    Or(SmallVec<[Box<Self>; 4]>),

    /// Logical NOT: Inverts the result of the sub-requirement
    ///
    /// Boxed to keep the enum size manageable since Not is less common.
    Not(Box<Self>),

    /// Group requirement: At least `min` of the sub-requirements must be satisfied
    ///
    /// This enables "complete at least N of these M tasks" logic with
    /// optimized evaluation that exits early when success/failure is
    /// mathematically determined.
    RequireGroup {
        /// Minimum number of sub-requirements that must be satisfied
        min: u8,
        /// The sub-requirements to choose from
        reqs: SmallVec<[Box<Self>; 8]>,
    },

    /// Domain-specific atomic predicate
    ///
    /// This is the optimization boundary where universal logic hands off
    /// to domain-specific evaluation.
    Predicate(P),
}

// ============================================================================
// SECTION: Execution Helpers
// ============================================================================

impl<P> Requirement<P> {
    /// Evaluates this requirement with aggressive short-circuiting
    ///
    /// This method implements the universal Boolean logic with optimal
    /// control flow. The actual predicate evaluation is delegated to
    /// the domain through the `PredicateEval` trait.
    ///
    /// Note: This method is for the old evaluation approach. New code should use
    /// the row-based evaluation via `PlanExecutor` instead.
    pub fn eval(&self, reader: &P::Reader<'_>, row: super::traits::Row) -> bool
    where
        P: super::traits::PredicateEval,
    {
        match self {
            // Delegate to domain-specific predicate evaluation
            Self::Predicate(predicate) => predicate.eval_row(reader, row),

            // Simple negation
            Self::Not(requirement) => !requirement.eval(reader, row),

            // Short-circuit AND: exit on first failure
            Self::And(requirements) => {
                for req in requirements {
                    if !req.eval(reader, row) {
                        return false;
                    }
                }
                true
            }

            // Short-circuit OR: exit on first success
            Self::Or(requirements) => {
                for req in requirements {
                    if req.eval(reader, row) {
                        return true;
                    }
                }
                false
            }

            // Optimized group evaluation with mathematical early exit
            Self::RequireGroup {
                min,
                reqs,
            } => {
                let mut satisfied = 0usize;
                let mut remaining = reqs.len();

                for req in reqs {
                    if req.eval(reader, row) {
                        satisfied += 1;
                        // Success early exit: we have enough satisfied requirements
                        if satisfied >= usize::from(*min) {
                            return true;
                        }
                    }

                    remaining = remaining.saturating_sub(1);
                    // Failure early exit: impossible to satisfy even if all remaining pass
                    if satisfied + remaining < usize::from(*min) {
                        return false;
                    }
                }

                satisfied >= usize::from(*min)
            }
        }
    }

    /// Evaluates this requirement for up to 64 consecutive rows, returning a bitmask.
    ///
    /// This provides a "mask-space" execution path for the universal requirement tree:
    /// - Leaves call into the domain via [`super::traits::BatchPredicateEval::eval_block`].
    /// - Internal nodes combine masks with bitwise operations.
    ///
    /// Domains reach the performance ceiling by overriding leaf `eval_block` with
    /// efficient batch kernels over their `SoA` readers.
    #[inline]
    pub fn eval_block(
        &self,
        reader: &P::Reader<'_>,
        start: super::traits::Row,
        count: usize,
    ) -> super::traits::Mask64
    where
        P: super::traits::BatchPredicateEval,
    {
        let n = count.min(64);
        if n == 0 {
            return 0;
        }

        let valid_mask: super::traits::Mask64 =
            if n == 64 { super::traits::Mask64::MAX } else { (1u64 << n) - 1 };

        match self {
            Self::Predicate(predicate) => predicate.eval_block(reader, start, n) & valid_mask,
            Self::Not(requirement) => (!requirement.eval_block(reader, start, n)) & valid_mask,
            Self::And(requirements) => {
                // Empty AND is trivially satisfied.
                let mut mask = valid_mask;
                for req in requirements {
                    mask &= req.eval_block(reader, start, n);
                    if mask == 0 {
                        return 0;
                    }
                }
                mask
            }
            Self::Or(requirements) => {
                // Empty OR is trivially unsatisfiable.
                let mut mask: super::traits::Mask64 = 0;
                for req in requirements {
                    mask |= req.eval_block(reader, start, n);
                    if mask == valid_mask {
                        return valid_mask;
                    }
                }
                mask & valid_mask
            }
            Self::RequireGroup {
                min,
                reqs,
            } => {
                let min_required = usize::from(*min);
                if min_required == 0 {
                    return valid_mask;
                }
                if min_required > reqs.len() {
                    return 0;
                }

                if min_required == 1 {
                    let mut mask: super::traits::Mask64 = 0;
                    for req in reqs {
                        mask |= req.eval_block(reader, start, n);
                        if mask == valid_mask {
                            return valid_mask;
                        }
                    }
                    return mask & valid_mask;
                }
                if min_required == reqs.len() {
                    let mut mask = valid_mask;
                    for req in reqs {
                        mask &= req.eval_block(reader, start, n);
                        if mask == 0 {
                            return 0;
                        }
                    }
                    return mask & valid_mask;
                }

                let mut counts: [u8; 64] = [0; 64];
                for req in reqs {
                    let mask = req.eval_block(reader, start, n) & valid_mask;
                    for (idx, count) in counts.iter_mut().enumerate().take(n) {
                        if ((mask >> idx) & 1) == 1 {
                            *count = count.saturating_add(1);
                        }
                    }
                }

                let mut out: super::traits::Mask64 = 0;
                for (idx, count) in counts.iter().enumerate().take(n) {
                    if usize::from(*count) >= min_required {
                        out |= 1u64 << idx;
                    }
                }
                out & valid_mask
            }
        }
    }

    // ========================================================================
    // SECTION: Tri-State Evaluation
    // ========================================================================

    /// Evaluates this requirement with tri-state semantics
    ///
    /// This method preserves "unknown" when evidence is insufficient and
    /// composes results using the supplied tri-state logic table.
    pub fn eval_tristate<L>(
        &self,
        reader: &P::Reader<'_>,
        row: super::traits::Row,
        logic: &L,
    ) -> TriState
    where
        P: TriStatePredicateEval,
        L: TriLogic,
    {
        let mut trace = NoopTrace;
        self.eval_tristate_with_trace(reader, row, logic, &mut trace)
    }

    /// Evaluates this requirement with tri-state semantics and a trace hook
    pub fn eval_tristate_with_trace<L, T>(
        &self,
        reader: &P::Reader<'_>,
        row: super::traits::Row,
        logic: &L,
        trace: &mut T,
    ) -> TriState
    where
        P: TriStatePredicateEval,
        L: TriLogic,
        T: RequirementTrace<P>,
    {
        match self {
            Self::Predicate(predicate) => {
                let result = predicate.eval_row_tristate(reader, row);
                trace.on_predicate_evaluated(predicate, result);
                result
            }
            Self::Not(requirement) => {
                logic.not(requirement.eval_tristate_with_trace(reader, row, logic, trace))
            }
            Self::And(requirements) => {
                let mut acc = TriState::True;
                for req in requirements {
                    acc = logic.and(acc, req.eval_tristate_with_trace(reader, row, logic, trace));
                }
                acc
            }
            Self::Or(requirements) => {
                let mut acc = TriState::False;
                for req in requirements {
                    acc = logic.or(acc, req.eval_tristate_with_trace(reader, row, logic, trace));
                }
                acc
            }
            Self::RequireGroup {
                min,
                reqs,
            } => {
                let mut satisfied = 0usize;
                let mut unknown = 0usize;

                for req in reqs {
                    match req.eval_tristate_with_trace(reader, row, logic, trace) {
                        TriState::True => satisfied += 1,
                        TriState::Unknown => unknown += 1,
                        TriState::False => {}
                    }
                }

                logic.require_group(
                    *min,
                    GroupCounts {
                        satisfied,
                        unknown,
                        total: reqs.len(),
                    },
                )
            }
        }
    }

    /// Determines if this requirement is trivially satisfied
    pub fn is_trivially_satisfied(&self) -> bool {
        match self {
            // Empty And is trivially satisfied (mathematical identity)
            Self::And(reqs) if reqs.is_empty() => true,

            // And is satisfied if all sub-requirements are trivially satisfied
            Self::And(reqs) => reqs.iter().all(|r| r.is_trivially_satisfied()),

            // Or is satisfied if any sub-requirement is trivially satisfied
            Self::Or(reqs) => reqs.iter().any(|r| r.is_trivially_satisfied()),

            // Not is satisfied if the sub-requirement is trivially unsatisfiable
            Self::Not(req) => req.is_trivially_unsatisfiable(),

            // Group with min = 0 is trivially satisfied
            Self::RequireGroup {
                min, ..
            } if *min == 0 => true,

            // Group is satisfied if enough sub-requirements are trivially satisfied
            Self::RequireGroup {
                min,
                reqs,
            } => {
                let trivially_satisfied_count =
                    reqs.iter().filter(|r| r.is_trivially_satisfied()).count();
                trivially_satisfied_count >= *min as usize
            }

            // Predicates require domain-specific analysis
            Self::Predicate(_) => false,
        }
    }

    /// Determines if this requirement is trivially unsatisfiable
    pub fn is_trivially_unsatisfiable(&self) -> bool {
        match self {
            // Empty Or is trivially unsatisfiable (no options)
            Self::Or(reqs) if reqs.is_empty() => true,

            // And is unsatisfiable if any sub-requirement is trivially unsatisfiable
            Self::And(reqs) => reqs.iter().any(|r| r.is_trivially_unsatisfiable()),

            // Or is unsatisfiable if all sub-requirements are trivially unsatisfiable
            Self::Or(reqs) => reqs.iter().all(|r| r.is_trivially_unsatisfiable()),

            // Not is unsatisfiable if the sub-requirement is trivially satisfied
            Self::Not(req) => req.is_trivially_satisfied(),

            // Group is unsatisfiable if min > total requirements
            Self::RequireGroup {
                min,
                reqs,
            } if *min as usize > reqs.len() => true,

            // Group is unsatisfiable if too many sub-requirements are trivially unsatisfiable
            Self::RequireGroup {
                min,
                reqs,
            } => {
                let unsatisfiable_count =
                    reqs.iter().filter(|r| r.is_trivially_unsatisfiable()).count();
                let max_satisfiable = reqs.len() - unsatisfiable_count;
                max_satisfiable < *min as usize
            }

            // Predicates require domain-specific analysis
            Self::Predicate(_) => false,
        }
    }

    /// Returns the complexity of this requirement tree
    pub fn complexity(&self) -> usize {
        match self {
            Self::Predicate(_) => 1,
            Self::Not(req) => 1 + req.complexity(),
            Self::And(reqs) | Self::Or(reqs) => {
                1 + reqs.iter().map(|r| r.complexity()).sum::<usize>()
            }
            Self::RequireGroup {
                reqs, ..
            } => 1 + reqs.iter().map(|r| r.complexity()).sum::<usize>(),
        }
    }
}

// ============================================================================
// SECTION: Constructor Helpers
// ============================================================================

impl<P> Requirement<P> {
    /// Creates a logical AND of the given requirements
    pub fn and(requirements: Vec<Self>) -> Self {
        Self::And(requirements.into_iter().map(Box::new).collect())
    }

    /// Creates a logical OR of the given requirements
    pub fn or(requirements: Vec<Self>) -> Self {
        Self::Or(requirements.into_iter().map(Box::new).collect())
    }

    /// Creates a logical NOT of the given requirement
    pub fn negate(requirement: Self) -> Self {
        Self::Not(Box::new(requirement))
    }

    /// Creates a group requirement with minimum satisfaction count
    pub fn require_group(min: u8, requirements: Vec<Self>) -> Self {
        Self::RequireGroup {
            min,
            reqs: requirements.into_iter().map(Box::new).collect(),
        }
    }

    /// Creates a requirement from a predicate
    pub const fn predicate(predicate: P) -> Self {
        Self::Predicate(predicate)
    }
}

impl<P> std::ops::Not for Requirement<P> {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::Not(Box::new(self))
    }
}

// ============================================================================
// SECTION: Default Implementations
// ============================================================================

impl<P> Default for Requirement<P> {
    /// Creates an empty And requirement (trivially satisfied)
    fn default() -> Self {
        Self::And(SmallVec::new())
    }
}

// ============================================================================
// SECTION: Requirement Groups
// ============================================================================

/// A group of requirements with a minimum satisfaction count
///
/// This structure enables complex requirements like "complete at least 3 out of 5 tasks".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequirementGroup<P> {
    /// The requirements in this group
    pub requirements: SmallVec<[Box<Requirement<P>>; 8]>,

    /// The minimum number of requirements that must be satisfied
    pub min_required: usize,
}

// ============================================================================
// SECTION: Requirement Group Helpers
// ============================================================================

/// Errors that can occur while constructing a [`RequirementGroup`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementGroupError {
    /// The requested minimum exceeds the number of provided requirements
    MinExceedsCount {
        /// Minimum number of requirements that must be satisfied
        min_required: usize,
        /// Number of requirements provided
        available: usize,
    },
}

impl fmt::Display for RequirementGroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MinExceedsCount {
                min_required,
                available,
            } => write!(
                f,
                "min_required ({min_required}) must not exceed the number of requirements \
                 ({available})"
            ),
        }
    }
}

impl std::error::Error for RequirementGroupError {}

impl<P> RequirementGroup<P> {
    /// Creates a new requirement group
    ///
    /// # Errors
    ///
    /// Returns an error when `min_required` exceeds the number of provided requirements.
    pub fn new(
        requirements: Vec<Requirement<P>>,
        min_required: usize,
    ) -> Result<Self, RequirementGroupError> {
        let available = requirements.len();
        if min_required > available {
            return Err(RequirementGroupError::MinExceedsCount {
                min_required,
                available,
            });
        }

        Ok(Self {
            requirements: requirements.into_iter().map(Box::new).collect(),
            min_required,
        })
    }

    /// Creates a group where all requirements must be satisfied
    pub fn all(requirements: Vec<Requirement<P>>) -> Self {
        Self {
            min_required: requirements.len(),
            requirements: requirements.into_iter().map(Box::new).collect(),
        }
    }

    /// Creates a group where at least one requirement must be satisfied
    ///
    /// # Errors
    ///
    /// Returns [`RequirementGroupError::MinExceedsCount`] when called with an empty set.
    pub fn any(requirements: Vec<Requirement<P>>) -> Result<Self, RequirementGroupError> {
        Self::new(requirements, 1)
    }
}
