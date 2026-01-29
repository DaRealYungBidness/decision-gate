// ret-logic/src/lib.rs
// ============================================================================
// Module: Requirement Root
// Description: Public API surface for the requirement subsystem.
// Purpose: Wire together core modules, re-exports, and the DSL macro.
// Dependencies: crate::{builder, dsl, error, executor, plan, requirement, serde_support, traits,
//              tristate}
// ============================================================================

//! ## Overview
//! This module exposes the building blocks (errors, plans, traits, execution)
//! plus a domain-agnostic DSL so callers can import requirements uniformly.

// ============================================================================
// SECTION: Core Modules
// ============================================================================

pub mod builder;
pub mod dsl;
pub mod error;
pub mod executor;
pub mod plan;
pub mod requirement;
pub mod serde_support;
pub mod traits;
pub mod tristate;

#[cfg(test)]
mod tests;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use dsl::DslError;
pub use dsl::PredicateResolver;
pub use dsl::parse_requirement;
pub use error::RequirementError;
pub use error::RequirementResult;
pub use executor::PlanExecutor;
pub use plan::ColumnKey;
pub use plan::Constant;
pub use plan::ConstantIndex;
pub use plan::OpCode;
pub use plan::Operation;
pub use plan::Plan;
pub use plan::PlanBuilder;
pub use plan::PlanError;
pub use requirement::Requirement;
pub use requirement::RequirementGroup;
pub use requirement::RequirementGroupError;
pub use requirement::RequirementId;
pub use requirement::RequirementIdError;
pub use traits::BatchPredicateEval;
pub use traits::BoolAsTri;
pub use traits::Mask64;
pub use traits::PredicateEval;
pub use traits::ReaderLen;
pub use traits::Row;
pub use traits::TriStatePredicateEval;
pub use traits::eval_reader_rows;
pub use tristate::BochvarLogic;
pub use tristate::GroupCounts;
pub use tristate::KleeneLogic;
pub use tristate::LogicMode;
pub use tristate::NoopTrace;
pub use tristate::RequirementTrace;
pub use tristate::TriLogic;
pub use tristate::TriState;

// ============================================================================
// SECTION: Convenience DSL
// ============================================================================

/// Convenience functions for creating requirements without builders
pub mod convenience {
    use super::Requirement;

    /// Creates a requirement requiring all of the given requirements
    #[must_use]
    pub fn all<P>(requirements: Vec<Requirement<P>>) -> Requirement<P> {
        Requirement::and(requirements)
    }

    /// Creates a requirement requiring any of the given requirements
    #[must_use]
    pub fn any<P>(requirements: Vec<Requirement<P>>) -> Requirement<P> {
        Requirement::or(requirements)
    }

    /// Creates a requirement that inverts another requirement
    #[must_use]
    pub fn not<P>(requirement: Requirement<P>) -> Requirement<P> {
        Requirement::negate(requirement)
    }

    /// Creates a requirement requiring at least N of the given requirements
    #[must_use]
    pub fn at_least<P>(min: u8, requirements: Vec<Requirement<P>>) -> Requirement<P> {
        Requirement::require_group(min, requirements)
    }

    /// Creates a requirement from a predicate
    #[must_use]
    pub const fn predicate<P>(predicate: P) -> Requirement<P> {
        Requirement::predicate(predicate)
    }
}

// ============================================================================
// SECTION: Requirement Macro
// ============================================================================

/// Macro for ergonomic requirement construction
///
/// This macro provides a DSL-like syntax for building requirements:
///
/// ```ignore
/// let req = requirement! {
///     and [
///         predicate(my_predicate),
///         or [
///             predicate(other_predicate),
///             not(predicate(third_predicate))
///         ],
///         require_group(2, [
///             predicate(option_a),
///             predicate(option_b),
///             predicate(option_c)
///         ])
///     ]
/// };
/// ```
#[macro_export]
macro_rules! requirement {
    // Base case: predicate
    (predicate($pred:expr)) => {
        $crate::requirement::Requirement::predicate($pred)
    };

    // Not case
    (not($req:tt)) => {
        $crate::requirement::Requirement::negate(requirement!($req))
    };

    // And case
    (and [$($req:tt),* $(,)?]) => {
        $crate::requirement::Requirement::and(vec![$(requirement!($req)),*])
    };

    // Or case
    (or [$($req:tt),* $(,)?]) => {
        $crate::requirement::Requirement::or(vec![$(requirement!($req)),*])
    };

    // RequireGroup case
    (require_group($min:expr, [$($req:tt),* $(,)?])) => {
        $crate::requirement::Requirement::require_group($min, vec![$(requirement!($req)),*])
    };
}
