// ret-logic/src/executor.rs
// ============================================================================
// Module: Requirement Executor
// Description: Prepared plan execution infrastructure for requirements.
// Purpose: Run compiled plans using domain dispatch tables and shared helpers.
// Dependencies: crate::requirement::{error, plan, traits}
// ============================================================================

//! ## Overview
//! Executes compiled requirement plans with a reusable stack machine while
//! exposing helpers for dispatch table construction and optimized operation
//! implementations. Domains implement `PredicateEval` for `PlanExecutor` via their
//! reader types.

// ============================================================================
// SECTION: Imports
// ============================================================================

use super::error::RequirementError;
use super::error::RequirementResult;
use super::plan::Constant;
use super::plan::OpCode;
use super::plan::Operation;
use super::plan::Plan;
use super::traits::BatchPredicateEval;
use super::traits::PredicateEval;
use super::traits::Row;

// ============================================================================
// SECTION: Type Aliases
// ============================================================================

/// Type alias for evaluation function pointers used in the dispatch table.
/// Maps an opcode to a function that evaluates a row against an operation with constants.
type EvalFn<R> = fn(&R, Row, Operation, &[Constant]) -> RequirementResult<bool>;

/// Type alias for the complete evaluation dispatch table.
/// Contains an optional function pointer per opcode value.
type EvalTable<R> = [Option<EvalFn<R>>; 256];

// ============================================================================
// SECTION: Internal Combine Mode
// ============================================================================

/// Boolean combine mode for plan stack execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CombineMode {
    /// Logical AND with short-circuit identity of true.
    And,
    /// Logical OR with short-circuit identity of false.
    Or,
}

impl CombineMode {
    /// Returns the identity value for the combine operator.
    const fn identity(self) -> bool {
        match self {
            Self::And => true,
            Self::Or => false,
        }
    }

    /// Combines two boolean values using the configured operator.
    const fn combine(self, lhs: bool, rhs: bool) -> bool {
        match self {
            Self::And => lhs && rhs,
            Self::Or => lhs || rhs,
        }
    }
}

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum stack depth allowed during plan execution.
const MAX_PLAN_STACK_DEPTH: usize = 64;

// ============================================================================
// SECTION: Plan Executor
// ============================================================================

/// Generic plan executor that implements evaluation traits over any Reader type
///
/// This structure bridges the gap between compiled plans and the evaluation trait system.
/// Domains provide a dispatch table that maps opcodes to evaluation functions over their
/// specific reader types.
pub struct PlanExecutor<R: 'static> {
    /// The compiled plan to execute
    pub plan: Plan,

    /// Dispatch table mapping opcodes to evaluation functions
    /// Index by `OpCode` as u8, contains function pointers for row evaluation
    pub eval_table: EvalTable<R>,
}

// ============================================================================
// SECTION: Plan Executor Methods
// ============================================================================

impl<R: 'static> PlanExecutor<R> {
    /// Creates a new plan executor with the given plan and dispatch table
    ///
    /// # Arguments
    /// * `plan` - The compiled plan to execute
    /// * `eval_table` - Dispatch table for opcode evaluation
    ///
    /// # Returns
    /// A new plan executor ready for evaluation
    #[must_use]
    pub fn new(plan: Plan, eval_table: EvalTable<R>) -> Self {
        Self {
            plan,
            eval_table,
        }
    }

    /// Returns a reference to the underlying plan
    #[must_use]
    pub const fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Returns the required columns for this executor's plan
    #[must_use]
    pub fn required_columns(&self) -> &[super::plan::ColumnKey] {
        self.plan.required_columns()
    }
}

// ============================================================================
// SECTION: Predicate Evaluation
// ============================================================================

impl<R: 'static> PredicateEval for PlanExecutor<R> {
    type Reader<'a> = R;

    fn eval_row(&self, reader: &Self::Reader<'_>, row: Row) -> bool {
        // Use a small stack for boolean logic evaluation
        let mut stack_values: [bool; MAX_PLAN_STACK_DEPTH] = [false; MAX_PLAN_STACK_DEPTH];
        let mut stack_modes: [CombineMode; MAX_PLAN_STACK_DEPTH] =
            [CombineMode::And; MAX_PLAN_STACK_DEPTH];
        let mut stack_pointer = 0usize;

        stack_modes[0] = CombineMode::And;
        stack_values[0] = CombineMode::And.identity();

        // Execute operations in sequence
        for operation in self.plan.operations() {
            match operation.opcode {
                OpCode::AndStart => {
                    // Push a new AND context
                    stack_pointer += 1;
                    if stack_pointer >= stack_values.len() {
                        // Stack overflow protection - treat as false
                        return false;
                    }
                    stack_modes[stack_pointer] = CombineMode::And;
                    stack_values[stack_pointer] = CombineMode::And.identity();
                }

                OpCode::AndEnd => {
                    // Pop AND context and combine with parent
                    if stack_pointer == 0 || stack_modes[stack_pointer] != CombineMode::And {
                        // Malformed plan - no matching start
                        return false;
                    }
                    let and_result = stack_values[stack_pointer];
                    stack_pointer -= 1;
                    stack_values[stack_pointer] =
                        stack_modes[stack_pointer].combine(stack_values[stack_pointer], and_result);
                }

                OpCode::OrStart => {
                    // Push a new OR context
                    stack_pointer += 1;
                    if stack_pointer >= stack_values.len() {
                        // Stack overflow protection - treat as false
                        return false;
                    }
                    stack_modes[stack_pointer] = CombineMode::Or;
                    stack_values[stack_pointer] = CombineMode::Or.identity();
                }

                OpCode::OrEnd => {
                    // Pop OR context and combine with parent
                    if stack_pointer == 0 || stack_modes[stack_pointer] != CombineMode::Or {
                        // Malformed plan - no matching start
                        return false;
                    }
                    let or_result = stack_values[stack_pointer];
                    stack_pointer -= 1;
                    stack_values[stack_pointer] =
                        stack_modes[stack_pointer].combine(stack_values[stack_pointer], or_result);
                }

                OpCode::Not => {
                    // NOT operation inverts the current context
                    stack_values[stack_pointer] = !stack_values[stack_pointer];
                }

                _ => {
                    // Domain-specific operation - delegate to dispatch table
                    if let Some(eval_fn) = self.eval_table[operation.opcode as u8 as usize] {
                        match eval_fn(reader, row, *operation, &self.plan.constants) {
                            Ok(result) => {
                                stack_values[stack_pointer] = stack_modes[stack_pointer]
                                    .combine(stack_values[stack_pointer], result);
                            }
                            Err(_) => {
                                // Evaluation error - treat as false and keep fail-closed semantics
                                stack_values[stack_pointer] = stack_modes[stack_pointer]
                                    .combine(stack_values[stack_pointer], false);
                            }
                        }
                    } else {
                        // Missing handler - treat as false and keep fail-closed semantics
                        stack_values[stack_pointer] =
                            stack_modes[stack_pointer].combine(stack_values[stack_pointer], false);
                    }
                }
            }
        }

        // Return the final result
        stack_values[0]
    }
}

// ============================================================================
// SECTION: Batch Evaluation
// ============================================================================

impl<R: 'static> BatchPredicateEval for PlanExecutor<R> {
    // Use default implementation that calls eval_row in a loop
    // Domains can create specialized batch executors if they need SIMD optimization
}

/// Helper trait to create no-op dispatch tables
// ============================================================================
// SECTION: Dispatch Table Builder
// ============================================================================
pub trait DispatchTableBuilder<R> {
    /// Creates a dispatch table initialized with no-op functions
    #[must_use]
    fn build_dispatch_table() -> EvalTable<R> {
        [None; 256]
    }
}

/// Macro to help domains build dispatch tables
// ============================================================================
// SECTION: Dispatch Macro
// ============================================================================
#[macro_export]
macro_rules! build_dispatch_table {
    ($reader_type:ty, $($opcode:path => $handler:path),* $(,)?) => {{
        let mut table: [Option<fn(&$reader_type, $crate::Row, $crate::Operation, &[$crate::Constant]) -> $crate::RequirementResult<bool>>; 256] =
            [None; 256];

        $(
            table[$opcode as u8 as usize] = Some($handler);
        )*

        table
    }};
}

// ============================================================================
// SECTION: Executor Builder
// ============================================================================

/// Builder for creating plan executors with domain-specific dispatch tables
pub struct ExecutorBuilder<R: 'static> {
    /// Dispatch table used by the executor builder.
    eval_table: EvalTable<R>,
}

// ============================================================================
// SECTION: Executor Builder Methods
// ============================================================================

impl<R: 'static> ExecutorBuilder<R> {
    /// Creates a new executor builder with all opcodes initialized to no-op
    #[must_use]
    pub fn new() -> Self {
        Self {
            eval_table: [None; 256],
        }
    }

    /// Registers a handler for a specific opcode
    #[must_use]
    pub fn register(
        mut self,
        opcode: OpCode,
        handler: fn(&R, Row, Operation, &[Constant]) -> RequirementResult<bool>,
    ) -> Self {
        self.eval_table[opcode as u8 as usize] = Some(handler);
        self
    }

    /// Builds a plan executor with the given plan
    #[must_use]
    pub fn build(self, plan: Plan) -> PlanExecutor<R> {
        // Create a static dispatch table - in practice domains should define these as statics
        PlanExecutor::new(plan, self.eval_table)
    }
}

// ============================================================================
// SECTION: Executor Builder Defaults
// ============================================================================

impl<R: 'static> Default for ExecutorBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SECTION: Operation Helpers
// ============================================================================

/// Helper functions for common operation implementations
pub mod operations {
    use super::Constant;
    use super::Operation;
    use super::RequirementError;
    use super::RequirementResult;
    use super::Row;

    /// Standard float comparison implementation
    ///
    /// # Errors
    /// Returns [`RequirementError`] when operands or constants are missing.
    pub fn float_gte<R, F>(
        reader: &R,
        row: Row,
        op: &Operation,
        constants: &[Constant],
        value_getter: F,
    ) -> RequirementResult<bool>
    where
        F: Fn(&R, Row, u16) -> Option<f32>,
    {
        let column_id = op.operand_a;
        let constant_idx = op.operand_b as usize;

        let value = value_getter(reader, row, column_id)
            .ok_or_else(|| RequirementError::predicate_error("Missing value for comparison"))?;

        let threshold = constants
            .get(constant_idx)
            .and_then(super::super::plan::Constant::as_float)
            .ok_or_else(|| RequirementError::predicate_error("Invalid threshold constant"))?;

        Ok(value >= threshold)
    }

    /// Standard flags check implementation
    ///
    /// # Errors
    /// Returns [`RequirementError`] when operands or constants are missing.
    pub fn has_all_flags<R, F>(
        reader: &R,
        row: Row,
        op: &Operation,
        constants: &[Constant],
        flags_getter: F,
    ) -> RequirementResult<bool>
    where
        F: Fn(&R, Row, u16) -> Option<u64>,
    {
        let column_id = op.operand_a;
        let constant_idx = op.operand_b as usize;

        let entity_flags = flags_getter(reader, row, column_id)
            .ok_or_else(|| RequirementError::predicate_error("Missing flags for check"))?;

        let required_flags = constants
            .get(constant_idx)
            .and_then(super::super::plan::Constant::as_flags)
            .ok_or_else(|| RequirementError::predicate_error("Invalid flags constant"))?;

        Ok((entity_flags & required_flags) == required_flags)
    }
}
