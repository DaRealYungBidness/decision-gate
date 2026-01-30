// ret-logic/tests/traits.rs
// ============================================================================
// Module: Traits Tests
// Description: Tests for ConditionEval, BatchConditionEval, and ReaderLen.
// Purpose: Validate condition evaluation trait helpers and reader utilities.
// Dependencies: ret_logic::traits
// ============================================================================
//! ## Overview
//! Integration tests for condition and reader traits.

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

#[path = "support/mocks.rs"]
mod mocks;
mod support;

use mocks::MockCondition;
use mocks::MockReader;
use ret_logic::BatchConditionEval;
use ret_logic::Mask64;
use ret_logic::ReaderLen;
use ret_logic::Row;
use ret_logic::eval_reader_rows;
use support::TestResult;
use support::ensure;

// ========================================================================
// SECTION: Mock Coverage
// ========================================================================

/// Tests mock condition variants used.
#[test]
fn test_mock_condition_variants_used() {
    let _ = mocks::all_variants();
}

// ============================================================================
// SECTION: ReaderLen Tests
// ============================================================================

/// Tests reader len empty.
#[test]
fn test_reader_len_empty() -> TestResult {
    let reader = MockReader::new(&[], &[]);
    ensure(reader.len() == 0, "Expected empty reader length to be zero")?;
    ensure(reader.is_empty(), "Expected empty reader to report is_empty")?;
    Ok(())
}

/// Tests reader len non empty.
#[test]
fn test_reader_len_non_empty() -> TestResult {
    let values = vec![1, 2, 3, 4, 5];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);
    ensure(reader.len() == 5, "Expected reader length to match values")?;
    ensure(!reader.is_empty(), "Expected non-empty reader to report not empty")?;
    Ok(())
}

/// Tests reader len single.
#[test]
fn test_reader_len_single() -> TestResult {
    let values = vec![42];
    let flags = vec![0];
    let reader = MockReader::new(&values, &flags);
    ensure(reader.len() == 1, "Expected reader length to match single value")?;
    ensure(!reader.is_empty(), "Expected single reader to report not empty")?;
    Ok(())
}

// ============================================================================
// SECTION: BatchConditionEval Default Implementation Tests
// ============================================================================

/// Tests eval block empty.
#[test]
fn test_eval_block_empty() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![];
    let flags = vec![];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 0);
    ensure(mask == 0, "Expected empty eval_block to return zero mask")?;
    Ok(())
}

/// Tests eval block single true.
#[test]
fn test_eval_block_single_true() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0];
    let flags = vec![0];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 1);
    ensure(mask == 0b1, "Expected single true condition to set bit 0")?;
    Ok(())
}

/// Tests eval block single false.
#[test]
fn test_eval_block_single_false() -> TestResult {
    let pred = MockCondition::AlwaysFalse;
    let values = vec![0];
    let flags = vec![0];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 1);
    ensure(mask == 0, "Expected single false condition to return zero mask")?;
    Ok(())
}

/// Tests eval block multiple all true.
#[test]
fn test_eval_block_multiple_all_true() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 8];
    let flags = vec![0; 8];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 8);
    ensure(mask == 0b1111_1111, "Expected all-true block to set all bits")?;
    Ok(())
}

/// Tests eval block multiple all false.
#[test]
fn test_eval_block_multiple_all_false() -> TestResult {
    let pred = MockCondition::AlwaysFalse;
    let values = vec![0; 8];
    let flags = vec![0; 8];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 8);
    ensure(mask == 0, "Expected all-false block to return zero mask")?;
    Ok(())
}

/// Tests eval block alternating.
#[test]
fn test_eval_block_alternating() -> TestResult {
    let pred = MockCondition::RowIndexEven;
    let values = vec![0; 8];
    let flags = vec![0; 8];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 8);
    // Rows 0, 2, 4, 6 are even
    ensure(mask == 0b0101_0101, "Expected alternating even mask pattern")?;
    Ok(())
}

/// Tests eval block with offset.
#[test]
fn test_eval_block_with_offset() -> TestResult {
    let pred = MockCondition::RowIndexLt(5);
    let values = vec![0; 10];
    let flags = vec![0; 10];
    let reader = MockReader::new(&values, &flags);

    // Start from row 3, check 4 rows (3, 4, 5, 6)
    let mask = pred.eval_block(&reader, 3, 4);
    // Rows 3, 4 pass (< 5), rows 5, 6 fail
    ensure(mask == 0b0011, "Expected offset block to map to local mask")?;
    Ok(())
}

/// Tests eval block value threshold.
#[test]
fn test_eval_block_value_threshold() -> TestResult {
    let pred = MockCondition::ValueGte(50);
    let values = vec![0, 25, 50, 75, 100, 25, 50, 75];
    let flags = vec![0; 8];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 8);
    // Values >= 50 at indices 2, 3, 4, 6, 7
    ensure(mask == 0b1101_1100, "Expected value threshold mask to match")?;
    Ok(())
}

/// Tests eval block full 64.
#[test]
fn test_eval_block_full_64() -> TestResult {
    let pred = MockCondition::RowIndexEven;
    let values = vec![0; 64];
    let flags = vec![0; 64];
    let reader = MockReader::new(&values, &flags);

    let mask = pred.eval_block(&reader, 0, 64);
    // All even indices set: 0x5555555555555555
    let expected: Mask64 = 0x5555_5555_5555_5555;
    ensure(mask == expected, "Expected full mask for even rows")?;
    Ok(())
}

/// Tests eval block count clamped to 64.
#[test]
fn test_eval_block_count_clamped_to_64() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 100];
    let flags = vec![0; 100];
    let reader = MockReader::new(&values, &flags);

    // Request 100 but should be clamped to 64
    let mask = pred.eval_block(&reader, 0, 100);
    ensure(mask == u64::MAX, "Expected count to clamp to 64 rows")?;
    Ok(())
}

/// Tests eval block partial window.
#[test]
fn test_eval_block_partial_window() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 10];
    let flags = vec![0; 10];
    let reader = MockReader::new(&values, &flags);

    // Only 10 entities but request 64
    let mask = pred.eval_block(&reader, 0, 10);
    // Only first 10 bits set
    ensure(mask == 0b11_1111_1111, "Expected partial window mask to set 10 bits")?;
    Ok(())
}

// ============================================================================
// SECTION: eval_reader_rows Tests
// ============================================================================

/// Tests eval reader rows empty.
#[test]
fn test_eval_reader_rows_empty() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let reader = MockReader::new(&[], &[]);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows.is_empty(), "Expected empty reader to produce no rows")?;
    Ok(())
}

/// Tests eval reader rows all pass.
#[test]
fn test_eval_reader_rows_all_pass() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 10];
    let flags = vec![0; 10];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows == vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9], "Expected all rows to pass")?;
    Ok(())
}

/// Tests eval reader rows none pass.
#[test]
fn test_eval_reader_rows_none_pass() -> TestResult {
    let pred = MockCondition::AlwaysFalse;
    let values = vec![0; 10];
    let flags = vec![0; 10];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows.is_empty(), "Expected no rows to pass")?;
    Ok(())
}

/// Tests eval reader rows some pass.
#[test]
fn test_eval_reader_rows_some_pass() -> TestResult {
    let pred = MockCondition::ValueGte(50);
    let values = vec![0, 25, 50, 75, 100];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows == vec![2, 3, 4], "Expected rows 2-4 to pass threshold")?;
    Ok(())
}

/// Tests eval reader rows alternating.
#[test]
fn test_eval_reader_rows_alternating() -> TestResult {
    let pred = MockCondition::RowIndexEven;
    let values = vec![0; 10];
    let flags = vec![0; 10];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows == vec![0, 2, 4, 6, 8], "Expected even rows to pass")?;
    Ok(())
}

/// Tests eval reader rows large reader.
#[test]
fn test_eval_reader_rows_large_reader() -> TestResult {
    let pred = MockCondition::RowIndexLt(100);
    let values = vec![0; 200];
    let flags = vec![0; 200];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows.len() == 100, "Expected 100 rows to pass")?;
    ensure(rows[0] == 0, "Expected first row to be 0")?;
    ensure(rows[99] == 99, "Expected last row to be 99")?;
    Ok(())
}

/// Tests eval reader rows exactly 64.
#[test]
fn test_eval_reader_rows_exactly_64() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 64];
    let flags = vec![0; 64];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows.len() == 64, "Expected 64 rows to pass")?;
    Ok(())
}

/// Tests eval reader rows just over 64.
#[test]
fn test_eval_reader_rows_just_over_64() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 65];
    let flags = vec![0; 65];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows.len() == 65, "Expected 65 rows to pass")?;
    ensure(rows[64] == 64, "Expected last row to be 64")?;
    Ok(())
}

/// Tests eval reader rows multiple windows.
#[test]
fn test_eval_reader_rows_multiple_windows() -> TestResult {
    let pred = MockCondition::AlwaysTrue;
    let values = vec![0; 150];
    let flags = vec![0; 150];
    let reader = MockReader::new(&values, &flags);

    let rows = eval_reader_rows(&pred, &reader);
    ensure(rows.len() == 150, "Expected all rows to pass across multiple windows")?;
    Ok(())
}

// ============================================================================
// SECTION: Type Alias Tests
// ============================================================================

/// Tests row type.
#[test]
fn test_row_type() -> TestResult {
    let row: Row = 42;
    ensure(row == 42usize, "Expected Row type alias to match usize")?;
    Ok(())
}

/// Tests mask64 type.
#[test]
fn test_mask64_type() -> TestResult {
    let mask: Mask64 = 0xDEAD_BEEF;
    ensure(mask == 0xDEAD_BEEF_u64, "Expected Mask64 type alias to match u64")?;
    Ok(())
}

/// Tests mask64 operations.
#[test]
fn test_mask64_operations() -> TestResult {
    let mask1: Mask64 = 0b1010;
    let mask2: Mask64 = 0b1100;

    ensure(mask1 & mask2 == 0b1000, "Expected AND to preserve shared bits")?;
    ensure(mask1 | mask2 == 0b1110, "Expected OR to combine bits")?;
    ensure(!mask1 == !0b1010_u64, "Expected NOT to invert bits")?;
    Ok(())
}
