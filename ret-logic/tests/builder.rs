// ret-logic/tests/builder.rs
// ============================================================================
// Module: Builder Tests
// Description: Tests for RequirementBuilder, AndBuilder, OrBuilder, GroupBuilder.
// Purpose: Ensure builder combinators emit the expected requirement trees.
// ============================================================================
//! ## Overview
//! Integration tests covering the builder helpers for composing requirements.

use std::ops::Not;

#[path = "support/mocks.rs"]
mod mocks;
mod support;

use mocks::MockPredicate;
use mocks::MockReader;
use ret_logic::Requirement;
use ret_logic::builder::AndBuilder;
use ret_logic::builder::GroupBuilder;
use ret_logic::builder::OrBuilder;
use ret_logic::builder::RequirementBuilder;
use ret_logic::builder::convenience;
use support::TestResult;
use support::ensure;

// ========================================================================
// SECTION: Mock Coverage
// ========================================================================

#[test]
fn test_mock_predicate_variants_used() {
    let _ = mocks::all_variants();
}

// ============================================================================
// SECTION: RequirementBuilder Tests
// ============================================================================

#[test]
fn test_requirement_builder_new() -> TestResult {
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);
    let builder = RequirementBuilder::new(req.clone());
    ensure(builder.build() == req, "Expected builder to return the original requirement")?;
    Ok(())
}

#[test]
fn test_requirement_builder_predicate() -> TestResult {
    let builder = RequirementBuilder::predicate(MockPredicate::ValueGte(50));
    let req = builder.build();
    match req {
        Requirement::Predicate(MockPredicate::ValueGte(50)) => Ok(()),
        _ => Err("Expected ValueGte predicate".into()),
    }
}

#[test]
fn test_requirement_builder_not() -> TestResult {
    let builder = RequirementBuilder::predicate(MockPredicate::AlwaysTrue);
    let req = builder.not().build();
    match req {
        Requirement::Not(_) => Ok(()),
        _ => Err("Expected Not variant".into()),
    }
}

#[test]
fn test_requirement_builder_double_not() -> TestResult {
    let builder = RequirementBuilder::predicate(MockPredicate::AlwaysTrue);
    let req = builder.not().not().build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected double NOT to cancel out")?;
    Ok(())
}

#[test]
fn test_requirement_builder_and_also() -> TestResult {
    let builder = RequirementBuilder::predicate(MockPredicate::AlwaysTrue);
    let req = builder.and_also(Requirement::predicate(MockPredicate::AlwaysTrue)).build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected AND to evaluate to true")?;
    Ok(())
}

#[test]
fn test_requirement_builder_or_else() -> TestResult {
    let builder = RequirementBuilder::predicate(MockPredicate::AlwaysFalse);
    let req = builder.or_else(Requirement::predicate(MockPredicate::AlwaysTrue)).build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected OR to evaluate to true")?;
    Ok(())
}

#[test]
fn test_requirement_builder_chaining() -> TestResult {
    let req = RequirementBuilder::predicate(MockPredicate::AlwaysTrue)
        .and_also(Requirement::predicate(MockPredicate::AlwaysTrue))
        .or_else(Requirement::predicate(MockPredicate::AlwaysFalse))
        .not()
        .build();

    // NOT((true AND true) OR false) = NOT(true) = false
    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(!req.eval(&reader, 0), "Expected chained builder to evaluate to false")?;
    Ok(())
}

// ============================================================================
// SECTION: AndBuilder Tests
// ============================================================================

#[test]
fn test_and_builder_new() -> TestResult {
    let builder = AndBuilder::<MockPredicate>::new();
    let req = builder.build();
    match req {
        Requirement::And(reqs) => {
            ensure(reqs.is_empty(), "Expected empty And builder to contain no requirements")?;
            Ok(())
        }
        _ => Err("Expected empty And".into()),
    }
}

#[test]
fn test_and_builder_default() -> TestResult {
    let builder = AndBuilder::<MockPredicate>::default();
    let req = builder.build();
    match req {
        Requirement::And(reqs) => {
            ensure(reqs.is_empty(), "Expected default And builder to contain no requirements")?;
            Ok(())
        }
        _ => Err("Expected empty And".into()),
    }
}

#[test]
fn test_and_builder_with() -> TestResult {
    let builder = AndBuilder::new().with(Requirement::predicate(MockPredicate::AlwaysTrue));
    let req = builder.build();
    match req {
        Requirement::And(reqs) => {
            ensure(reqs.len() == 1, "Expected And builder to contain one requirement")?;
            Ok(())
        }
        _ => Err("Expected And with one element".into()),
    }
}

#[test]
fn test_and_builder_with_predicate() -> TestResult {
    let builder = AndBuilder::new()
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysFalse);
    let req = builder.build();
    match req {
        Requirement::And(reqs) => {
            ensure(reqs.len() == 2, "Expected And builder to contain two requirements")?;
            Ok(())
        }
        _ => Err("Expected And with two elements".into()),
    }
}

#[test]
fn test_and_builder_with_all() -> TestResult {
    let reqs = vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::ValueGte(10)),
    ];
    let builder = AndBuilder::new().with_all(reqs);
    let req = builder.build();
    match req {
        Requirement::And(reqs) => {
            ensure(reqs.len() == 3, "Expected And builder to contain three requirements")?;
            Ok(())
        }
        _ => Err("Expected And with three elements".into()),
    }
}

#[test]
fn test_and_builder_chaining() -> TestResult {
    let req = AndBuilder::new()
        .with_predicate(MockPredicate::ValueGte(10))
        .with(Requirement::or(vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ]))
        .with_predicate(MockPredicate::HasAllFlags(0b11))
        .build();

    ensure(req.complexity() == 6, "Expected chained And builder complexity to match")?;
    Ok(())
}

#[test]
fn test_and_builder_from_static_method() -> TestResult {
    let builder = RequirementBuilder::<MockPredicate>::and();
    let req = builder.with_predicate(MockPredicate::AlwaysTrue).build();
    match req {
        Requirement::And(reqs) => {
            ensure(reqs.len() == 1, "Expected And builder to contain one requirement")?;
            Ok(())
        }
        _ => Err("Expected And".into()),
    }
}

// ============================================================================
// SECTION: OrBuilder Tests
// ============================================================================

#[test]
fn test_or_builder_new() -> TestResult {
    let builder = OrBuilder::<MockPredicate>::new();
    let req = builder.build();
    match req {
        Requirement::Or(reqs) => {
            ensure(reqs.is_empty(), "Expected empty Or builder to contain no requirements")?;
            Ok(())
        }
        _ => Err("Expected empty Or".into()),
    }
}

#[test]
fn test_or_builder_default() -> TestResult {
    let builder = OrBuilder::<MockPredicate>::default();
    let req = builder.build();
    match req {
        Requirement::Or(reqs) => {
            ensure(reqs.is_empty(), "Expected default Or builder to contain no requirements")?;
            Ok(())
        }
        _ => Err("Expected empty Or".into()),
    }
}

#[test]
fn test_or_builder_with() -> TestResult {
    let builder = OrBuilder::new().with(Requirement::predicate(MockPredicate::AlwaysTrue));
    let req = builder.build();
    match req {
        Requirement::Or(reqs) => {
            ensure(reqs.len() == 1, "Expected Or builder to contain one requirement")?;
            Ok(())
        }
        _ => Err("Expected Or with one element".into()),
    }
}

#[test]
fn test_or_builder_with_predicate() -> TestResult {
    let builder = OrBuilder::new()
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysFalse)
        .with_predicate(MockPredicate::ValueGte(10));
    let req = builder.build();
    match req {
        Requirement::Or(reqs) => {
            ensure(reqs.len() == 3, "Expected Or builder to contain three requirements")?;
            Ok(())
        }
        _ => Err("Expected Or with three elements".into()),
    }
}

#[test]
fn test_or_builder_with_all() -> TestResult {
    let reqs = vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ];
    let builder = OrBuilder::new().with_all(reqs);
    let req = builder.build();
    match req {
        Requirement::Or(reqs) => {
            ensure(reqs.len() == 2, "Expected Or builder to contain two requirements")?;
            Ok(())
        }
        _ => Err("Expected Or with two elements".into()),
    }
}

#[test]
fn test_or_builder_from_static_method() -> TestResult {
    let builder = RequirementBuilder::<MockPredicate>::or();
    let req = builder
        .with_predicate(MockPredicate::AlwaysFalse)
        .with_predicate(MockPredicate::AlwaysTrue)
        .build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected OR builder result to evaluate to true")?;
    Ok(())
}

// ============================================================================
// SECTION: GroupBuilder Tests
// ============================================================================

#[test]
fn test_group_builder_new() -> TestResult {
    let builder = GroupBuilder::<MockPredicate>::new(2);
    let req = builder.build();
    match req {
        Requirement::RequireGroup {
            min,
            reqs,
        } => {
            ensure(min == 2, "Expected RequireGroup min to match constructor")?;
            ensure(reqs.is_empty(), "Expected RequireGroup to start empty")?;
            Ok(())
        }
        _ => Err("Expected RequireGroup".into()),
    }
}

#[test]
fn test_group_builder_with() -> TestResult {
    let builder = GroupBuilder::new(1)
        .with(Requirement::predicate(MockPredicate::AlwaysTrue))
        .with(Requirement::predicate(MockPredicate::AlwaysFalse));
    let req = builder.build();
    match req {
        Requirement::RequireGroup {
            min,
            reqs,
        } => {
            ensure(min == 1, "Expected RequireGroup min to match builder")?;
            ensure(reqs.len() == 2, "Expected RequireGroup to contain two requirements")?;
            Ok(())
        }
        _ => Err("Expected RequireGroup".into()),
    }
}

#[test]
fn test_group_builder_with_predicate() -> TestResult {
    let builder = GroupBuilder::new(2)
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysFalse);
    let req = builder.build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected group builder predicate to pass (2 of 3)")?;
    Ok(())
}

#[test]
fn test_group_builder_with_all() -> TestResult {
    let reqs = vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::ValueGte(10)),
    ];
    let builder = GroupBuilder::new(1).with_all(reqs);
    let req = builder.build();
    match req {
        Requirement::RequireGroup {
            min,
            reqs,
        } => {
            ensure(min == 1, "Expected RequireGroup min to be 1")?;
            ensure(reqs.len() == 3, "Expected RequireGroup to contain three requirements")?;
            Ok(())
        }
        _ => Err("Expected RequireGroup".into()),
    }
}

#[test]
fn test_group_builder_min_update() -> TestResult {
    let builder = GroupBuilder::new(1)
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysTrue)
        .min(2);
    let req = builder.build();
    match req {
        Requirement::RequireGroup {
            min, ..
        } => {
            ensure(min == 2, "Expected RequireGroup min to update to 2")?;
            Ok(())
        }
        _ => Err("Expected RequireGroup".into()),
    }
}

#[test]
fn test_group_builder_from_static_method() -> TestResult {
    let builder = RequirementBuilder::<MockPredicate>::require_group(2);
    let req = builder
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysTrue)
        .with_predicate(MockPredicate::AlwaysFalse)
        .build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected RequireGroup builder to evaluate to true")?;
    Ok(())
}

// ============================================================================
// SECTION: Convenience Function Tests
// ============================================================================

#[test]
fn test_convenience_all() -> TestResult {
    let req = convenience::all(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected convenience::all to evaluate to true")?;
    Ok(())
}

#[test]
fn test_convenience_any() -> TestResult {
    let req = convenience::any(vec![
        Requirement::predicate(MockPredicate::AlwaysFalse),
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ]);

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected convenience::any to evaluate to true")?;
    Ok(())
}

#[test]
fn test_convenience_not() -> TestResult {
    let req = convenience::not(Requirement::predicate(MockPredicate::AlwaysFalse));

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected convenience::not to invert predicate")?;
    Ok(())
}

#[test]
fn test_convenience_at_least() -> TestResult {
    let req = convenience::at_least(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
        ],
    );

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected convenience::at_least to evaluate to true")?;
    Ok(())
}

#[test]
fn test_convenience_predicate() -> TestResult {
    let req = convenience::predicate(MockPredicate::ValueEq(42));
    let values = vec![42];
    let flags = vec![0];
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected convenience::predicate to evaluate to true")?;
    Ok(())
}

// ============================================================================
// SECTION: Complex Builder Pattern Tests
// ============================================================================

#[test]
fn test_complex_nested_builders() -> TestResult {
    // (A AND B) OR (C AND D)
    let req = OrBuilder::new()
        .with(
            AndBuilder::new()
                .with_predicate(MockPredicate::ValueGte(10))
                .with_predicate(MockPredicate::ValueLte(20))
                .build(),
        )
        .with(
            AndBuilder::new()
                .with_predicate(MockPredicate::ValueGte(80))
                .with_predicate(MockPredicate::ValueLte(90))
                .build(),
        )
        .build();

    let values = vec![5, 15, 50, 85, 95];
    let flags = vec![0; 5];
    let reader = MockReader::new(&values, &flags);

    ensure(!req.eval(&reader, 0), "Expected 5 to be outside all ranges")?;
    ensure(req.eval(&reader, 1), "Expected 15 to be within [10, 20]")?;
    ensure(!req.eval(&reader, 2), "Expected 50 to be outside all ranges")?;
    ensure(req.eval(&reader, 3), "Expected 85 to be within [80, 90]")?;
    ensure(!req.eval(&reader, 4), "Expected 95 to be outside all ranges")?;
    Ok(())
}

#[test]
fn test_builder_with_groups() -> TestResult {
    // Need at least 2 of: (A OR B), C, D
    let req = GroupBuilder::new(2)
        .with(
            OrBuilder::new()
                .with_predicate(MockPredicate::AlwaysFalse)
                .with_predicate(MockPredicate::AlwaysTrue)
                .build(),
        )
        .with_predicate(MockPredicate::AlwaysFalse)
        .with_predicate(MockPredicate::AlwaysTrue)
        .build();

    let (values, flags) = (vec![0], vec![0]);
    let reader = MockReader::new(&values, &flags);
    ensure(req.eval(&reader, 0), "Expected grouped builder to meet min pass count")?;
    Ok(())
}
