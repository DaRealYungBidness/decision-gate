// ret-logic/src/error.rs
// ============================================================================
// Module: Requirement Error Definitions
// Description: Structured diagnostics for the requirement system.
// Purpose: Provide rich diagnostics and helper getters for requirement failures.
// Dependencies: serde::{Serialize, Deserialize}, std::fmt
// ============================================================================

//! ## Overview
//! Centralizes the requirement evaluation errors, their user-facing messaging,
//! conversions, and serialization guarantees so evaluation and UI layers remain
//! decoupled while still exposing actionable diagnostics.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

/// Errors that can occur during requirement evaluation
///
/// This enum represents the various ways requirement evaluation can fail,
/// from logical composition failures to domain-specific condition failures.
/// The error types are designed to provide clear diagnostic information
/// while maintaining zero-allocation evaluation paths where possible.
///
/// # Invariants
/// - None. Variants capture structured evaluation failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequirementError {
    // ============================================================================
    // SECTION: Logical Composition Errors
    // ============================================================================
    /// A group requirement failed because not enough sub-requirements were satisfied
    GroupRequirementFailed {
        /// How many requirements were actually passed
        passed: usize,
        /// How many requirements needed to pass
        required: usize,
    },

    /// All requirements in an OR clause failed
    OrAllFailed,

    /// The inner requirement of a NOT clause was satisfied (making the NOT fail)
    NotFailed,

    // ============================================================================
    // SECTION: Evaluation Context Errors
    // ============================================================================
    /// The subject wasn't available in the evaluation context
    SubjectNotAvailable,

    /// Target wasn't available in the evaluation context when required
    TargetNotAvailable,

    /// World state wasn't available or accessible
    WorldStateUnavailable,

    // ============================================================================
    // SECTION: Domain Condition Errors
    // ============================================================================
    /// A domain-specific condition failed evaluation
    ///
    /// This provides a user-friendly message explaining why the condition failed,
    /// suitable for displaying to users in UIs or error messages.
    ConditionFailed(String),

    /// A domain condition encountered an internal error during evaluation
    ///
    /// This is for technical errors like missing components, invalid state, etc.
    /// that are not user-facing requirement failures.
    ConditionError(String),

    // ============================================================================
    // SECTION: Structural Errors
    // ============================================================================
    /// Invalid requirement structure was encountered
    InvalidStructure(String),

    /// Requirement tree too deep (potential stack overflow protection)
    TooDeep {
        /// Maximum allowed recursion depth
        max_depth: usize,
        /// Depth encountered while evaluating
        actual_depth: usize,
    },

    // ============================================================================
    // SECTION: Generic Error
    // ============================================================================
    /// An error occurred that doesn't fit other categories
    Other(String),
}

// ============================================================================
// SECTION: Display Implementation
// ============================================================================

impl fmt::Display for RequirementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GroupRequirementFailed {
                passed,
                required,
            } => {
                write!(f, "Group requirement failed: passed {passed}, needed {required}")
            }
            Self::OrAllFailed => {
                write!(f, "All alternatives in OR requirement failed")
            }
            Self::NotFailed => {
                write!(f, "NOT requirement failed: inner requirement was satisfied")
            }
            Self::SubjectNotAvailable => {
                write!(f, "Subject not available in evaluation context")
            }
            Self::TargetNotAvailable => {
                write!(f, "Target not available in evaluation context")
            }
            Self::WorldStateUnavailable => {
                write!(f, "World state unavailable or inaccessible")
            }
            Self::ConditionFailed(msg) => {
                write!(f, "Requirement not met: {msg}")
            }
            Self::ConditionError(msg) => {
                write!(f, "Condition evaluation error: {msg}")
            }
            Self::InvalidStructure(msg) => {
                write!(f, "Invalid requirement structure: {msg}")
            }
            Self::TooDeep {
                max_depth,
                actual_depth,
            } => {
                write!(f, "Requirement tree too deep: {actual_depth} levels (max {max_depth})")
            }
            Self::Other(msg) => {
                write!(f, "Requirement error: {msg}")
            }
        }
    }
}

// ============================================================================
// SECTION: Standard Trait Implementations
// ============================================================================

impl std::error::Error for RequirementError {}

// ============================================================================
// SECTION: Convenience Helpers
// ============================================================================

impl RequirementError {
    /// Returns a user-friendly message for this error
    ///
    /// This produces messages suitable for in-game UI,
    /// formatted for player consumption rather than debugging.
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            Self::GroupRequirementFailed {
                passed,
                required,
            } => {
                let remaining = required.saturating_sub(*passed);
                format!(
                    "You need to meet {} more requirement{}",
                    remaining,
                    if remaining == 1 { "" } else { "s" }
                )
            }
            Self::OrAllFailed => "None of the alternative requirements were met".to_string(),
            Self::NotFailed => "A condition that should not be true was satisfied".to_string(),
            Self::SubjectNotAvailable => {
                "Cannot evaluate requirement: no subject available".to_string()
            }
            Self::TargetNotAvailable => {
                "Cannot evaluate requirement: no target available".to_string()
            }
            Self::WorldStateUnavailable => {
                "Cannot evaluate requirement: world state unavailable".to_string()
            }
            Self::ConditionFailed(msg) => msg.clone(),
            Self::ConditionError(_) => {
                "An internal error occurred while checking requirements".to_string()
            }
            Self::InvalidStructure(_) => "Invalid requirement configuration".to_string(),
            Self::TooDeep {
                ..
            } => "Requirement too complex to evaluate".to_string(),
            Self::Other(msg) => {
                format!("Requirement not met: {msg}")
            }
        }
    }

    /// Creates a condition failure error with a custom message
    pub fn condition_failed(message: impl Into<String>) -> Self {
        Self::ConditionFailed(message.into())
    }

    /// Creates a condition error (technical failure) with a custom message
    pub fn condition_error(message: impl Into<String>) -> Self {
        Self::ConditionError(message.into())
    }

    /// Creates a generic error with a custom message
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    /// Creates an invalid structure error
    pub fn invalid_structure(message: impl Into<String>) -> Self {
        Self::InvalidStructure(message.into())
    }
}

// ============================================================================
// SECTION: Conversion Helpers
// ============================================================================

// Allow converting strings to RequirementError
impl From<String> for RequirementError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

impl From<&str> for RequirementError {
    fn from(message: &str) -> Self {
        Self::Other(message.to_string())
    }
}

// ============================================================================
// SECTION: Result Alias
// ============================================================================

/// Convenient Result type for requirement operations
pub type RequirementResult<T = ()> = Result<T, RequirementError>;

// Tests are in the central tests module (tests/error.rs)
