// ret-logic/src/tristate.rs
// ============================================================================
// Module: Tri-State Logic
// Description: Tri-state truth values and configurable logic tables.
// Purpose: Provide deterministic tri-state evaluation for requirement gates.
// Dependencies: serde::{Deserialize, Serialize}
// ============================================================================

//! ## Overview
//! Defines tri-state truth values (`true/false/unknown`) and logic tables that
//! can be swapped to match domain needs. The default logic is strong Kleene,
//! which preserves fail-closed semantics when evidence is incomplete.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// SECTION: Tri-State Value
// ============================================================================

/// Tri-state truth value for evidence-aware evaluation
///
/// # Invariants
/// - Represents a closed set of truth values: true, false, or unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriState {
    /// Definitively true
    True,
    /// Definitively false
    False,
    /// Indeterminate due to missing or insufficient evidence
    Unknown,
}

impl TriState {
    /// Returns true if the value is `True`
    #[must_use]
    pub const fn is_true(self) -> bool {
        matches!(self, Self::True)
    }

    /// Returns true if the value is `False`
    #[must_use]
    pub const fn is_false(self) -> bool {
        matches!(self, Self::False)
    }

    /// Returns true if the value is `Unknown`
    #[must_use]
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }
}

impl From<bool> for TriState {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
}

// ============================================================================
// SECTION: Group Semantics
// ============================================================================

/// Aggregated counts for group evaluation
///
/// # Invariants
/// - Callers should ensure `satisfied + unknown <= total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupCounts {
    /// Number of satisfied requirements
    pub satisfied: usize,
    /// Number of unknown requirements
    pub unknown: usize,
    /// Total number of requirements in the group
    pub total: usize,
}

impl GroupCounts {
    /// Returns the number of definitively failed requirements
    #[must_use]
    pub const fn failed(self) -> usize {
        self.total.saturating_sub(self.satisfied + self.unknown)
    }
}

// ============================================================================
// SECTION: Logic Tables
// ============================================================================

/// Tri-state logic tables for composable evaluation
pub trait TriLogic {
    /// Logical AND for tri-state values
    fn and(&self, lhs: TriState, rhs: TriState) -> TriState;

    /// Logical OR for tri-state values
    fn or(&self, lhs: TriState, rhs: TriState) -> TriState;

    /// Logical NOT for tri-state values
    fn not(&self, value: TriState) -> TriState;

    /// Group evaluation semantics (default: "insufficient evidence")
    fn require_group(&self, min: u8, counts: GroupCounts) -> TriState {
        let min_required = usize::from(min);
        if min_required == 0 {
            return TriState::True;
        }

        if counts.satisfied >= min_required {
            return TriState::True;
        }

        if counts.satisfied + counts.unknown < min_required {
            return TriState::False;
        }

        TriState::Unknown
    }
}

/// Strong Kleene logic (default)
///
/// # Invariants
/// - Zero-sized marker type; carries no state.
#[derive(Debug, Clone, Copy)]
pub struct KleeneLogic;

impl TriLogic for KleeneLogic {
    fn and(&self, lhs: TriState, rhs: TriState) -> TriState {
        match (lhs, rhs) {
            (TriState::False, _) | (_, TriState::False) => TriState::False,
            (TriState::True, TriState::True) => TriState::True,
            _ => TriState::Unknown,
        }
    }

    fn or(&self, lhs: TriState, rhs: TriState) -> TriState {
        match (lhs, rhs) {
            (TriState::True, _) | (_, TriState::True) => TriState::True,
            (TriState::False, TriState::False) => TriState::False,
            _ => TriState::Unknown,
        }
    }

    fn not(&self, value: TriState) -> TriState {
        match value {
            TriState::True => TriState::False,
            TriState::False => TriState::True,
            TriState::Unknown => TriState::Unknown,
        }
    }
}

/// Bochvar logic (infectious unknowns)
///
/// # Invariants
/// - Zero-sized marker type; carries no state.
#[derive(Debug, Clone, Copy)]
pub struct BochvarLogic;

impl TriLogic for BochvarLogic {
    fn and(&self, lhs: TriState, rhs: TriState) -> TriState {
        match (lhs, rhs) {
            (TriState::Unknown, _) | (_, TriState::Unknown) => TriState::Unknown,
            (TriState::False, _) | (_, TriState::False) => TriState::False,
            _ => TriState::True,
        }
    }

    fn or(&self, lhs: TriState, rhs: TriState) -> TriState {
        match (lhs, rhs) {
            (TriState::Unknown, _) | (_, TriState::Unknown) => TriState::Unknown,
            (TriState::True, _) | (_, TriState::True) => TriState::True,
            _ => TriState::False,
        }
    }

    fn not(&self, value: TriState) -> TriState {
        match value {
            TriState::True => TriState::False,
            TriState::False => TriState::True,
            TriState::Unknown => TriState::Unknown,
        }
    }
}

/// Runtime-selectable logic mode
///
/// # Invariants
/// - Enumerates the supported tri-state logic tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicMode {
    /// Strong Kleene logic (default)
    Kleene,
    /// Bochvar logic (infectious unknowns)
    Bochvar,
}

impl TriLogic for LogicMode {
    fn and(&self, lhs: TriState, rhs: TriState) -> TriState {
        match self {
            Self::Kleene => KleeneLogic.and(lhs, rhs),
            Self::Bochvar => BochvarLogic.and(lhs, rhs),
        }
    }

    fn or(&self, lhs: TriState, rhs: TriState) -> TriState {
        match self {
            Self::Kleene => KleeneLogic.or(lhs, rhs),
            Self::Bochvar => BochvarLogic.or(lhs, rhs),
        }
    }

    fn not(&self, value: TriState) -> TriState {
        match self {
            Self::Kleene => KleeneLogic.not(value),
            Self::Bochvar => BochvarLogic.not(value),
        }
    }

    fn require_group(&self, min: u8, counts: GroupCounts) -> TriState {
        match self {
            Self::Kleene | Self::Bochvar => KleeneLogic.require_group(min, counts),
        }
    }
}

// ============================================================================
// SECTION: Trace Hooks
// ============================================================================

/// Trace hook for predicate evaluation
pub trait RequirementTrace<P> {
    /// Called whenever a predicate is evaluated
    fn on_predicate_evaluated(&mut self, predicate: &P, result: TriState);
}

/// No-op trace hook for fast paths
///
/// # Invariants
/// - Zero-sized marker type; carries no state.
#[derive(Debug, Default)]
pub struct NoopTrace;

impl<P> RequirementTrace<P> for NoopTrace {
    fn on_predicate_evaluated(&mut self, _predicate: &P, _result: TriState) {}
}
