// ret-logic/tests/executor.rs
// ============================================================================
// Module: Executor Tests
// Description: Tests for PlanExecutor and execution machinery.
// ============================================================================
//! ## Overview
//! Integration tests for the requirement plan executor covering planning and execution paths.

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

mod support;

use ret_logic::ColumnKey;
use ret_logic::Constant;
use ret_logic::OpCode;
use ret_logic::Operation;
use ret_logic::Plan;
use ret_logic::PlanBuilder;
use ret_logic::PredicateEval;
use ret_logic::RequirementError;
use ret_logic::RequirementResult;
use ret_logic::Row;
use ret_logic::executor::ExecutorBuilder;
use ret_logic::executor::PlanExecutor;
use ret_logic::executor::operations;
use support::TestResult;
use support::ensure;

// ============================================================================
// SECTION: Test Reader Type
// ============================================================================

/// Simple test reader for executor tests
struct TestReader {
    values: Vec<f32>,
    flags: Vec<u64>,
}

impl TestReader {
    const fn new(values: Vec<f32>, flags: Vec<u64>) -> Self {
        Self {
            values,
            flags,
        }
    }

    fn get_value(&self, row: usize) -> Option<f32> {
        self.values.get(row).copied()
    }

    fn get_flags(&self, row: usize) -> Option<u64> {
        self.flags.get(row).copied()
    }
}

// ============================================================================
// SECTION: Dispatch Table Handlers
// ============================================================================

fn require_constant(op: Operation, constants: &[Constant]) -> RequirementResult<()> {
    constants
        .get(usize::from(op.operand_b))
        .ok_or_else(|| RequirementError::predicate_error("Missing constant"))?;
    Ok(())
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "dispatch table signature requires &Operation"
)]
fn handle_float_gte(
    reader: &TestReader,
    row: Row,
    op: Operation,
    constants: &[Constant],
) -> RequirementResult<bool> {
    operations::float_gte(reader, row, &op, constants, |r, row, _col| r.get_value(row))
}

fn handle_float_lte(
    reader: &TestReader,
    row: Row,
    op: Operation,
    constants: &[Constant],
) -> RequirementResult<bool> {
    let value =
        reader.get_value(row).ok_or_else(|| RequirementError::predicate_error("Missing value"))?;
    let threshold = constants
        .get(usize::from(op.operand_b))
        .and_then(ret_logic::Constant::as_float)
        .ok_or_else(|| RequirementError::predicate_error("Invalid threshold"))?;
    Ok(value <= threshold)
}

fn handle_has_all_flags(
    reader: &TestReader,
    row: Row,
    op: Operation,
    constants: &[Constant],
) -> RequirementResult<bool> {
    operations::has_all_flags(reader, row, &op, constants, |r, row, _col| r.get_flags(row))
}

fn handle_has_any_flags(
    reader: &TestReader,
    row: Row,
    op: Operation,
    constants: &[Constant],
) -> RequirementResult<bool> {
    let flags =
        reader.get_flags(row).ok_or_else(|| RequirementError::predicate_error("Missing flags"))?;
    let test = constants
        .get(usize::from(op.operand_b))
        .and_then(ret_logic::Constant::as_flags)
        .ok_or_else(|| RequirementError::predicate_error("Invalid flags"))?;
    Ok((flags & test) != 0)
}

fn handle_always_true(
    _reader: &TestReader,
    _row: Row,
    op: Operation,
    constants: &[Constant],
) -> RequirementResult<bool> {
    require_constant(op, constants)?;
    Ok(true)
}

fn handle_always_false(
    _reader: &TestReader,
    _row: Row,
    op: Operation,
    constants: &[Constant],
) -> RequirementResult<bool> {
    require_constant(op, constants)?;
    Ok(false)
}

// ============================================================================
// SECTION: Static Dispatch Table
// ============================================================================

/// Function pointer signature for executor dispatch handlers.
type DispatchHandler = fn(&TestReader, Row, Operation, &[Constant]) -> RequirementResult<bool>;

/// Returns the opcode table index for a known opcode variant.
const fn opcode_index(opcode: OpCode) -> usize {
    match opcode {
        OpCode::AndStart => 0,
        OpCode::AndEnd => 1,
        OpCode::OrStart => 2,
        OpCode::OrEnd => 3,
        OpCode::Not => 4,
        OpCode::FloatGte => 10,
        OpCode::FloatLte => 11,
        OpCode::FloatEq => 12,
        OpCode::IntGte => 13,
        OpCode::IntLte => 14,
        OpCode::IntEq => 15,
        OpCode::HasAllFlags => 20,
        OpCode::HasAnyFlags => 21,
        OpCode::HasNoneFlags => 22,
        OpCode::InRange => 30,
        OpCode::InRegion => 31,
        OpCode::DomainStart => 100,
    }
}

static TEST_DISPATCH_TABLE: [Option<DispatchHandler>; 256] = {
    let mut table: [Option<DispatchHandler>; 256] = [None; 256];
    table[opcode_index(OpCode::FloatGte)] = Some(handle_float_gte);
    table[opcode_index(OpCode::FloatLte)] = Some(handle_float_lte);
    table[opcode_index(OpCode::HasAllFlags)] = Some(handle_has_all_flags);
    table[opcode_index(OpCode::HasAnyFlags)] = Some(handle_has_any_flags);
    // Custom opcodes for testing
    table[100] = Some(handle_always_true);
    table[101] = Some(handle_always_false);
    table
};

// ============================================================================
// SECTION: PlanExecutor Creation Tests
// ============================================================================

/// Tests plan executor new.
#[test]
fn test_plan_executor_new() -> TestResult {
    let plan = Plan::new();
    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    ensure(executor.plan().operations().is_empty(), "Expected empty plan operations")?;
    ensure(executor.required_columns().is_empty(), "Expected empty required columns")?;
    Ok(())
}

/// Tests plan executor with plan.
#[test]
fn test_plan_executor_with_plan() -> TestResult {
    let mut builder = PlanBuilder::new();
    builder.add_float_constant(50.0)?;

    let plan = builder.require_column(ColumnKey::new(0)).add_op(OpCode::FloatGte, 0, 0, 0).build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    ensure(executor.required_columns().len() == 1, "Expected one required column")?;
    Ok(())
}

// ============================================================================
// SECTION: Simple Execution Tests
// ============================================================================

/// Tests executor empty plan.
#[test]
fn test_executor_empty_plan() -> TestResult {
    let plan = Plan::new();
    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![100.0], vec![0]);

    // Empty plan should return true (default stack value)
    ensure(executor.eval_row(&reader, 0), "Expected empty plan to evaluate true")?;
    Ok(())
}

/// Tests executor single operation.
#[test]
fn test_executor_single_operation() -> TestResult {
    let mut builder = PlanBuilder::new();
    let threshold_idx = builder.add_float_constant(50.0)?;

    let plan = builder.add_op(OpCode::FloatGte, 0, threshold_idx.0, 0).build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![25.0, 50.0, 75.0], vec![0, 0, 0]);

    ensure(!executor.eval_row(&reader, 0), "Expected 25 < 50 to fail")?;
    ensure(executor.eval_row(&reader, 1), "Expected 50 >= 50 to pass")?;
    ensure(executor.eval_row(&reader, 2), "Expected 75 >= 50 to pass")?;
    Ok(())
}

// ============================================================================
// SECTION: AND Group Tests
// ============================================================================

/// Tests executor and group all true.
#[test]
fn test_executor_and_group_all_true() -> TestResult {
    let mut builder = PlanBuilder::new();
    let low_idx = builder.add_float_constant(10.0)?;
    let high_idx = builder.add_float_constant(90.0)?;

    let plan = builder
        .and_start()
        .add_op(OpCode::FloatGte, 0, low_idx.0, 0)
        .add_op(OpCode::FloatLte, 0, high_idx.0, 0)
        .and_end()
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![50.0], vec![0]);

    ensure(executor.eval_row(&reader, 0), "Expected AND group to pass")?;
    Ok(())
}

/// Tests executor and group one false.
#[test]
fn test_executor_and_group_one_false() -> TestResult {
    let mut builder = PlanBuilder::new();
    let low_idx = builder.add_float_constant(10.0)?;
    let high_idx = builder.add_float_constant(40.0)?;

    let plan = builder
        .and_start()
        .add_op(OpCode::FloatGte, 0, low_idx.0, 0)
        .add_op(OpCode::FloatLte, 0, high_idx.0, 0)
        .and_end()
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![50.0], vec![0]);

    ensure(!executor.eval_row(&reader, 0), "Expected AND group to fail")?;
    Ok(())
}

/// Tests executor empty and.
#[test]
fn test_executor_empty_and() -> TestResult {
    let plan = PlanBuilder::new().and_start().and_end().build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![0.0], vec![0]);

    // Empty AND should be true
    ensure(executor.eval_row(&reader, 0), "Expected empty AND group to evaluate true")?;
    Ok(())
}

// ============================================================================
// SECTION: OR Group Tests
// ============================================================================

/// Tests executor or group one true.
#[test]
fn test_executor_or_group_one_true() -> TestResult {
    let mut builder = PlanBuilder::new();
    let low_idx = builder.add_float_constant(100.0)?;
    let high_idx = builder.add_float_constant(0.0)?;

    let plan = builder
        .or_start()
        .add_op(OpCode::FloatGte, 0, low_idx.0, 0) // 50 >= 100: false
        .add_op(OpCode::FloatGte, 0, high_idx.0, 0) // 50 >= 0: true
        .or_end()
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![50.0], vec![0]);

    ensure(executor.eval_row(&reader, 0), "Expected OR group to evaluate true")?;
    Ok(())
}

/// Tests executor empty or.
#[test]
fn test_executor_empty_or() -> TestResult {
    let plan = PlanBuilder::new().or_start().or_end().build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![0.0], vec![0]);

    // Empty OR should be false (identity for OR)
    ensure(!executor.eval_row(&reader, 0), "Expected empty OR group to evaluate false")?;
    Ok(())
}

// ============================================================================
// SECTION: NOT Operation Tests
// ============================================================================

/// Tests executor not true.
#[test]
fn test_executor_not_true() -> TestResult {
    let plan = PlanBuilder::new()
        .add_op(OpCode::Not, 0, 0, 0) // Invert true -> false
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![0.0], vec![0]);

    ensure(!executor.eval_row(&reader, 0), "Expected NOT true to evaluate false")?;
    Ok(())
}

/// Tests executor not with operation.
#[test]
fn test_executor_not_with_operation() -> TestResult {
    let mut builder = PlanBuilder::new();
    let threshold_idx = builder.add_float_constant(50.0)?;

    let plan = builder
        .add_op(OpCode::FloatGte, 0, threshold_idx.0, 0)
        .add_op(OpCode::Not, 0, 0, 0)
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![25.0, 75.0], vec![0, 0]);

    ensure(executor.eval_row(&reader, 0), "Expected NOT false to evaluate true")?;
    ensure(!executor.eval_row(&reader, 1), "Expected NOT true to evaluate false")?;
    Ok(())
}

// ============================================================================
// SECTION: Nested Group Tests
// ============================================================================

/// Tests executor nested and in or.
#[test]
fn test_executor_nested_and_in_or() -> TestResult {
    let mut builder = PlanBuilder::new();
    let val1 = builder.add_float_constant(100.0)?;
    let val2 = builder.add_float_constant(0.0)?;

    let plan = builder
        .or_start()
        .and_start()
        .add_op(OpCode::FloatGte, 0, val1.0, 0) // false
        .and_end()
        .and_start()
        .add_op(OpCode::FloatGte, 0, val2.0, 0) // true
        .and_end()
        .or_end()
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![50.0], vec![0]);

    ensure(executor.eval_row(&reader, 0), "Expected nested AND in OR to evaluate true")?;
    Ok(())
}

// ============================================================================
// SECTION: Flag Operation Tests
// ============================================================================

/// Tests executor has all flags.
#[test]
fn test_executor_has_all_flags() -> TestResult {
    let mut builder = PlanBuilder::new();
    let flags_idx = builder.add_flags_constant(0b11)?;

    let plan = builder.add_op(OpCode::HasAllFlags, 1, flags_idx.0, 0).build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);

    let reader1 = TestReader::new(vec![0.0], vec![0b00]);
    let reader2 = TestReader::new(vec![0.0], vec![0b01]);
    let reader3 = TestReader::new(vec![0.0], vec![0b11]);
    let reader4 = TestReader::new(vec![0.0], vec![0b111]);

    ensure(!executor.eval_row(&reader1, 0), "Expected no flags to fail HasAllFlags")?;
    ensure(!executor.eval_row(&reader2, 0), "Expected missing bit to fail HasAllFlags")?;
    ensure(executor.eval_row(&reader3, 0), "Expected required flags to pass HasAllFlags")?;
    ensure(executor.eval_row(&reader4, 0), "Expected extra flags to still pass HasAllFlags")?;
    Ok(())
}

/// Tests executor has any flags.
#[test]
fn test_executor_has_any_flags() -> TestResult {
    let mut builder = PlanBuilder::new();
    let flags_idx = builder.add_flags_constant(0b11)?;

    let plan = builder.add_op(OpCode::HasAnyFlags, 1, flags_idx.0, 0).build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);

    let reader1 = TestReader::new(vec![0.0], vec![0b00]);
    let reader2 = TestReader::new(vec![0.0], vec![0b01]);
    let reader3 = TestReader::new(vec![0.0], vec![0b100]);

    ensure(!executor.eval_row(&reader1, 0), "Expected no flags to fail HasAnyFlags")?;
    ensure(executor.eval_row(&reader2, 0), "Expected matching bit to pass HasAnyFlags")?;
    ensure(!executor.eval_row(&reader3, 0), "Expected non-matching bit to fail HasAnyFlags")?;
    Ok(())
}

// ============================================================================
// SECTION: Multiple Row Tests
// ============================================================================

/// Tests executor multiple rows.
#[test]
fn test_executor_multiple_rows() -> TestResult {
    let mut builder = PlanBuilder::new();
    let threshold_idx = builder.add_float_constant(50.0)?;

    let plan = builder.add_op(OpCode::FloatGte, 0, threshold_idx.0, 0).build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![0.0, 25.0, 50.0, 75.0, 100.0], vec![0, 0, 0, 0, 0]);

    ensure(!executor.eval_row(&reader, 0), "Expected 0.0 to fail threshold")?;
    ensure(!executor.eval_row(&reader, 1), "Expected 25.0 to fail threshold")?;
    ensure(executor.eval_row(&reader, 2), "Expected 50.0 to pass threshold")?;
    ensure(executor.eval_row(&reader, 3), "Expected 75.0 to pass threshold")?;
    ensure(executor.eval_row(&reader, 4), "Expected 100.0 to pass threshold")?;
    Ok(())
}

// ============================================================================
// SECTION: ExecutorBuilder Tests
// ============================================================================

/// Tests executor builder new.
#[test]
fn test_executor_builder_new() {
    let builder = ExecutorBuilder::<TestReader>::new();
    let plan = Plan::new();
    let _executor = builder.build(plan);
    // Just verify it compiles and doesn't panic
}

/// Tests executor builder default.
#[test]
fn test_executor_builder_default() {
    let builder = ExecutorBuilder::<TestReader>::default();
    let plan = Plan::new();
    let _executor = builder.build(plan);
}

/// Tests executor builder register.
#[test]
fn test_executor_builder_register() -> TestResult {
    let mut plan_builder = PlanBuilder::new();
    let threshold = plan_builder.add_float_constant(50.0)?;

    let plan = plan_builder.add_op(OpCode::FloatGte, 0, threshold.0, 0).build();

    let executor = ExecutorBuilder::<TestReader>::new()
        .register(OpCode::FloatGte, handle_float_gte)
        .build(plan);

    let reader = TestReader::new(vec![75.0], vec![0]);
    ensure(executor.eval_row(&reader, 0), "Expected registered handler to evaluate true")?;
    Ok(())
}

// ============================================================================
// SECTION: Error Handling Tests
// ============================================================================

/// Tests executor stack overflow protection.
#[test]
fn test_executor_stack_overflow_protection() -> TestResult {
    // Create a plan that would overflow the stack depth.
    let mut builder = PlanBuilder::new();

    // Push more AND starts than the executor stack supports.
    for _ in 0 .. 70 {
        builder = builder.and_start();
    }

    let plan = builder.build();
    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![0.0], vec![0]);

    // Should return false due to stack overflow protection
    ensure(!executor.eval_row(&reader, 0), "Expected stack overflow protection to fail")?;
    Ok(())
}

/// Tests executor malformed plan unmatched end.
#[test]
fn test_executor_malformed_plan_unmatched_end() -> TestResult {
    let plan = PlanBuilder::new()
        .and_end() // Unmatched end
        .build();

    let executor = PlanExecutor::new(plan, TEST_DISPATCH_TABLE);
    let reader = TestReader::new(vec![0.0], vec![0]);

    // Should return false due to malformed plan
    ensure(!executor.eval_row(&reader, 0), "Expected malformed plan to fail")?;
    Ok(())
}

// ============================================================================
// SECTION: Operation Helpers Tests
// ============================================================================

/// Tests operations float gte helper.
#[test]
fn test_operations_float_gte_helper() -> TestResult {
    let constants = vec![Constant::Float(50.0)];
    let op = Operation::new(OpCode::FloatGte, 0, 0, 0);
    let reader = TestReader::new(vec![75.0], vec![0]);

    let result = operations::float_gte(&reader, 0, &op, &constants, |r, row, _| r.get_value(row));

    ensure(result?, "Expected float_gte helper to return true")?;
    Ok(())
}

/// Tests operations has all flags helper.
#[test]
fn test_operations_has_all_flags_helper() -> TestResult {
    let constants = vec![Constant::Flags(0b11)];
    let op = Operation::new(OpCode::HasAllFlags, 0, 0, 0);
    let reader = TestReader::new(vec![0.0], vec![0b11]);

    let result =
        operations::has_all_flags(&reader, 0, &op, &constants, |r, row, _| r.get_flags(row));

    ensure(result?, "Expected has_all_flags helper to return true")?;
    Ok(())
}

/// Tests operations missing value.
#[test]
fn test_operations_missing_value() -> TestResult {
    let constants = vec![Constant::Float(50.0)];
    let op = Operation::new(OpCode::FloatGte, 0, 0, 0);
    let reader = TestReader::new(vec![], vec![]);

    let result = operations::float_gte(&reader, 0, &op, &constants, |r, row, _| r.get_value(row));

    ensure(result.is_err(), "Expected missing value to return error")?;
    Ok(())
}

/// Tests operations invalid constant.
#[test]
fn test_operations_invalid_constant() -> TestResult {
    let constants = vec![]; // No constants
    let op = Operation::new(OpCode::FloatGte, 0, 0, 0);
    let reader = TestReader::new(vec![75.0], vec![0]);

    let result = operations::float_gte(&reader, 0, &op, &constants, |r, row, _| r.get_value(row));

    ensure(result.is_err(), "Expected invalid constant to return error")?;
    Ok(())
}
