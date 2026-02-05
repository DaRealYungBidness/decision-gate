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
use ret_logic::GroupCounts;
use ret_logic::KleeneLogic;
use ret_logic::LogicMode;
use ret_logic::Requirement;
use ret_logic::RequirementTrace;
use ret_logic::TriLogic;
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

// ============================================================================
// SECTION: Kleene Logic Complete Truth Tables
// ============================================================================

/// Complete 3x3 truth table for Kleene AND
///
/// Truth table (Strong Kleene):
/// AND     | True    | False   | Unknown
/// --------|---------|---------|--------
/// True    | True    | False   | Unknown
/// False   | False   | False   | False
/// Unknown | Unknown | False   | Unknown
#[test]
fn kleene_and_complete_truth_table() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = KleeneLogic;

    // Row 1: True AND x
    ensure(logic.and(True, True) == True, "T AND T = T")?;
    ensure(logic.and(True, False) == False, "T AND F = F")?;
    ensure(logic.and(True, Unknown) == Unknown, "T AND U = U")?;

    // Row 2: False AND x (absorbing element)
    ensure(logic.and(False, True) == False, "F AND T = F")?;
    ensure(logic.and(False, False) == False, "F AND F = F")?;
    ensure(logic.and(False, Unknown) == False, "F AND U = F")?;

    // Row 3: Unknown AND x
    ensure(logic.and(Unknown, True) == Unknown, "U AND T = U")?;
    ensure(logic.and(Unknown, False) == False, "U AND F = F")?;
    ensure(logic.and(Unknown, Unknown) == Unknown, "U AND U = U")?;

    Ok(())
}

/// Complete 3x3 truth table for Kleene OR
///
/// Truth table (Strong Kleene):
/// OR      | True    | False   | Unknown
/// --------|---------|---------|--------
/// True    | True    | True    | True
/// False   | True    | False   | Unknown
/// Unknown | True    | Unknown | Unknown
#[test]
fn kleene_or_complete_truth_table() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = KleeneLogic;

    // Row 1: True OR x (absorbing element)
    ensure(logic.or(True, True) == True, "T OR T = T")?;
    ensure(logic.or(True, False) == True, "T OR F = T")?;
    ensure(logic.or(True, Unknown) == True, "T OR U = T")?;

    // Row 2: False OR x
    ensure(logic.or(False, True) == True, "F OR T = T")?;
    ensure(logic.or(False, False) == False, "F OR F = F")?;
    ensure(logic.or(False, Unknown) == Unknown, "F OR U = U")?;

    // Row 3: Unknown OR x
    ensure(logic.or(Unknown, True) == True, "U OR T = T")?;
    ensure(logic.or(Unknown, False) == Unknown, "U OR F = U")?;
    ensure(logic.or(Unknown, Unknown) == Unknown, "U OR U = U")?;

    Ok(())
}

/// Complete truth table for Kleene NOT
#[test]
fn kleene_not_complete_truth_table() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = KleeneLogic;

    ensure(logic.not(True) == False, "NOT T = F")?;
    ensure(logic.not(False) == True, "NOT F = T")?;
    ensure(logic.not(Unknown) == Unknown, "NOT U = U")?;

    Ok(())
}

/// Verify Kleene AND commutativity
#[test]
fn kleene_and_is_commutative() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = KleeneLogic;
    let values = [True, False, Unknown];

    for &a in &values {
        for &b in &values {
            ensure(
                logic.and(a, b) == logic.and(b, a),
                format!("AND must be commutative: {:?} AND {:?}", a, b),
            )?;
        }
    }
    Ok(())
}

/// Verify Kleene OR commutativity
#[test]
fn kleene_or_is_commutative() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = KleeneLogic;
    let values = [True, False, Unknown];

    for &a in &values {
        for &b in &values {
            ensure(
                logic.or(a, b) == logic.or(b, a),
                format!("OR must be commutative: {:?} OR {:?}", a, b),
            )?;
        }
    }
    Ok(())
}

/// Verify Kleene double negation
#[test]
fn kleene_double_negation() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = KleeneLogic;

    ensure(logic.not(logic.not(True)) == True, "NOT NOT T = T")?;
    ensure(logic.not(logic.not(False)) == False, "NOT NOT F = F")?;
    ensure(logic.not(logic.not(Unknown)) == Unknown, "NOT NOT U = U")?;

    Ok(())
}

// ============================================================================
// SECTION: Bochvar Logic Complete Truth Tables
// ============================================================================

/// Complete 3x3 truth table for Bochvar AND
///
/// Truth table (Bochvar - infectious Unknown):
/// AND     | True    | False   | Unknown
/// --------|---------|---------|--------
/// True    | True    | False   | Unknown
/// False   | False   | False   | Unknown
/// Unknown | Unknown | Unknown | Unknown
#[test]
fn bochvar_and_complete_truth_table() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = BochvarLogic;

    // Row 1: True AND x
    ensure(logic.and(True, True) == True, "T AND T = T")?;
    ensure(logic.and(True, False) == False, "T AND F = F")?;
    ensure(logic.and(True, Unknown) == Unknown, "T AND U = U (infectious)")?;

    // Row 2: False AND x
    ensure(logic.and(False, True) == False, "F AND T = F")?;
    ensure(logic.and(False, False) == False, "F AND F = F")?;
    ensure(logic.and(False, Unknown) == Unknown, "F AND U = U (infectious)")?;

    // Row 3: Unknown AND x (always Unknown)
    ensure(logic.and(Unknown, True) == Unknown, "U AND T = U (infectious)")?;
    ensure(logic.and(Unknown, False) == Unknown, "U AND F = U (infectious)")?;
    ensure(logic.and(Unknown, Unknown) == Unknown, "U AND U = U")?;

    Ok(())
}

/// Complete 3x3 truth table for Bochvar OR
///
/// Truth table (Bochvar - infectious Unknown):
/// OR      | True    | False   | Unknown
/// --------|---------|---------|--------
/// True    | True    | True    | Unknown
/// False   | True    | False   | Unknown
/// Unknown | Unknown | Unknown | Unknown
#[test]
fn bochvar_or_complete_truth_table() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = BochvarLogic;

    // Row 1: True OR x
    ensure(logic.or(True, True) == True, "T OR T = T")?;
    ensure(logic.or(True, False) == True, "T OR F = T")?;
    ensure(logic.or(True, Unknown) == Unknown, "T OR U = U (infectious)")?;

    // Row 2: False OR x
    ensure(logic.or(False, True) == True, "F OR T = T")?;
    ensure(logic.or(False, False) == False, "F OR F = F")?;
    ensure(logic.or(False, Unknown) == Unknown, "F OR U = U (infectious)")?;

    // Row 3: Unknown OR x (always Unknown)
    ensure(logic.or(Unknown, True) == Unknown, "U OR T = U (infectious)")?;
    ensure(logic.or(Unknown, False) == Unknown, "U OR F = U (infectious)")?;
    ensure(logic.or(Unknown, Unknown) == Unknown, "U OR U = U")?;

    Ok(())
}

/// Complete truth table for Bochvar NOT
#[test]
fn bochvar_not_complete_truth_table() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;
    let logic = BochvarLogic;

    ensure(logic.not(True) == False, "NOT T = F")?;
    ensure(logic.not(False) == True, "NOT F = T")?;
    ensure(logic.not(Unknown) == Unknown, "NOT U = U")?;

    Ok(())
}

/// Key difference: Bochvar vs Kleene on False AND Unknown and True OR Unknown
#[test]
fn bochvar_vs_kleene_key_difference() -> TestResult {
    use TriState::False;
    use TriState::True;
    use TriState::Unknown;

    // This is THE key difference between the two logics
    ensure(KleeneLogic.and(False, Unknown) == False, "Kleene: F AND U = F (False absorbs)")?;
    ensure(
        BochvarLogic.and(False, Unknown) == Unknown,
        "Bochvar: F AND U = U (Unknown is infectious)",
    )?;

    ensure(KleeneLogic.or(True, Unknown) == True, "Kleene: T OR U = T (True absorbs)")?;
    ensure(
        BochvarLogic.or(True, Unknown) == Unknown,
        "Bochvar: T OR U = U (Unknown is infectious)",
    )?;

    Ok(())
}

// ============================================================================
// SECTION: require_group Complete Boundary Tests
// ============================================================================

/// min=0 always returns True (vacuously satisfied)
#[test]
fn require_group_min_zero_always_true() -> TestResult {
    let logic = KleeneLogic;

    // All combinations with min=0 should return True
    let test_cases = [
        GroupCounts {
            satisfied: 0,
            unknown: 0,
            total: 0,
        },
        GroupCounts {
            satisfied: 0,
            unknown: 0,
            total: 5,
        },
        GroupCounts {
            satisfied: 0,
            unknown: 5,
            total: 5,
        },
        GroupCounts {
            satisfied: 5,
            unknown: 0,
            total: 5,
        },
        GroupCounts {
            satisfied: 2,
            unknown: 3,
            total: 5,
        },
    ];

    for counts in test_cases {
        ensure(
            logic.require_group(0, counts) == TriState::True,
            format!("min=0 with {:?} should be True", counts),
        )?;
    }
    Ok(())
}

/// Exact threshold satisfaction
#[test]
fn require_group_exact_threshold() -> TestResult {
    let logic = KleeneLogic;

    // Exactly meeting the threshold
    let counts = GroupCounts {
        satisfied: 3,
        unknown: 0,
        total: 5,
    };
    ensure(logic.require_group(3, counts) == TriState::True, "satisfied == min should be True")?;

    // One under threshold with no unknowns
    let counts = GroupCounts {
        satisfied: 2,
        unknown: 0,
        total: 5,
    };
    ensure(
        logic.require_group(3, counts) == TriState::False,
        "satisfied < min with no unknowns should be False",
    )?;

    // One over threshold
    let counts = GroupCounts {
        satisfied: 4,
        unknown: 0,
        total: 5,
    };
    ensure(logic.require_group(3, counts) == TriState::True, "satisfied > min should be True")?;

    Ok(())
}

/// Impossible combinations (satisfied + unknown < min)
#[test]
fn require_group_impossible_path() -> TestResult {
    let logic = KleeneLogic;

    // Even if all unknowns resolve to True, cannot meet threshold
    let counts = GroupCounts {
        satisfied: 1,
        unknown: 1,
        total: 5,
    };
    ensure(
        logic.require_group(3, counts) == TriState::False,
        "Impossible to reach threshold should be False",
    )?;

    // Edge case: exactly one short
    let counts = GroupCounts {
        satisfied: 2,
        unknown: 0,
        total: 5,
    };
    ensure(
        logic.require_group(3, counts) == TriState::False,
        "satisfied + unknown < min should be False",
    )?;

    Ok(())
}

/// Uncertain outcomes (could still reach threshold)
#[test]
fn require_group_uncertain_outcomes() -> TestResult {
    let logic = KleeneLogic;

    // Can potentially reach threshold if unknowns resolve favorably
    let counts = GroupCounts {
        satisfied: 2,
        unknown: 2,
        total: 5,
    };
    ensure(
        logic.require_group(3, counts) == TriState::Unknown,
        "Could reach threshold with unknowns should be Unknown",
    )?;

    // Exactly at boundary
    let counts = GroupCounts {
        satisfied: 2,
        unknown: 1,
        total: 5,
    };
    ensure(
        logic.require_group(3, counts) == TriState::Unknown,
        "satisfied + unknown == min should be Unknown",
    )?;

    Ok(())
}

/// min=1 edge cases
#[test]
fn require_group_min_one() -> TestResult {
    let logic = KleeneLogic;

    // One satisfied
    ensure(
        logic.require_group(
            1,
            GroupCounts {
                satisfied: 1,
                unknown: 0,
                total: 3,
            },
        ) == TriState::True,
        "min=1, satisfied=1 should be True",
    )?;

    // None satisfied, one unknown
    ensure(
        logic.require_group(
            1,
            GroupCounts {
                satisfied: 0,
                unknown: 1,
                total: 3,
            },
        ) == TriState::Unknown,
        "min=1, satisfied=0, unknown=1 should be Unknown",
    )?;

    // None satisfied, none unknown
    ensure(
        logic.require_group(
            1,
            GroupCounts {
                satisfied: 0,
                unknown: 0,
                total: 3,
            },
        ) == TriState::False,
        "min=1, all failed should be False",
    )?;

    Ok(())
}

/// min equals total (all must pass)
#[test]
fn require_group_min_equals_total() -> TestResult {
    let logic = KleeneLogic;

    // All satisfied
    ensure(
        logic.require_group(
            5,
            GroupCounts {
                satisfied: 5,
                unknown: 0,
                total: 5,
            },
        ) == TriState::True,
        "All satisfied should be True",
    )?;

    // One unknown
    ensure(
        logic.require_group(
            5,
            GroupCounts {
                satisfied: 4,
                unknown: 1,
                total: 5,
            },
        ) == TriState::Unknown,
        "4/5 satisfied, 1 unknown should be Unknown",
    )?;

    // One failed
    ensure(
        logic.require_group(
            5,
            GroupCounts {
                satisfied: 4,
                unknown: 0,
                total: 5,
            },
        ) == TriState::False,
        "4/5 satisfied, 1 failed should be False",
    )?;

    Ok(())
}

/// u8 boundary for min parameter
#[test]
fn require_group_u8_max_boundary() -> TestResult {
    let logic = KleeneLogic;

    // min = 255 (u8::MAX)
    let counts = GroupCounts {
        satisfied: 255,
        unknown: 0,
        total: 255,
    };
    ensure(
        logic.require_group(255, counts) == TriState::True,
        "min=255, satisfied=255 should be True",
    )?;

    let counts = GroupCounts {
        satisfied: 254,
        unknown: 1,
        total: 255,
    };
    ensure(
        logic.require_group(255, counts) == TriState::Unknown,
        "min=255, satisfied=254, unknown=1 should be Unknown",
    )?;

    Ok(())
}

/// GroupCounts::failed() calculation
#[test]
fn group_counts_failed_calculation() -> TestResult {
    let counts = GroupCounts {
        satisfied: 2,
        unknown: 1,
        total: 5,
    };
    ensure(counts.failed() == 2, "5 - 2 - 1 = 2 failed")?;

    let counts = GroupCounts {
        satisfied: 5,
        unknown: 0,
        total: 5,
    };
    ensure(counts.failed() == 0, "All satisfied = 0 failed")?;

    let counts = GroupCounts {
        satisfied: 0,
        unknown: 0,
        total: 5,
    };
    ensure(counts.failed() == 5, "None satisfied = all failed")?;

    // Saturating behavior (should not panic on overflow)
    let counts = GroupCounts {
        satisfied: 10,
        unknown: 10,
        total: 5,
    };
    ensure(counts.failed() == 0, "Saturating sub prevents overflow")?;

    Ok(())
}

// ============================================================================
// SECTION: TriState Helper Method Tests
// ============================================================================

#[test]
fn tristate_is_true() -> TestResult {
    ensure(TriState::True.is_true(), "True.is_true() should be true")?;
    ensure(!TriState::False.is_true(), "False.is_true() should be false")?;
    ensure(!TriState::Unknown.is_true(), "Unknown.is_true() should be false")?;
    Ok(())
}

#[test]
fn tristate_is_false() -> TestResult {
    ensure(!TriState::True.is_false(), "True.is_false() should be false")?;
    ensure(TriState::False.is_false(), "False.is_false() should be true")?;
    ensure(!TriState::Unknown.is_false(), "Unknown.is_false() should be false")?;
    Ok(())
}

#[test]
fn tristate_is_unknown() -> TestResult {
    ensure(!TriState::True.is_unknown(), "True.is_unknown() should be false")?;
    ensure(!TriState::False.is_unknown(), "False.is_unknown() should be false")?;
    ensure(TriState::Unknown.is_unknown(), "Unknown.is_unknown() should be true")?;
    Ok(())
}

#[test]
fn tristate_from_bool() -> TestResult {
    ensure(TriState::from(true) == TriState::True, "true -> True")?;
    ensure(TriState::from(false) == TriState::False, "false -> False")?;
    Ok(())
}
