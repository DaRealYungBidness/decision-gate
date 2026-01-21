// ret-logic/tests/tristate.rs
// ============================================================================
// Module: Tri-State Tests
// Description: Tests for tri-state logic, group semantics, and trace hooks.
// ============================================================================
//! ## Overview
//! Validates tri-state evaluation modes and trace hooks for requirement gates.

mod support;

use ret_logic::BochvarLogic;
use ret_logic::KleeneLogic;
use ret_logic::LogicMode;
use ret_logic::Requirement;
use ret_logic::RequirementTrace;
use ret_logic::TriState;
use ret_logic::TriStatePredicateEval;
use support::TestResult;
use support::ensure;

// ============================================================================
// SECTION: Test Predicate + Reader
// ============================================================================

/// Test predicates for tri-state evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestPredicate {
    /// Predicate A
    A,
    /// Predicate B
    B,
    /// Predicate C
    C,
}

/// Reader that provides tri-state values per row
struct TestReader {
    /// Per-row predicate values in order A, B, C
    rows: Vec<[TriState; 3]>,
}

impl TestReader {
    /// Creates a reader with the provided row values
    const fn new(rows: Vec<[TriState; 3]>) -> Self {
        Self {
            rows,
        }
    }

    /// Returns the tri-state value for a predicate at the given row
    fn value(&self, row: usize, predicate: TestPredicate) -> TriState {
        let index = match predicate {
            TestPredicate::A => 0,
            TestPredicate::B => 1,
            TestPredicate::C => 2,
        };
        self.rows[row][index]
    }
}

impl TriStatePredicateEval for TestPredicate {
    type Reader<'a> = TestReader;

    fn eval_row_tristate(&self, reader: &Self::Reader<'_>, row: usize) -> TriState {
        reader.value(row, *self)
    }
}

// ============================================================================
// SECTION: Trace Hook
// ============================================================================

/// Captures predicate evaluations for trace verification
#[derive(Default)]
struct Trace {
    /// Ordered predicate evaluation records
    entries: Vec<(TestPredicate, TriState)>,
}

impl RequirementTrace<TestPredicate> for Trace {
    fn on_predicate_evaluated(&mut self, predicate: &TestPredicate, result: TriState) {
        self.entries.push((*predicate, result));
    }
}

// ============================================================================
// SECTION: Kleene Logic Tests
// ============================================================================

#[test]
fn test_kleene_and_or_not() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::False]]);

    let and_req = Requirement::and(vec![
        Requirement::predicate(TestPredicate::A),
        Requirement::predicate(TestPredicate::B),
    ]);
    let or_req = Requirement::or(vec![
        Requirement::predicate(TestPredicate::B),
        Requirement::predicate(TestPredicate::C),
    ]);
    let not_req = Requirement::not(Requirement::predicate(TestPredicate::B));

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

#[test]
fn test_bochvar_infectious_unknown() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::True]]);

    let and_req = Requirement::and(vec![
        Requirement::predicate(TestPredicate::A),
        Requirement::predicate(TestPredicate::B),
    ]);
    let or_req = Requirement::or(vec![
        Requirement::predicate(TestPredicate::A),
        Requirement::predicate(TestPredicate::B),
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

#[test]
fn test_require_group_insufficient_evidence() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::False]]);

    let group_req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(TestPredicate::A),
            Requirement::predicate(TestPredicate::B),
            Requirement::predicate(TestPredicate::C),
        ],
    );

    ensure(
        group_req.eval_tristate(&reader, 0, &KleeneLogic) == TriState::Unknown,
        "Expected insufficient evidence to yield Unknown",
    )?;
    Ok(())
}

#[test]
fn test_require_group_failure() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::False, TriState::False]]);

    let group_req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(TestPredicate::A),
            Requirement::predicate(TestPredicate::B),
            Requirement::predicate(TestPredicate::C),
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

#[test]
fn test_trace_hook_records_predicates() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::False, TriState::Unknown]]);

    let req = Requirement::and(vec![
        Requirement::predicate(TestPredicate::A),
        Requirement::predicate(TestPredicate::B),
        Requirement::predicate(TestPredicate::C),
    ]);

    let mut trace = Trace::default();
    let result = req.eval_tristate_with_trace(&reader, 0, &KleeneLogic, &mut trace);

    ensure(result == TriState::False, "Expected traced result to be False")?;
    ensure(trace.entries.len() == 3, "Expected three trace entries")?;
    ensure(
        trace.entries[0] == (TestPredicate::A, TriState::True),
        "Expected trace entry for predicate A",
    )?;
    ensure(
        trace.entries[1] == (TestPredicate::B, TriState::False),
        "Expected trace entry for predicate B",
    )?;
    ensure(
        trace.entries[2] == (TestPredicate::C, TriState::Unknown),
        "Expected trace entry for predicate C",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Runtime Logic Mode
// ============================================================================

#[test]
fn test_logic_mode_dispatch() -> TestResult {
    let reader = TestReader::new(vec![[TriState::True, TriState::Unknown, TriState::False]]);

    let req = Requirement::and(vec![
        Requirement::predicate(TestPredicate::A),
        Requirement::predicate(TestPredicate::B),
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
