// ret-logic/tests/requirement.rs
// ============================================================================
// Module: Core Requirement Tests
// Description: Exhaustive tests for requirement evaluation and analysis.
// ============================================================================
//! ## Overview
//! Integration tests for the core requirement types and evaluation paths.

#[path = "support/flags.rs"]
mod flags;
#[path = "support/mocks.rs"]
mod mocks;
mod support;

use flags::FLAG_A;
use flags::FLAG_AB;
use flags::FLAG_B;
use flags::FLAG_C;
use mocks::MockPredicate;
use mocks::MockReader;
use ret_logic::Requirement;
use ret_logic::RequirementGroup;
use ret_logic::RequirementGroupError;
use ret_logic::RequirementId;
use support::TestResult;
use support::ensure;

// ========================================================================
// SECTION: Mock Coverage
// ========================================================================

#[test]
fn test_mock_predicate_variants_used() {
    let _ = mocks::all_variants();
}

/// Creates a requirement id for test fixtures.
macro_rules! rid {
    ($value:expr) => {
        RequirementId::new($value)?
    };
}

/// Checks a condition and returns a test error instead of panicking.
macro_rules! check {
    ($cond:expr $(,)?) => {{
        ensure($cond, concat!("Assertion failed: ", stringify!($cond)))?;
    }};
    ($cond:expr, $($arg:tt)+) => {{
        ensure($cond, format!($($arg)+))?;
    }};
}

/// Checks equality and returns a test error instead of panicking.
macro_rules! check_eq {
    ($left:expr, $right:expr $(,)?) => {{
        let left_val = &$left;
        let right_val = &$right;
        ensure(
            left_val == right_val,
            format!("Expected {left_val:?} == {right_val:?}"),
        )?;
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left_val = &$left;
        let right_val = &$right;
        ensure(left_val == right_val, format!($($arg)+))?;
    }};
}

/// Checks inequality and returns a test error instead of panicking.
macro_rules! check_ne {
    ($left:expr, $right:expr $(,)?) => {{
        let left_val = &$left;
        let right_val = &$right;
        ensure(
            left_val != right_val,
            format!("Expected {left_val:?} != {right_val:?}"),
        )?;
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left_val = &$left;
        let right_val = &$right;
        ensure(left_val != right_val, format!($($arg)+))?;
    }};
}

// ============================================================================
// SECTION: RequirementId Tests
// ============================================================================

#[test]
fn test_requirement_id_creation() -> TestResult {
    let id = rid!(42);
    check_eq!(id.value(), 42);
    Ok(())
}

#[test]
fn test_requirement_id_value() -> TestResult {
    let id = rid!(12345);
    check_eq!(id.0.get(), 12345);
    check_eq!(id.value(), 12345);
    Ok(())
}

#[test]
fn test_requirement_id_equality() -> TestResult {
    let id1 = rid!(100);
    let id2 = rid!(100);
    let id3 = rid!(200);

    check_eq!(id1, id2);
    check_ne!(id1, id3);
    Ok(())
}

#[test]
fn test_requirement_id_hash() -> TestResult {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(rid!(1));
    set.insert(rid!(2));
    set.insert(rid!(1)); // Duplicate

    check_eq!(set.len(), 2);
    Ok(())
}

#[test]
fn test_requirement_id_clone_copy() -> TestResult {
    let id = rid!(999);
    let cloned = id;
    let copied = id;

    check_eq!(id, cloned);
    check_eq!(id, copied);
    Ok(())
}

// ============================================================================
// SECTION: Predicate Evaluation Tests
// ============================================================================

#[test]
fn test_predicate_always_true() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_predicate_always_false() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysFalse);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_predicate_value_gte() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueGte(50));
    let values = vec![0, 49, 50, 51, 100];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0)); // 0 < 50
    check!(!req.eval(&reader, 1)); // 49 < 50
    check!(req.eval(&reader, 2)); // 50 >= 50
    check!(req.eval(&reader, 3)); // 51 >= 50
    check!(req.eval(&reader, 4)); // 100 >= 50
    Ok(())
}

#[test]
fn test_predicate_value_lte() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueLte(50));
    let values = vec![0, 49, 50, 51, 100];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // 0 <= 50
    check!(req.eval(&reader, 1)); // 49 <= 50
    check!(req.eval(&reader, 2)); // 50 <= 50
    check!(!req.eval(&reader, 3)); // 51 > 50
    check!(!req.eval(&reader, 4)); // 100 > 50
    Ok(())
}

#[test]
fn test_predicate_value_eq() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueEq(42));
    let values = vec![41, 42, 43];
    let flags = vec![0; 3];
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    check!(req.eval(&reader, 1));
    check!(!req.eval(&reader, 2));
    Ok(())
}

#[test]
fn test_predicate_has_all_flags() -> TestResult {
    let req = Requirement::predicate(MockPredicate::HasAllFlags(FLAG_AB));
    let values = vec![0; 4];
    let flags = vec![
        0,       // None
        FLAG_A,  // Only A
        FLAG_B,  // Only B
        FLAG_AB, // Both A and B
    ];
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0)); // Missing both
    check!(!req.eval(&reader, 1)); // Missing B
    check!(!req.eval(&reader, 2)); // Missing A
    check!(req.eval(&reader, 3)); // Has both
    Ok(())
}

#[test]
fn test_predicate_has_any_flags() -> TestResult {
    let req = Requirement::predicate(MockPredicate::HasAnyFlags(FLAG_AB));
    let values = vec![0; 4];
    let flags = vec![
        0,      // None
        FLAG_A, // Only A
        FLAG_B, // Only B
        FLAG_C, // Only C (not in test set)
    ];
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0)); // Has none
    check!(req.eval(&reader, 1)); // Has A
    check!(req.eval(&reader, 2)); // Has B
    check!(!req.eval(&reader, 3)); // Has C but not A or B
    Ok(())
}

#[test]
fn test_predicate_has_none_flags() -> TestResult {
    let req = Requirement::predicate(MockPredicate::HasNoneFlags(FLAG_AB));
    let values = vec![0; 4];
    let flags = vec![
        0,       // None
        FLAG_A,  // Has A
        FLAG_C,  // Has C (not forbidden)
        FLAG_AB, // Has both A and B
    ];
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // Has none of forbidden
    check!(!req.eval(&reader, 1)); // Has A (forbidden)
    check!(req.eval(&reader, 2)); // Has C but not A or B
    check!(!req.eval(&reader, 3)); // Has both A and B
    Ok(())
}

#[test]
fn test_predicate_row_index_even() -> TestResult {
    let req = Requirement::predicate(MockPredicate::RowIndexEven);
    let values = vec![0; 5];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // Even
    check!(!req.eval(&reader, 1)); // Odd
    check!(req.eval(&reader, 2)); // Even
    check!(!req.eval(&reader, 3)); // Odd
    check!(req.eval(&reader, 4)); // Even
    Ok(())
}

#[test]
fn test_predicate_row_index_lt() -> TestResult {
    let req = Requirement::predicate(MockPredicate::RowIndexLt(3));
    let values = vec![0; 5];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    check!(req.eval(&reader, 1));
    check!(req.eval(&reader, 2));
    check!(!req.eval(&reader, 3));
    check!(!req.eval(&reader, 4));
    Ok(())
}

// ============================================================================
// SECTION: AND Evaluation Tests
// ============================================================================

#[test]
fn test_and_empty_trivially_satisfied() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::and(vec![]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    // Empty AND is trivially true (mathematical identity)
    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_single_true() -> TestResult {
    let req = Requirement::and(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_single_false() -> TestResult {
    let req = Requirement::and(vec![Requirement::predicate(MockPredicate::AlwaysFalse)]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_all_true() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_one_false() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_all_false() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_short_circuit_on_first_false() -> TestResult {
    // The first false should cause immediate return
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_and_with_value_predicates() -> TestResult {
    // Value must be >= 10 AND <= 20
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::ValueGte(10)),
        Requirement::predicate(MockPredicate::ValueLte(20)),
    ]);
    let values = vec![5, 10, 15, 20, 25];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0)); // 5: too low
    check!(req.eval(&reader, 1)); // 10: at lower bound
    check!(req.eval(&reader, 2)); // 15: in range
    check!(req.eval(&reader, 3)); // 20: at upper bound
    check!(!req.eval(&reader, 4)); // 25: too high
    Ok(())
}

// ============================================================================
// SECTION: OR Evaluation Tests
// ============================================================================

#[test]
fn test_or_empty_trivially_unsatisfied() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::or(vec![]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    // Empty OR is trivially false (no options)
    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_single_true() -> TestResult {
    let req = Requirement::or(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_single_false() -> TestResult {
    let req = Requirement::or(vec![Requirement::predicate(MockPredicate::AlwaysFalse)]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_all_true() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_one_true() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_all_false() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_short_circuit_on_first_true() -> TestResult {
    // The first true should cause immediate return
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_or_with_value_predicates() -> TestResult {
    // Value < 10 OR value > 90
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::ValueLte(10)),
        Requirement::predicate(MockPredicate::ValueGte(90)),
    ]);
    let values = vec![5, 10, 50, 90, 95];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // 5: low
    check!(req.eval(&reader, 1)); // 10: at low bound
    check!(!req.eval(&reader, 2)); // 50: middle
    check!(req.eval(&reader, 3)); // 90: at high bound
    check!(req.eval(&reader, 4)); // 95: high
    Ok(())
}

// ============================================================================
// SECTION: NOT Evaluation Tests
// ============================================================================

#[test]
fn test_not_true_becomes_false() -> TestResult {
    let req = Requirement::not(Requirement::predicate(MockPredicate::AlwaysTrue));
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_not_false_becomes_true() -> TestResult {
    let req = Requirement::not(Requirement::predicate(MockPredicate::AlwaysFalse));
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_not_double_negation() -> TestResult {
    let req = Requirement::not(Requirement::not(Requirement::predicate(MockPredicate::AlwaysTrue)));
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_not_with_value_predicate() -> TestResult {
    // NOT (value >= 50) is equivalent to value < 50
    let req = Requirement::not(Requirement::predicate(MockPredicate::ValueGte(50)));
    let values = vec![0, 49, 50, 51, 100];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // 0 < 50
    check!(req.eval(&reader, 1)); // 49 < 50
    check!(!req.eval(&reader, 2)); // 50 >= 50
    check!(!req.eval(&reader, 3)); // 51 >= 50
    check!(!req.eval(&reader, 4)); // 100 >= 50
    Ok(())
}

#[test]
fn test_not_and_becomes_nand() -> TestResult {
    // NOT(A AND B) is NAND
    let req = Requirement::not(Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]));
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // NOT(true AND false) = NOT(false) = true
    Ok(())
}

#[test]
fn test_not_or_becomes_nor() -> TestResult {
    // NOT(A OR B) is NOR
    let req = Requirement::not(Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]));
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // NOT(false OR false) = NOT(false) = true
    Ok(())
}

// ============================================================================
// SECTION: RequireGroup Evaluation Tests
// ============================================================================

#[test]
fn test_require_group_min_zero_always_satisfied() -> TestResult {
    let req = Requirement::require_group(
        0,
        vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    // min=0 means no requirements needed
    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_min_equals_total() -> TestResult {
    // Equivalent to AND when min == total
    let req = Requirement::require_group(
        3,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_min_one_equivalent_to_or() -> TestResult {
    // min=1 is equivalent to OR
    let req = Requirement::require_group(
        1,
        vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_exact_min_satisfied() -> TestResult {
    // Need exactly 2 out of 3
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_more_than_min_satisfied() -> TestResult {
    // Need 2 out of 3, have 3
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_less_than_min_satisfied() -> TestResult {
    // Need 2 out of 3, have 1
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_none_satisfied() -> TestResult {
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_early_success_exit() -> TestResult {
    // Should exit early once min is reached
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse), // Won't be evaluated
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_early_failure_exit() -> TestResult {
    // Should exit early when success is impossible
    let req = Requirement::require_group(
        3,
        vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysTrue), // Can't help now
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_require_group_with_value_predicates() -> TestResult {
    // Need at least 2 of: value >= 10, value >= 20, value >= 30
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::ValueGte(10)),
            Requirement::predicate(MockPredicate::ValueGte(20)),
            Requirement::predicate(MockPredicate::ValueGte(30)),
        ],
    );
    let values = vec![5, 15, 25, 35];
    let flags = vec![0; 4];
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0)); // 5: passes 0
    check!(!req.eval(&reader, 1)); // 15: passes 1
    check!(req.eval(&reader, 2)); // 25: passes 2
    check!(req.eval(&reader, 3)); // 35: passes 3
    Ok(())
}

// ============================================================================
// SECTION: Nested Requirement Tests
// ============================================================================

#[test]
fn test_nested_and_in_or() -> TestResult {
    // (A AND B) OR C
    let req = Requirement::or(vec![
        Requirement::and(vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysTrue),
        ]),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // (false AND true) OR true = false OR true = true
    Ok(())
}

#[test]
fn test_nested_or_in_and() -> TestResult {
    // A AND (B OR C)
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::AlwaysTrue),
        ]),
    ]);
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0)); // true AND (false OR true) = true AND true = true
    Ok(())
}

#[test]
fn test_deeply_nested() -> TestResult {
    // NOT(A AND (B OR (NOT C)))
    let req = Requirement::not(Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::not(Requirement::predicate(MockPredicate::AlwaysFalse)),
        ]),
    ]));
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    // NOT(true AND (false OR NOT(false)))
    // = NOT(true AND (false OR true))
    // = NOT(true AND true)
    // = NOT(true)
    // = false
    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_group_with_nested_requirements() -> TestResult {
    // At least 2 of: (A AND B), C, (D OR E)
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::and(vec![
                Requirement::predicate(MockPredicate::AlwaysTrue),
                Requirement::predicate(MockPredicate::AlwaysTrue),
            ]),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::or(vec![
                Requirement::predicate(MockPredicate::AlwaysFalse),
                Requirement::predicate(MockPredicate::AlwaysTrue),
            ]),
        ],
    );
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    // (true AND true)=true, false, (false OR true)=true
    // 2 out of 3 pass
    check!(req.eval(&reader, 0));
    Ok(())
}

// ============================================================================
// SECTION: Trivial Satisfaction Tests
// ============================================================================

#[test]
fn test_is_trivially_satisfied_empty_and() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::and(vec![]);
    check!(req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_trivially_satisfied_and_of_trivial() -> TestResult {
    let req: Requirement<MockPredicate> =
        Requirement::and(vec![Requirement::and(vec![]), Requirement::and(vec![])]);
    check!(req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_trivially_satisfied_or_of_trivial() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::or(vec![
        Requirement::and(vec![]),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    check!(req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_trivially_satisfied_not_of_unsatisfiable() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::not(Requirement::or(vec![]));
    check!(req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_trivially_satisfied_group_min_zero() -> TestResult {
    let req =
        Requirement::require_group(0, vec![Requirement::predicate(MockPredicate::AlwaysFalse)]);
    check!(req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_trivially_satisfied_group_enough_trivial() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::require_group(
        2,
        vec![
            Requirement::and(vec![]),
            Requirement::and(vec![]),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    check!(req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_not_trivially_satisfied_predicate() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);
    check!(!req.is_trivially_satisfied());
    Ok(())
}

#[test]
fn test_is_not_trivially_satisfied_and_with_predicate() -> TestResult {
    let req = Requirement::and(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    check!(!req.is_trivially_satisfied());
    Ok(())
}

// ============================================================================
// SECTION: Trivial Unsatisfiability Tests
// ============================================================================

#[test]
fn test_is_trivially_unsatisfiable_empty_or() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::or(vec![]);
    check!(req.is_trivially_unsatisfiable());
    Ok(())
}

#[test]
fn test_is_trivially_unsatisfiable_and_of_unsatisfiable() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::and(vec![
        Requirement::or(vec![]),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);
    check!(req.is_trivially_unsatisfiable());
    Ok(())
}

#[test]
fn test_is_trivially_unsatisfiable_or_of_all_unsatisfiable() -> TestResult {
    let req: Requirement<MockPredicate> =
        Requirement::or(vec![Requirement::or(vec![]), Requirement::or(vec![])]);
    check!(req.is_trivially_unsatisfiable());
    Ok(())
}

#[test]
fn test_is_trivially_unsatisfiable_not_of_satisfied() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::not(Requirement::and(vec![]));
    check!(req.is_trivially_unsatisfiable());
    Ok(())
}

#[test]
fn test_is_trivially_unsatisfiable_group_min_exceeds_total() -> TestResult {
    let req = Requirement::require_group(
        5,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
        ],
    );
    check!(req.is_trivially_unsatisfiable());
    Ok(())
}

#[test]
fn test_is_trivially_unsatisfiable_group_too_many_unsatisfiable() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::require_group(
        2,
        vec![
            Requirement::or(vec![]), // Trivially unsatisfiable
            Requirement::or(vec![]), // Trivially unsatisfiable
            Requirement::predicate(MockPredicate::AlwaysTrue),
        ],
    );
    check!(req.is_trivially_unsatisfiable());
    Ok(())
}

#[test]
fn test_is_not_trivially_unsatisfiable_predicate() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysFalse);
    check!(!req.is_trivially_unsatisfiable());
    Ok(())
}

// ============================================================================
// SECTION: Complexity Tests
// ============================================================================

#[test]
fn test_complexity_predicate() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);
    check_eq!(req.complexity(), 1);
    Ok(())
}

#[test]
fn test_complexity_not() -> TestResult {
    let req = Requirement::not(Requirement::predicate(MockPredicate::AlwaysTrue));
    check_eq!(req.complexity(), 2); // 1 for NOT + 1 for predicate
    Ok(())
}

#[test]
fn test_complexity_and() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    check_eq!(req.complexity(), 3); // 1 for AND + 2 for predicates
    Ok(())
}

#[test]
fn test_complexity_or() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::ValueGte(10)),
    ]);
    check_eq!(req.complexity(), 4); // 1 for OR + 3 for predicates
    Ok(())
}

#[test]
fn test_complexity_empty_and() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::and(vec![]);
    check_eq!(req.complexity(), 1); // Just the AND node
    Ok(())
}

#[test]
fn test_complexity_require_group() -> TestResult {
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );
    check_eq!(req.complexity(), 3); // 1 for group + 2 for predicates
    Ok(())
}

#[test]
fn test_complexity_nested() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ]),
        Requirement::not(Requirement::predicate(MockPredicate::ValueGte(10))),
    ]);
    // AND(1) + OR(1) + pred(1) + pred(1) + NOT(1) + pred(1) = 6
    check_eq!(req.complexity(), 6);
    Ok(())
}

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

#[test]
fn test_constructor_and() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    if let Requirement::And(reqs) = req {
        check_eq!(reqs.len(), 2);
        return Ok(());
    }
    Err("Expected And variant".into())
}

#[test]
fn test_constructor_or() -> TestResult {
    let req = Requirement::or(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    if let Requirement::Or(reqs) = req {
        check_eq!(reqs.len(), 1);
        return Ok(());
    }
    Err("Expected Or variant".into())
}

#[test]
fn test_constructor_not() -> TestResult {
    let req = Requirement::not(Requirement::predicate(MockPredicate::AlwaysTrue));
    if matches!(req, Requirement::Not(_)) {
        return Ok(());
    }
    Err("Expected Not variant".into())
}

#[test]
fn test_constructor_require_group() -> TestResult {
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::ValueGte(10)),
        ],
    );
    match req {
        Requirement::RequireGroup {
            min,
            reqs,
        } => {
            check_eq!(min, 2);
            check_eq!(reqs.len(), 3);
            Ok(())
        }
        _ => Err("Expected RequireGroup variant".into()),
    }
}

#[test]
fn test_constructor_predicate() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueEq(42));
    if matches!(req, Requirement::Predicate(MockPredicate::ValueEq(42))) {
        return Ok(());
    }
    Err("Expected Predicate(ValueEq(42)) variant".into())
}

// ============================================================================
// SECTION: Default Tests
// ============================================================================

#[test]
fn test_default_is_empty_and() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::default();
    if let Requirement::And(reqs) = req {
        check!(reqs.is_empty());
        return Ok(());
    }
    Err("Expected empty And variant".into())
}

#[test]
fn test_default_is_trivially_satisfied() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::default();
    check!(req.is_trivially_satisfied());
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    check!(req.eval(&reader, 0));
    Ok(())
}

// ============================================================================
// SECTION: RequirementGroup Tests
// ============================================================================

#[test]
fn test_requirement_group_new() -> TestResult {
    let group = RequirementGroup::new(
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
        1,
    )?;
    check_eq!(group.min_required, 1);
    check_eq!(group.requirements.len(), 2);
    Ok(())
}

#[test]
fn test_requirement_group_panics_on_invalid_min() -> TestResult {
    let result = RequirementGroup::new(
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
        3, // More than available
    );
    match result {
        Err(RequirementGroupError::MinExceedsCount {
            min_required,
            available,
        }) => {
            check_eq!(min_required, 3);
            check_eq!(available, 2);
        }
        Ok(_) => return Err("Expected failure when min exceeds available requirements".into()),
    }
    Ok(())
}

#[test]
fn test_requirement_group_all() -> TestResult {
    let group = RequirementGroup::all(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::ValueGte(10)),
    ]);
    check_eq!(group.min_required, 3);
    check_eq!(group.requirements.len(), 3);
    Ok(())
}

#[test]
fn test_requirement_group_any() -> TestResult {
    let group = RequirementGroup::any(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ])?;
    check_eq!(group.min_required, 1);
    check_eq!(group.requirements.len(), 2);
    Ok(())
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

#[test]
fn test_eval_out_of_bounds_row() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueGte(50));
    let values = vec![100];
    let flags = vec![0];
    let reader = MockReader::new(&values, &flags);

    // Row 10 is out of bounds - should return false safely
    check!(!req.eval(&reader, 10));
    Ok(())
}

#[test]
fn test_eval_empty_reader() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);
    let reader = MockReader::new(&[], &[]);

    // Even "always true" returns false for out-of-bounds with value checks
    // But AlwaysTrue doesn't check bounds, so it returns true
    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_many_nested_levels() -> TestResult {
    // Build a deeply nested requirement: NOT(NOT(NOT(NOT(true))))
    let mut req = Requirement::predicate(MockPredicate::AlwaysTrue);
    for _ in 0 .. 10 {
        req = Requirement::not(req);
    }

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    // 10 NOTs means result stays true (even number of inversions)
    check!(req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_large_and_requirement() -> TestResult {
    let reqs: Vec<_> =
        (0 .. 100).map(|_| Requirement::predicate(MockPredicate::AlwaysTrue)).collect();
    let req = Requirement::and(reqs);

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(req.eval(&reader, 0));
    check_eq!(req.complexity(), 101); // 1 for AND + 100 for predicates
    Ok(())
}

#[test]
fn test_large_or_requirement() -> TestResult {
    let reqs: Vec<_> =
        (0 .. 100).map(|_| Requirement::predicate(MockPredicate::AlwaysFalse)).collect();
    let req = Requirement::or(reqs);

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);

    check!(!req.eval(&reader, 0));
    Ok(())
}

#[test]
fn test_requirement_clone() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::ValueGte(10)),
        ]),
    ]);

    let cloned = req.clone();
    check_eq!(req, cloned);
    Ok(())
}

#[test]
fn test_requirement_equality() -> TestResult {
    let req1 = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let req2 = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let req3 = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);

    check_eq!(req1, req2);
    check_ne!(req1, req3); // Order matters
    Ok(())
}

// ============================================================================
// SECTION: Mask Evaluation Tests
// ============================================================================

fn eval_block_by_rows(
    req: &Requirement<MockPredicate>,
    reader: &MockReader<'_>,
    start: usize,
    count: usize,
) -> u64 {
    let n = count.min(64);
    let mut mask = 0u64;
    for i in 0 .. n {
        if req.eval(reader, start + i) {
            mask |= 1u64 << i;
        }
    }
    mask
}

#[test]
fn test_eval_block_matches_row_eval_for_compound_logic() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::not(Requirement::predicate(MockPredicate::AlwaysFalse)),
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::RowIndexEven),
            Requirement::predicate(MockPredicate::RowIndexLt(3)),
        ]),
    ]);

    let (values, flags) = (vec![0; 10], vec![0; 10]);
    let reader = MockReader::new(&values, &flags);

    let mask = req.eval_block(&reader, 0, 10);
    let expected = eval_block_by_rows(&req, &reader, 0, 10);
    check_eq!(mask, expected);
    Ok(())
}

#[test]
fn test_eval_block_matches_row_eval_for_require_group_threshold() -> TestResult {
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::HasAllFlags(FLAG_A)),
            Requirement::predicate(MockPredicate::RowIndexEven),
            Requirement::predicate(MockPredicate::RowIndexLt(2)),
        ],
    );

    let values = vec![0; 8];
    let flags = vec![0, FLAG_A, 0, FLAG_A, 0, FLAG_A, 0, FLAG_A];
    let reader = MockReader::new(&values, &flags);

    let mask = req.eval_block(&reader, 0, 8);
    let expected = eval_block_by_rows(&req, &reader, 0, 8);
    check_eq!(mask, expected);
    Ok(())
}

#[test]
fn test_eval_block_respects_start_and_count_window() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::RowIndexEven),
        Requirement::predicate(MockPredicate::RowIndexLt(5)),
    ]);

    let (values, flags) = (vec![0; 20], vec![0; 20]);
    let reader = MockReader::new(&values, &flags);

    let mask = req.eval_block(&reader, 3, 5);
    let expected = eval_block_by_rows(&req, &reader, 3, 5);
    check_eq!(mask, expected);
    Ok(())
}
