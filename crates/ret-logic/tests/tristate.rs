// crates/ret-logic/tests/tristate.rs
// ============================================================================
// Module: Tri-State Tests
// Description: Tests for tri-state logic, group semantics, and trace hooks.
// Purpose: Validate tri-state logic tables and group semantics behavior.
// Dependencies: ret_logic::tristate
// ============================================================================
//! ## Overview
//! Validates tri-state evaluation modes and trace hooks for requirement gates.

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

use ret_logic::BochvarLogic;
use ret_logic::KleeneLogic;
use ret_logic::LogicMode;
use ret_logic::Requirement;
use ret_logic::RequirementTrace;
use ret_logic::TriState;
use ret_logic::TriStateConditionEval;
use support::TestResult;
use support::ensure;

// ============================================================================
// SECTION: Test Condition + Reader
// ============================================================================

/// Test conditions for tri-state evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestCondition {
    /// Condition A
    A,
    /// Condition B
    B,
    /// Condition C
    C,
}

/// Reader that provides tri-state values per row
struct TestReader {
    /// Per-row condition values in order A, B, C
    rows: Vec<[TriState; 3]>,
}

impl TestReader {
    /// Creates a reader with the provided row values
    const fn new(rows: Vec<[TriState; 3]>) -> Self {
        Self {
            rows,
        }
    }

    /// Returns the tri-state value for a condition at the given row
    fn value(&self, row: usize, condition: TestCondition) -> TriState {
        let index = match condition {
            TestCondition::A => 0,
            TestCondition::B => 1,
            TestCondition::C => 2,
        };
        self.rows[row][index]
    }
}

impl TriStateConditionEval for TestCondition {
    type Reader<'a> = TestReader;

    fn eval_row_tristate(&self, reader: &Self::Reader<'_>, row: usize) -> TriState {
        reader.value(row, *self)
    }
}

// ============================================================================
// SECTION: Trace Hook
// ============================================================================

/// Captures condition evaluations for trace verification
#[derive(Default)]
struct Trace {
    /// Ordered condition evaluation records
    entries: Vec<(TestCondition, TriState)>,
}

impl RequirementTrace<TestCondition> for Trace {
    fn on_condition_evaluated(&mut self, condition: &TestCondition, result: TriState) {
        self.entries.push((*condition, result));
    }
}

// ============================================================================
// SECTION: Kleene Logic Tests
// ============================================================================

/// Tests kleene and or not.
#[test]
fn test_kleene_and_or_not() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::False]]);

    let and_req = Requirement::and(vec![
        Requirement::condition(TestCondition::A),
        Requirement::condition(TestCondition::B),
    ]);
    let or_req = Requirement::or(vec![
        Requirement::condition(TestCondition::B),
        Requirement::condition(TestCondition::C),
    ]);
    let not_req = Requirement::negate(Requirement::condition(TestCondition::B));

    ensure(
        and_req.eval_tristate(&reader, 0, &KleeneLogic) == TriState::Unknown,
        "Expected Kleene AND to resolve to Unknown",
    )?;
    ensure(
        or_req.eval_tristate(&reader, 0, &KleeneLogic) == TriState::Unknown,
        "Expected Kleene OR to resolve to Unknown",
    )?;
    ensure(
        not_req.eval_tristate(&reader, 0, &KleeneLogic) == TriState::Unknown,
        "Expected Kleene NOT to resolve to Unknown",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Bochvar Logic Tests
// ============================================================================

/// Tests bochvar infectious unknown.
#[test]
fn test_bochvar_infectious_unknown() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::True]]);

    let and_req = Requirement::and(vec![
        Requirement::condition(TestCondition::A),
        Requirement::condition(TestCondition::B),
    ]);
    let or_req = Requirement::or(vec![
        Requirement::condition(TestCondition::A),
        Requirement::condition(TestCondition::B),
    ]);

    ensure(
        and_req.eval_tristate(&reader, 0, &BochvarLogic) == TriState::Unknown,
        "Expected Bochvar AND to resolve to Unknown",
    )?;
    ensure(
        or_req.eval_tristate(&reader, 0, &BochvarLogic) == TriState::Unknown,
        "Expected Bochvar OR to resolve to Unknown",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: RequireGroup Semantics
// ============================================================================

/// Tests require group insufficient evidence.
#[test]
fn test_require_group_insufficient_evidence() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::False]]);

    let group_req = Requirement::require_group(
        2,
        vec![
            Requirement::condition(TestCondition::A),
            Requirement::condition(TestCondition::B),
            Requirement::condition(TestCondition::C),
        ],
    );

    ensure(
        group_req.eval_tristate(&reader, 0, &KleeneLogic) == TriState::Unknown,
        "Expected insufficient evidence to yield Unknown",
    )?;
    Ok(())
}

/// Tests require group failure.
#[test]
fn test_require_group_failure() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::False, TriState::False]]);

    let group_req = Requirement::require_group(
        2,
        vec![
            Requirement::condition(TestCondition::A),
            Requirement::condition(TestCondition::B),
            Requirement::condition(TestCondition::C),
        ],
    );

    ensure(
        group_req.eval_tristate(&reader, 0, &KleeneLogic) == TriState::False,
        "Expected failing require_group to resolve to False",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Trace Hook Tests
// ============================================================================

/// Tests trace hook records conditions.
#[test]
fn test_trace_hook_records_conditions() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::False, TriState::Unknown]]);

    let req = Requirement::and(vec![
        Requirement::condition(TestCondition::A),
        Requirement::condition(TestCondition::B),
        Requirement::condition(TestCondition::C),
    ]);

    let mut trace = Trace::default();
    let result = req.eval_tristate_with_trace(&reader, 0, &KleeneLogic, &mut trace);

    ensure(result == TriState::False, "Expected traced result to be False")?;
    ensure(trace.entries.len() == 3, "Expected three trace entries")?;
    ensure(
        trace.entries[0] == (TestCondition::A, TriState::True),
        "Expected trace entry for condition A",
    )?;
    ensure(
        trace.entries[1] == (TestCondition::B, TriState::False),
        "Expected trace entry for condition B",
    )?;
    ensure(
        trace.entries[2] == (TestCondition::C, TriState::Unknown),
        "Expected trace entry for condition C",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Runtime Logic Mode
// ============================================================================

/// Tests logic mode dispatch.
#[test]
fn test_logic_mode_dispatch() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::False]]);

    let req = Requirement::and(vec![
        Requirement::condition(TestCondition::A),
        Requirement::condition(TestCondition::B),
    ]);

    ensure(
        req.eval_tristate(&reader, 0, &LogicMode::Kleene) == TriState::Unknown,
        "Expected Kleene logic mode to match Kleene evaluation",
    )?;
    ensure(
        req.eval_tristate(&reader, 0, &LogicMode::Bochvar) == TriState::Unknown,
        "Expected Bochvar logic mode to match Bochvar evaluation",
    )?;
    Ok(())
}
