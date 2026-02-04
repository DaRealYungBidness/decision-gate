// ret-logic/src/traits.rs
// ============================================================================
// Module: Requirement Traits
// Description: Row-based evaluation contracts for requirement executors.
// Purpose: Define condition, batch, and reader utilities for ECS chunk processing.
// Dependencies: crate::tristate, std
// ============================================================================

//! ## Overview
//! Row-based contracts describe how conditions evaluate against chunk readers and
//! provide helpers for mask-based batch evaluation and row iteration.

// ============================================================================
// SECTION: Imports
// ============================================================================

use crate::tristate::TriState;

// ============================================================================
// SECTION: Type Aliases
// ============================================================================

/// Row index within a chunk
pub type Row = usize;

/// 64-bit mask for batch evaluation results
pub type Mask64 = u64;

// ============================================================================
// SECTION: Condition Trait
// ============================================================================

/// Core trait for condition evaluation over chunk readers
///
/// Conditions evaluate against a specific row within a reader that contains
/// component slices from an ECS chunk. This design enables:
///
/// - Direct slice access (no hash lookups)
/// - Cache-friendly memory access patterns
/// - SIMD optimization opportunities
/// - Zero allocation in hot paths
pub trait ConditionEval {
    /// Domain-specific reader type containing component slices
    ///
    /// Examples: `ShipReader`<'a>, `ActorReader`<'a>, `NeuronReader`<'a>
    /// Each contains component slices needed for evaluation.
    type Reader<'a>;

    /// Evaluate condition for a specific row within the reader
    ///
    /// This is the core hot path method. It should:
    /// - Access component data via direct array indexing: `reader.health[row]`
    /// - Perform simple comparisons and bitwise operations
    /// - Be marked #[inline(always)] for maximum optimization
    ///
    /// # Arguments
    /// * `reader` - Bundle of component slices from an ECS chunk
    /// * `row` - Index within the chunk (`0..chunk_len`)
    ///
    /// # Returns
    /// `true` if the condition is satisfied for this row
    fn eval_row(&self, reader: &Self::Reader<'_>, row: Row) -> bool;
}

// ============================================================================
// SECTION: Batch Condition Trait
// ============================================================================

/// Batch evaluation trait for vectorized processing
///
/// Provides default window-based evaluation and allows domains to override
/// with SIMD implementations for maximum performance.
pub trait BatchConditionEval: ConditionEval {
    /// Evaluate condition for up to 64 consecutive rows
    ///
    /// Returns a bitmask where bit N indicates whether row start+N passed.
    /// Default implementation calls [`ConditionEval::eval_row`] in a loop. Domains can override
    /// with SIMD intrinsics for vectorized evaluation.
    ///
    /// # Arguments
    /// * `reader` - Bundle of component slices
    /// * `start` - Starting row index
    /// * `count` - Number of rows to evaluate (clamped to 64)
    ///
    /// # Returns
    /// Bitmask where bit N set means row start+N satisfied the condition
    #[inline]
    fn eval_block(&self, reader: &Self::Reader<'_>, start: Row, count: usize) -> Mask64 {
        let n = count.min(64);
        let mut mask: Mask64 = 0;

        for i in 0 .. n {
            if self.eval_row(reader, start + i) {
                mask |= 1u64 << i;
            }
        }

        mask
    }
}

// ============================================================================
// SECTION: Tri-State Condition Trait
// ============================================================================
/// Condition evaluation that can return `Unknown` for insufficient evidence
pub trait TriStateConditionEval {
    /// Domain-specific reader type containing component slices or evidence data
    type Reader<'a>;

    /// Evaluate condition for a specific row within the reader
    ///
    /// Returns `TriState::Unknown` when evidence is missing or indeterminate.
    fn eval_row_tristate(&self, reader: &Self::Reader<'_>, row: Row) -> TriState;
}

/// Adapter for boolean conditions that should participate in tri-state evaluation
///
/// # Invariants
/// - Holds a condition value of type `P` with no additional constraints.
#[derive(Debug, Clone, Copy)]
pub struct BoolAsTri<P>(pub P);

impl<P> BoolAsTri<P> {
    /// Wraps a boolean condition for tri-state evaluation
    pub const fn new(condition: P) -> Self {
        Self(condition)
    }
}

impl<P: ConditionEval> TriStateConditionEval for BoolAsTri<P> {
    type Reader<'a> = P::Reader<'a>;

    fn eval_row_tristate(&self, reader: &Self::Reader<'_>, row: Row) -> TriState {
        self.0.eval_row(reader, row).into()
    }
}

// ============================================================================
// SECTION: Reader Length Trait
// ============================================================================

/// Trait for readers to expose their length
///
/// All readers must implement this so generic evaluation code
/// can determine chunk boundaries without knowing the specific reader type.
pub trait ReaderLen {
    /// Returns the number of entities/rows in this chunk reader
    fn len(&self) -> usize;

    /// Returns whether the reader is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// SECTION: Batch Evaluation Helpers
// ============================================================================

/// Helper function to evaluate an entire reader and collect passing row indices
///
/// Most domains will drive evaluation themselves to collect Entity IDs instead
/// of row indices, but this provides a generic implementation for testing.
#[inline]
pub fn eval_reader_rows<P>(condition: &P, reader: &P::Reader<'_>) -> Vec<Row>
where
    P: BatchConditionEval,
    for<'a> P::Reader<'a>: ReaderLen,
{
    let mut passing_rows = Vec::new();
    let total_len = reader.len();
    let mut row = 0;

    while row < total_len {
        let count = (total_len - row).min(64);
        let mask = condition.eval_block(reader, row, count);

        // Extract set bits from mask
        for i in 0 .. count {
            if (mask >> i) & 1 == 1 {
                passing_rows.push(row + i);
            }
        }

        row += count;
    }

    passing_rows
}
