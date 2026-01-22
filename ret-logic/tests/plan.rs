// ret-logic/tests/plan.rs
// ============================================================================
// Module: Plan Tests
// Description: Tests for Plan, PlanBuilder, Operation, OpCode, Constant.
// ============================================================================
//! ## Overview
//! Integration tests for plan compilation primitives and supporting types.

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
use ret_logic::ConstantIndex;
use ret_logic::OpCode;
use ret_logic::Operation;
use ret_logic::Plan;
use ret_logic::PlanBuilder;
use support::TestResult;
use support::ensure;

const SAMPLE_FLOAT: f32 = std::f32::consts::PI;

// ========================================================================
// Test Helpers
// ========================================================================

/// Returns the canonical byte value for an opcode variant.
const fn opcode_value(opcode: OpCode) -> u8 {
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

// ============================================================================
// SECTION: ColumnKey Tests
// ============================================================================

/// Tests column key new.
#[test]
fn test_column_key_new() -> TestResult {
    let key = ColumnKey::new(42);
    ensure(key.id() == 42, "Expected ColumnKey id to match constructor")?;
    Ok(())
}

/// Tests column key value.
#[test]
fn test_column_key_value() -> TestResult {
    let key = ColumnKey(100);
    ensure(key.0 == 100, "Expected ColumnKey tuple field to match")?;
    ensure(key.id() == 100, "Expected ColumnKey id to match")?;
    Ok(())
}

/// Tests column key equality.
#[test]
fn test_column_key_equality() -> TestResult {
    let key1 = ColumnKey::new(1);
    let key2 = ColumnKey::new(1);
    let key3 = ColumnKey::new(2);

    ensure(key1 == key2, "Expected identical ColumnKeys to compare equal")?;
    ensure(key1 != key3, "Expected distinct ColumnKeys to compare not equal")?;
    Ok(())
}

/// Tests column key hash.
#[test]
fn test_column_key_hash() -> TestResult {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ColumnKey::new(1));
    set.insert(ColumnKey::new(2));
    set.insert(ColumnKey::new(1)); // Duplicate

    ensure(set.len() == 2, "Expected hash set to de-duplicate keys")?;
    Ok(())
}

// ============================================================================
// SECTION: ConstantIndex Tests
// ============================================================================

/// Tests constant index value.
#[test]
fn test_constant_index_value() -> TestResult {
    let idx = ConstantIndex(42);
    ensure(idx.0 == 42, "Expected ConstantIndex tuple field to match")?;
    Ok(())
}

/// Tests constant index equality.
#[test]
fn test_constant_index_equality() -> TestResult {
    let idx1 = ConstantIndex(10);
    let idx2 = ConstantIndex(10);
    let idx3 = ConstantIndex(20);

    ensure(idx1 == idx2, "Expected identical ConstantIndex values to compare equal")?;
    ensure(idx1 != idx3, "Expected distinct ConstantIndex values to compare not equal")?;
    Ok(())
}

// ============================================================================
// SECTION: Operation Tests
// ============================================================================

/// Tests operation new.
#[test]
fn test_operation_new() -> TestResult {
    let op = Operation::new(OpCode::FloatGte, 1, 2, 3);
    ensure(op.opcode == OpCode::FloatGte, "Expected opcode to match constructor")?;
    ensure(op.operand_a == 1, "Expected operand_a to match constructor")?;
    ensure(op.operand_b == 2, "Expected operand_b to match constructor")?;
    ensure(op.operand_c == 3, "Expected operand_c to match constructor")?;
    Ok(())
}

/// Tests operation logical.
#[test]
fn test_operation_logical() -> TestResult {
    let and_start = Operation::new(OpCode::AndStart, 0, 0, 0);
    let and_end = Operation::new(OpCode::AndEnd, 0, 0, 0);
    let or_start = Operation::new(OpCode::OrStart, 0, 0, 0);
    let or_end = Operation::new(OpCode::OrEnd, 0, 0, 0);
    let not = Operation::new(OpCode::Not, 0, 0, 0);

    ensure(and_start.opcode == OpCode::AndStart, "Expected AndStart opcode")?;
    ensure(and_end.opcode == OpCode::AndEnd, "Expected AndEnd opcode")?;
    ensure(or_start.opcode == OpCode::OrStart, "Expected OrStart opcode")?;
    ensure(or_end.opcode == OpCode::OrEnd, "Expected OrEnd opcode")?;
    ensure(not.opcode == OpCode::Not, "Expected Not opcode")?;
    Ok(())
}

// ============================================================================
// SECTION: OpCode Tests
// ============================================================================

/// Tests opcode is logical group.
#[test]
fn test_opcode_is_logical_group() -> TestResult {
    ensure(OpCode::AndStart.is_logical_group(), "Expected AndStart to be logical group")?;
    ensure(OpCode::AndEnd.is_logical_group(), "Expected AndEnd to be logical group")?;
    ensure(OpCode::OrStart.is_logical_group(), "Expected OrStart to be logical group")?;
    ensure(OpCode::OrEnd.is_logical_group(), "Expected OrEnd to be logical group")?;

    ensure(!OpCode::Not.is_logical_group(), "Expected Not to be non-group")?;
    ensure(!OpCode::FloatGte.is_logical_group(), "Expected FloatGte to be non-group")?;
    ensure(!OpCode::HasAllFlags.is_logical_group(), "Expected HasAllFlags to be non-group")?;
    Ok(())
}

/// Tests opcode is comparison.
#[test]
fn test_opcode_is_comparison() -> TestResult {
    ensure(OpCode::FloatGte.is_comparison(), "Expected FloatGte to be comparison")?;
    ensure(OpCode::FloatLte.is_comparison(), "Expected FloatLte to be comparison")?;
    ensure(OpCode::FloatEq.is_comparison(), "Expected FloatEq to be comparison")?;
    ensure(OpCode::IntGte.is_comparison(), "Expected IntGte to be comparison")?;
    ensure(OpCode::IntLte.is_comparison(), "Expected IntLte to be comparison")?;
    ensure(OpCode::IntEq.is_comparison(), "Expected IntEq to be comparison")?;

    ensure(!OpCode::AndStart.is_comparison(), "Expected AndStart to be non-comparison")?;
    ensure(!OpCode::HasAllFlags.is_comparison(), "Expected HasAllFlags to be non-comparison")?;
    ensure(!OpCode::InRange.is_comparison(), "Expected InRange to be non-comparison")?;
    Ok(())
}

/// Tests opcode values.
#[test]
fn test_opcode_values() -> TestResult {
    ensure(opcode_value(OpCode::AndStart) == 0, "Expected AndStart opcode value")?;
    ensure(opcode_value(OpCode::AndEnd) == 1, "Expected AndEnd opcode value")?;
    ensure(opcode_value(OpCode::OrStart) == 2, "Expected OrStart opcode value")?;
    ensure(opcode_value(OpCode::OrEnd) == 3, "Expected OrEnd opcode value")?;
    ensure(opcode_value(OpCode::Not) == 4, "Expected Not opcode value")?;

    ensure(opcode_value(OpCode::FloatGte) == 10, "Expected FloatGte opcode value")?;
    ensure(opcode_value(OpCode::HasAllFlags) == 20, "Expected HasAllFlags opcode value")?;
    ensure(opcode_value(OpCode::InRange) == 30, "Expected InRange opcode value")?;
    ensure(opcode_value(OpCode::DomainStart) == 100, "Expected DomainStart opcode value")?;
    Ok(())
}

// ============================================================================
// SECTION: Constant Tests
// ============================================================================

/// Tests constant float.
#[test]
fn test_constant_float() -> TestResult {
    let c = Constant::Float(SAMPLE_FLOAT);
    ensure(c.as_float() == Some(SAMPLE_FLOAT), "Expected float constant roundtrip")?;
    ensure(c.as_int() == Some(3), "Expected float constant to coerce to int")?;
    ensure(c.as_flags().is_none(), "Expected float constant to skip flags")?;
    ensure(c.as_string().is_none(), "Expected float constant to skip string")?;
    Ok(())
}

/// Tests constant int.
#[test]
fn test_constant_int() -> TestResult {
    let c = Constant::Int(42);
    ensure(c.as_float() == Some(42.0), "Expected int constant to coerce to float")?;
    ensure(c.as_int() == Some(42), "Expected int constant roundtrip")?;
    ensure(c.as_flags().is_none(), "Expected int constant to skip flags")?;
    ensure(c.as_string().is_none(), "Expected int constant to skip string")?;
    Ok(())
}

/// Tests constant uint.
#[test]
fn test_constant_uint() -> TestResult {
    let c = Constant::UInt(100);
    ensure(c.as_float() == Some(100.0), "Expected uint constant to coerce to float")?;
    ensure(c.as_int() == Some(100), "Expected uint constant to coerce to int")?;
    ensure(c.as_flags() == Some(100), "Expected uint constant to coerce to flags")?;
    ensure(c.as_string().is_none(), "Expected uint constant to skip string")?;
    Ok(())
}

/// Tests constant string.
#[test]
fn test_constant_string() -> TestResult {
    let c = Constant::String("hello".to_string());
    ensure(c.as_float().is_none(), "Expected string constant to skip float")?;
    ensure(c.as_int().is_none(), "Expected string constant to skip int")?;
    ensure(c.as_flags().is_none(), "Expected string constant to skip flags")?;
    ensure(c.as_string() == Some("hello"), "Expected string constant roundtrip")?;
    Ok(())
}

/// Tests constant flags.
#[test]
fn test_constant_flags() -> TestResult {
    let c = Constant::Flags(0xDEAD_BEEF);
    ensure(c.as_float().is_none(), "Expected flags constant to skip float")?;
    ensure(c.as_int().is_none(), "Expected flags constant to skip int")?;
    ensure(c.as_flags() == Some(0xDEAD_BEEF), "Expected flags constant roundtrip")?;
    ensure(c.as_string().is_none(), "Expected flags constant to skip string")?;
    Ok(())
}

/// Tests constant custom.
#[test]
fn test_constant_custom() -> TestResult {
    let c = Constant::Custom(vec![1, 2, 3, 4]);
    ensure(c.as_float().is_none(), "Expected custom constant to skip float")?;
    ensure(c.as_int().is_none(), "Expected custom constant to skip int")?;
    ensure(c.as_flags().is_none(), "Expected custom constant to skip flags")?;
    ensure(c.as_string().is_none(), "Expected custom constant to skip string")?;
    Ok(())
}

/// Tests constant negative int.
#[test]
fn test_constant_negative_int() -> TestResult {
    let c = Constant::Int(-100);
    ensure(c.as_float() == Some(-100.0), "Expected negative int to coerce to float")?;
    ensure(c.as_int() == Some(-100), "Expected negative int roundtrip")?;
    Ok(())
}

// ============================================================================
// SECTION: Plan Tests
// ============================================================================

/// Tests plan new.
#[test]
fn test_plan_new() -> TestResult {
    let plan = Plan::new();
    ensure(plan.required_columns().is_empty(), "Expected new plan to have no columns")?;
    ensure(plan.operations().is_empty(), "Expected new plan to have no operations")?;
    Ok(())
}

/// Tests plan default.
#[test]
fn test_plan_default() -> TestResult {
    let plan = Plan::default();
    ensure(plan.required_columns().is_empty(), "Expected default plan to have no columns")?;
    ensure(plan.operations().is_empty(), "Expected default plan to have no operations")?;
    Ok(())
}

/// Tests plan add column.
#[test]
fn test_plan_add_column() -> TestResult {
    let mut plan = Plan::new();
    plan.add_column(ColumnKey::new(1));
    plan.add_column(ColumnKey::new(2));

    ensure(plan.required_columns().len() == 2, "Expected two required columns")?;
    ensure(
        plan.required_columns()[0] == ColumnKey::new(1),
        "Expected first column to be preserved",
    )?;
    ensure(
        plan.required_columns()[1] == ColumnKey::new(2),
        "Expected second column to be preserved",
    )?;
    Ok(())
}

/// Tests plan add column dedup.
#[test]
fn test_plan_add_column_dedup() -> TestResult {
    let mut plan = Plan::new();
    plan.add_column(ColumnKey::new(1));
    plan.add_column(ColumnKey::new(1)); // Duplicate
    plan.add_column(ColumnKey::new(2));

    ensure(plan.required_columns().len() == 2, "Expected duplicate columns to be de-duped")?;
    Ok(())
}

/// Tests plan add operation.
#[test]
fn test_plan_add_operation() -> TestResult {
    let mut plan = Plan::new();
    plan.add_operation(Operation::new(OpCode::AndStart, 0, 0, 0));
    plan.add_operation(Operation::new(OpCode::FloatGte, 1, 0, 0));
    plan.add_operation(Operation::new(OpCode::AndEnd, 0, 0, 0));

    ensure(plan.operations().len() == 3, "Expected three operations")?;
    ensure(plan.operations()[0].opcode == OpCode::AndStart, "Expected AndStart at operation 0")?;
    ensure(plan.operations()[1].opcode == OpCode::FloatGte, "Expected FloatGte at operation 1")?;
    ensure(plan.operations()[2].opcode == OpCode::AndEnd, "Expected AndEnd at operation 2")?;
    Ok(())
}

/// Tests plan add constant.
#[test]
fn test_plan_add_constant() -> TestResult {
    let mut plan = Plan::new();
    let idx1 = plan.add_constant(Constant::Float(SAMPLE_FLOAT));
    let idx2 = plan.add_constant(Constant::Int(42));
    let idx3 = plan.add_constant(Constant::Flags(0xFF));

    ensure(idx1.0 == 0, "Expected float constant index to be 0")?;
    ensure(idx2.0 == 1, "Expected int constant index to be 1")?;
    ensure(idx3.0 == 2, "Expected flags constant index to be 2")?;

    ensure(
        plan.constant(idx1).ok_or("missing const 1")?.as_float() == Some(SAMPLE_FLOAT),
        "Expected float constant to be stored",
    )?;
    ensure(
        plan.constant(idx2).ok_or("missing const 2")?.as_int() == Some(42),
        "Expected int constant to be stored",
    )?;
    ensure(
        plan.constant(idx3).ok_or("missing const 3")?.as_flags() == Some(0xFF),
        "Expected flags constant to be stored",
    )?;
    Ok(())
}

/// Tests plan constant out of bounds.
#[test]
fn test_plan_constant_out_of_bounds() -> TestResult {
    let plan = Plan::new();
    ensure(plan.constant(ConstantIndex(0)).is_none(), "Expected missing constant for empty plan")?;
    ensure(
        plan.constant(ConstantIndex(100)).is_none(),
        "Expected missing constant for out-of-range index",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: PlanBuilder Tests
// ============================================================================

/// Tests plan builder new.
#[test]
fn test_plan_builder_new() -> TestResult {
    let builder = PlanBuilder::new();
    let plan = builder.build();
    ensure(plan.required_columns().is_empty(), "Expected builder plan to have no columns")?;
    ensure(plan.operations().is_empty(), "Expected builder plan to have no operations")?;
    Ok(())
}

/// Tests plan builder default.
#[test]
fn test_plan_builder_default() -> TestResult {
    let builder = PlanBuilder::default();
    let plan = builder.build();
    ensure(plan.required_columns().is_empty(), "Expected default builder to have no columns")?;
    Ok(())
}

/// Tests plan builder require column.
#[test]
fn test_plan_builder_require_column() -> TestResult {
    let plan = PlanBuilder::new()
        .require_column(ColumnKey::new(1))
        .require_column(ColumnKey::new(2))
        .build();

    ensure(plan.required_columns().len() == 2, "Expected two required columns")?;
    Ok(())
}

/// Tests plan builder add op.
#[test]
fn test_plan_builder_add_op() -> TestResult {
    let plan = PlanBuilder::new()
        .add_op(OpCode::AndStart, 0, 0, 0)
        .add_op(OpCode::FloatGte, 1, 0, 0)
        .add_op(OpCode::AndEnd, 0, 0, 0)
        .build();

    ensure(plan.operations().len() == 3, "Expected three operations from builder")?;
    Ok(())
}

/// Tests plan builder and start end.
#[test]
fn test_plan_builder_and_start_end() -> TestResult {
    let plan = PlanBuilder::new().and_start().add_op(OpCode::FloatGte, 0, 0, 0).and_end().build();

    ensure(plan.operations().len() == 3, "Expected three operations for AND group")?;
    ensure(
        plan.operations()[0].opcode == OpCode::AndStart,
        "Expected AndStart opcode at operation 0",
    )?;
    ensure(plan.operations()[2].opcode == OpCode::AndEnd, "Expected AndEnd opcode at operation 2")?;
    Ok(())
}

/// Tests plan builder or start end.
#[test]
fn test_plan_builder_or_start_end() -> TestResult {
    let plan = PlanBuilder::new().or_start().add_op(OpCode::IntEq, 0, 0, 0).or_end().build();

    ensure(plan.operations().len() == 3, "Expected three operations for OR group")?;
    ensure(
        plan.operations()[0].opcode == OpCode::OrStart,
        "Expected OrStart opcode at operation 0",
    )?;
    ensure(plan.operations()[2].opcode == OpCode::OrEnd, "Expected OrEnd opcode at operation 2")?;
    Ok(())
}

/// Tests plan builder add constants.
#[test]
fn test_plan_builder_add_constants() -> TestResult {
    let mut builder = PlanBuilder::new();
    let float_idx = builder.add_float_constant(SAMPLE_FLOAT);
    let int_idx = builder.add_int_constant(42);
    let flags_idx = builder.add_flags_constant(0xFF);
    let string_idx = builder.add_string_constant("test".to_string());

    let plan = builder.build();

    ensure(float_idx.0 == 0, "Expected float constant index to be 0")?;
    ensure(int_idx.0 == 1, "Expected int constant index to be 1")?;
    ensure(flags_idx.0 == 2, "Expected flags constant index to be 2")?;
    ensure(string_idx.0 == 3, "Expected string constant index to be 3")?;

    ensure(
        plan.constant(float_idx).ok_or("missing float")?.as_float() == Some(SAMPLE_FLOAT),
        "Expected float constant to be stored",
    )?;
    ensure(
        plan.constant(int_idx).ok_or("missing int")?.as_int() == Some(42),
        "Expected int constant to be stored",
    )?;
    ensure(
        plan.constant(flags_idx).ok_or("missing flags")?.as_flags() == Some(0xFF),
        "Expected flags constant to be stored",
    )?;
    ensure(
        plan.constant(string_idx).ok_or("missing string")?.as_string() == Some("test"),
        "Expected string constant to be stored",
    )?;
    Ok(())
}

/// Tests plan builder complex plan.
#[test]
fn test_plan_builder_complex_plan() -> TestResult {
    let mut builder = PlanBuilder::new();
    let threshold_idx = builder.add_float_constant(50.0);
    let flags_idx = builder.add_flags_constant(0b11);

    let plan = builder
        .require_column(ColumnKey::new(0)) // Health
        .require_column(ColumnKey::new(1)) // Flags
        .and_start()
        .add_op(OpCode::FloatGte, 0, threshold_idx.0, 0)
        .add_op(OpCode::HasAllFlags, 1, flags_idx.0, 0)
        .and_end()
        .build();

    ensure(plan.required_columns().len() == 2, "Expected two required columns")?;
    ensure(plan.operations().len() == 4, "Expected four operations")?;
    Ok(())
}

/// Tests plan builder nested groups.
#[test]
fn test_plan_builder_nested_groups() -> TestResult {
    let plan = PlanBuilder::new()
        .and_start()
        .or_start()
        .add_op(OpCode::FloatGte, 0, 0, 0)
        .add_op(OpCode::FloatLte, 0, 1, 0)
        .or_end()
        .add_op(OpCode::HasAllFlags, 1, 0, 0)
        .and_end()
        .build();

    ensure(plan.operations().len() == 7, "Expected seven operations for nested groups")?;
    Ok(())
}

// ============================================================================
// SECTION: Plan Clone Tests
// ============================================================================

/// Tests plan clone.
#[test]
fn test_plan_clone() -> TestResult {
    let mut builder = PlanBuilder::new();
    builder.add_float_constant(SAMPLE_FLOAT);

    let plan = builder
        .require_column(ColumnKey::new(1))
        .and_start()
        .add_op(OpCode::FloatGte, 0, 0, 0)
        .and_end()
        .build();

    let cloned = plan.clone();
    ensure(
        plan.required_columns().len() == cloned.required_columns().len(),
        "Expected cloned plan to preserve column count",
    )?;
    ensure(
        plan.operations().len() == cloned.operations().len(),
        "Expected cloned plan to preserve operation count",
    )?;
    Ok(())
}
