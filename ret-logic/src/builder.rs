// ret-logic/src/builder.rs
// ============================================================================
// Module: Requirement Builders
// Description: Fluent builders over the universal requirement tree.
// Purpose: Provide ergonomic, type-safe APIs for composing boolean requirements.
// Dependencies: crate::requirement::requirement::{Requirement, RequirementGroup}
// ============================================================================

//! ## Overview
//! Fluent builders simplify composing requirement trees by enabling chained calls
//! for `and`, `or`, `not`, and grouped semantics while keeping the same invariants
//! as the core [`Requirement`] algebra.

use std::ops::Not;

use crate::requirement::Requirement;

// ============================================================================
// SECTION: Fluent Builder API
// ============================================================================

/// Fluent builder for constructing requirements programmatically
///
/// This builder provides an ergonomic API for creating complex requirement
/// trees in code. It uses the builder pattern to enable chaining and
/// provides compile-time safety for requirement construction.
///
/// # Type Parameter
/// * `P` - The domain-specific predicate type
pub struct RequirementBuilder<P> {
    /// Root requirement under construction.
    requirement: Requirement<P>,
}

impl<P> RequirementBuilder<P> {
    /// Creates a new builder with the given requirement as the root
    pub const fn new(requirement: Requirement<P>) -> Self {
        Self {
            requirement,
        }
    }

    /// Creates a builder starting with a predicate
    #[must_use]
    pub const fn predicate(predicate: P) -> Self {
        Self::new(Requirement::Predicate(predicate))
    }

    /// Creates a builder starting with an And requirement
    #[must_use]
    pub const fn and() -> AndBuilder<P> {
        AndBuilder::<P>::new()
    }

    /// Creates a builder starting with an Or requirement
    #[must_use]
    pub const fn or() -> OrBuilder<P> {
        OrBuilder::<P>::new()
    }

    /// Creates a builder starting with a `RequireGroup`
    #[must_use]
    pub const fn require_group(min: u8) -> GroupBuilder<P> {
        GroupBuilder::<P>::new(min)
    }

    /// Combines this requirement with another using And
    #[must_use]
    pub fn and_also(self, other: Requirement<P>) -> Self {
        Self::new(Requirement::and(vec![self.requirement, other]))
    }

    /// Combines this requirement with another using Or
    #[must_use]
    pub fn or_else(self, other: Requirement<P>) -> Self {
        Self::new(Requirement::or(vec![self.requirement, other]))
    }

    /// Builds the final requirement
    pub fn build(self) -> Requirement<P> {
        self.requirement
    }
}

// ============================================================================
// SECTION: Operator Trait Implementations
// ============================================================================

/// Implements the `!` operator for [`RequirementBuilder`].
///
/// This allows using `!builder` as a more idiomatic alternative to calling
/// a `not()` method. The negation wraps the current requirement in a logical NOT.
///
/// # Examples
///
/// ```
/// # use ret_logic::builder::RequirementBuilder;
/// # use ret_logic::Requirement;
/// # let predicate = ();
/// let builder = RequirementBuilder::predicate(predicate);
/// let negated = !builder; // Equivalent to Requirement::negate(...)
/// ```
impl<P> Not for RequirementBuilder<P> {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::new(Requirement::negate(self.requirement))
    }
}

// ============================================================================
// SECTION: And Builder
// ============================================================================

/// Builder for And requirements with fluent chaining
pub struct AndBuilder<P> {
    /// Requirements collected for the And clause.
    requirements: Vec<Requirement<P>>,
}

impl<P> AndBuilder<P> {
    /// Creates a new And builder
    #[must_use]
    pub const fn new() -> Self {
        Self {
            requirements: Vec::new(),
        }
    }

    /// Adds a requirement to the And clause
    #[must_use]
    pub fn with(mut self, requirement: Requirement<P>) -> Self {
        self.requirements.push(requirement);
        self
    }

    /// Adds a predicate to the And clause
    #[must_use]
    pub fn with_predicate(mut self, predicate: P) -> Self {
        self.requirements.push(Requirement::Predicate(predicate));
        self
    }

    /// Adds multiple requirements to the And clause
    #[must_use]
    pub fn with_all<I>(mut self, requirements: I) -> Self
    where
        I: IntoIterator<Item = Requirement<P>>,
    {
        self.requirements.extend(requirements);
        self
    }

    /// Builds the And requirement
    #[must_use]
    pub fn build(self) -> Requirement<P> {
        Requirement::and(self.requirements)
    }
}

// ============================================================================
// SECTION: Or Builder
// ============================================================================

/// Builder for Or requirements with fluent chaining
pub struct OrBuilder<P> {
    /// Requirements collected for the Or clause.
    requirements: Vec<Requirement<P>>,
}

impl<P> OrBuilder<P> {
    /// Creates a new Or builder
    #[must_use]
    pub const fn new() -> Self {
        Self {
            requirements: Vec::new(),
        }
    }

    /// Adds a requirement to the Or clause
    #[must_use]
    pub fn with(mut self, requirement: Requirement<P>) -> Self {
        self.requirements.push(requirement);
        self
    }

    /// Adds a predicate to the Or clause
    #[must_use]
    pub fn with_predicate(mut self, predicate: P) -> Self {
        self.requirements.push(Requirement::Predicate(predicate));
        self
    }

    /// Adds multiple requirements to the Or clause
    #[must_use]
    pub fn with_all<I>(mut self, requirements: I) -> Self
    where
        I: IntoIterator<Item = Requirement<P>>,
    {
        self.requirements.extend(requirements);
        self
    }

    /// Builds the Or requirement
    #[must_use]
    pub fn build(self) -> Requirement<P> {
        Requirement::or(self.requirements)
    }
}

// ============================================================================
// SECTION: Group Builder
// ============================================================================

/// Builder for `RequireGroup` requirements with fluent chaining
pub struct GroupBuilder<P> {
    /// Minimum number of requirements that must pass.
    min: u8,
    /// Requirements collected for the group.
    requirements: Vec<Requirement<P>>,
}

impl<P> GroupBuilder<P> {
    /// Creates a new `RequireGroup` builder
    #[must_use]
    pub const fn new(min: u8) -> Self {
        Self {
            min,
            requirements: Vec::new(),
        }
    }

    /// Adds a requirement to the group
    #[must_use]
    pub fn with(mut self, requirement: Requirement<P>) -> Self {
        self.requirements.push(requirement);
        self
    }

    /// Adds a predicate to the group
    #[must_use]
    pub fn with_predicate(mut self, predicate: P) -> Self {
        self.requirements.push(Requirement::Predicate(predicate));
        self
    }

    /// Adds multiple requirements to the group
    #[must_use]
    pub fn with_all<I>(mut self, requirements: I) -> Self
    where
        I: IntoIterator<Item = Requirement<P>>,
    {
        self.requirements.extend(requirements);
        self
    }

    /// Updates the minimum required count
    #[must_use]
    pub const fn min(mut self, min: u8) -> Self {
        self.min = min;
        self
    }

    /// Builds the `RequireGroup` requirement
    #[must_use]
    pub fn build(self) -> Requirement<P> {
        Requirement::require_group(self.min, self.requirements)
    }
}

// ============================================================================
// SECTION: Default Implementations
// ============================================================================

impl<P> Default for AndBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> Default for OrBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SECTION: Convenience Facades
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
