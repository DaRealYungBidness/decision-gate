// ret-logic/tests/support/mocks.rs
// ============================================================================
// Module: Mock Conditions
// Description: Shared mock conditions and readers for requirement tests.
// ============================================================================
//! ## Overview
//! Mock condition and reader types used by integration tests.

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

use ret_logic::BatchConditionEval;
use ret_logic::ConditionEval;
use ret_logic::ReaderLen;
use ret_logic::Row;
use serde::Deserialize;
use serde::Serialize;

// ========================================================================
// Mock Condition Types
// ========================================================================

/// Simple mock condition for testing the requirement system.
///
/// This condition type is domain-agnostic and allows testing the core
/// boolean algebra without any domain-specific logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MockCondition {
    /// Always returns true.
    AlwaysTrue,

    /// Always returns false.
    AlwaysFalse,

    /// Returns true if the value at the row is greater than or equal to threshold.
    ValueGte(i32),

    /// Returns true if the value at the row is less than or equal to threshold.
    ValueLte(i32),

    /// Returns true if the value at the row equals the specified value.
    ValueEq(i32),

    /// Returns true if entity flags contain all required flags.
    HasAllFlags(u64),

    /// Returns true if entity flags contain any of the test flags.
    HasAnyFlags(u64),

    /// Returns true if entity flags contain none of the forbidden flags.
    HasNoneFlags(u64),

    /// Returns true based on a specific row index (for testing specific patterns).
    RowIndexEven,

    /// Returns true for rows where index < threshold.
    RowIndexLt(usize),
}

// ========================================================================
// Mock Reader Type
// ========================================================================

/// Mock reader that provides test data for condition evaluation.
///
/// This reader simulates the `SoA` (Struct of Arrays) pattern used by
/// real readers, providing slices of component data for row-based access.
pub struct MockReader<'a> {
    /// Integer values for numeric conditions.
    values: &'a [i32],

    /// Flags for bitwise conditions.
    flags: &'a [u64],
}

impl<'a> MockReader<'a> {
    /// Creates a new mock reader with the given data.
    #[must_use]
    pub const fn new(values: &'a [i32], flags: &'a [u64]) -> Self {
        Self {
            values,
            flags,
        }
    }
}

impl ReaderLen for MockReader<'_> {
    fn len(&self) -> usize {
        self.values.len()
    }
}

// ========================================================================
// ConditionEval Implementation
// ========================================================================

impl ConditionEval for MockCondition {
    type Reader<'a> = MockReader<'a>;

    #[inline]
    fn eval_row(&self, reader: &Self::Reader<'_>, row: Row) -> bool {
        match *self {
            Self::AlwaysTrue => true,
            Self::AlwaysFalse => false,
            Self::ValueGte(threshold) => reader.values.get(row).is_some_and(|&v| v >= threshold),
            Self::ValueLte(threshold) => reader.values.get(row).is_some_and(|&v| v <= threshold),
            Self::ValueEq(value) => reader.values.get(row).is_some_and(|&v| v == value),
            Self::HasAllFlags(required) => {
                reader.flags.get(row).is_some_and(|&f| (f & required) == required)
            }
            Self::HasAnyFlags(test) => reader.flags.get(row).is_some_and(|&f| (f & test) != 0),
            Self::HasNoneFlags(forbidden) => {
                reader.flags.get(row).is_none_or(|&f| (f & forbidden) == 0)
            }
            Self::RowIndexEven => row.is_multiple_of(2),
            Self::RowIndexLt(threshold) => row < threshold,
        }
    }
}

impl BatchConditionEval for MockCondition {
    // Use default implementation that calls eval_row in a loop.
}

// ========================================================================
// Variant Coverage Helpers
// ========================================================================

/// Returns a list of all mock condition variants for coverage checks.
#[must_use]
pub fn all_variants() -> Vec<MockCondition> {
    vec![
        MockCondition::AlwaysTrue,
        MockCondition::AlwaysFalse,
        MockCondition::ValueGte(100),
        MockCondition::ValueLte(-50),
        MockCondition::ValueEq(0),
        MockCondition::HasAllFlags(0xDEAD_BEEF),
        MockCondition::HasAnyFlags(0b10101),
        MockCondition::HasNoneFlags(0xFF00),
        MockCondition::RowIndexEven,
        MockCondition::RowIndexLt(1000),
    ]
}
