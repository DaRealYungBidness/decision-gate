// ret-logic/tests/dsl.rs
// ============================================================================
// Test Module: Requirement DSL
// Coverage: Happy-path parsing, precedence, group handling, and error cases.
// ============================================================================
//! ## Overview
//! Integration tests for the requirement DSL parser.

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

use std::collections::HashMap;
use std::fmt;

use ret_logic::Requirement;
use ret_logic::dsl::DslError;
use ret_logic::dsl::parse_requirement;
use support::TestResult;
use support::ensure;

// ========================================================================
// Test Error Helpers
// ========================================================================

/// Error type used for DSL test failures.
#[derive(Debug)]
struct DslTestError {
    /// Failure message describing the mismatch.
    message: String,
}

impl fmt::Display for DslTestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DslTestError {}

/// Returns a formatted test failure.
fn fail<T>(message: impl Into<String>) -> TestResult<T> {
    Err(Box::new(DslTestError {
        message: message.into(),
    }))
}

/// Builds a simple predicate resolver for DSL tests.
fn resolver() -> HashMap<String, u8> {
    let mut map = HashMap::new();
    map.insert("is_alive".to_string(), 1);
    map.insert("has_ap".to_string(), 2);
    map.insert("stunned".to_string(), 3);
    map.insert("in_range".to_string(), 4);
    map
}

/// Tests parses nested boolean expression.
#[test]
fn parses_nested_boolean_expression() -> TestResult {
    let Ok(req) = parse_requirement("all(is_alive, any(has_ap, not stunned))", &resolver()) else {
        return fail("Expected parse success");
    };

    let expected = Requirement::and(vec![
        Requirement::predicate(1),
        Requirement::or(vec![
            Requirement::predicate(2),
            Requirement::not(Requirement::predicate(3)),
        ]),
    ]);

    ensure(req == expected, "Expected nested boolean expression to parse correctly")?;
    Ok(())
}

/// Tests respects operator precedence.
#[test]
fn respects_operator_precedence() -> TestResult {
    let Ok(req) = parse_requirement("is_alive && has_ap || not stunned", &resolver()) else {
        return fail("Expected parse success");
    };

    let expected = Requirement::or(vec![
        Requirement::and(vec![Requirement::predicate(1), Requirement::predicate(2)]),
        Requirement::not(Requirement::predicate(3)),
    ]);

    ensure(req == expected, "Expected operator precedence to match infix rules")?;
    Ok(())
}

/// Tests parses group with count.
#[test]
fn parses_group_with_count() -> TestResult {
    let Ok(req) = parse_requirement("at_least(2, is_alive, has_ap, in_range)", &resolver()) else {
        return fail("Expected parse success");
    };

    let expected = Requirement::require_group(
        2,
        vec![Requirement::predicate(1), Requirement::predicate(2), Requirement::predicate(4)],
    );

    ensure(req == expected, "Expected group count parsing to match DSL")?;
    Ok(())
}

/// Tests errors on unknown predicate.
#[test]
fn errors_on_unknown_predicate() -> TestResult {
    let Err(err) = parse_requirement::<u8, _>("unknown_pred", &resolver()) else {
        return fail("Expected unknown predicate error");
    };
    ensure(
        matches!(err, DslError::UnknownPredicate { name, .. } if name == "unknown_pred"),
        "Expected unknown predicate diagnostic with predicate name",
    )?;
    Ok(())
}

/// Tests errors on unknown function with args.
#[test]
fn errors_on_unknown_function_with_args() -> TestResult {
    let Err(err) = parse_requirement::<u8, _>("foo(is_alive)", &resolver()) else {
        return fail("Expected unknown function error");
    };
    ensure(
        matches!(err, DslError::UnknownFunction { name, .. } if name == "foo"),
        "Expected unknown function diagnostic with function name",
    )?;
    Ok(())
}

/// Tests errors on trailing input.
#[test]
fn errors_on_trailing_input() -> TestResult {
    let Err(err) = parse_requirement::<u8, _>("is_alive extra", &resolver()) else {
        return fail("Expected trailing input error");
    };
    ensure(matches!(err, DslError::TrailingInput { .. }), "Expected trailing input diagnostic")?;
    Ok(())
}

/// Tests validation error when group min exceeds total.
#[test]
fn validation_error_when_group_min_exceeds_total() -> TestResult {
    let Err(err) = parse_requirement::<u8, _>("at_least(2, is_alive)", &resolver()) else {
        return fail("Expected validation error for invalid group min");
    };
    ensure(
        matches!(err, DslError::Validation(msg) if msg.contains("Invalid group")),
        "Expected validation error for invalid group min",
    )?;
    Ok(())
}

/// Tests errors on empty input.
#[test]
fn errors_on_empty_input() -> TestResult {
    let Err(err) = parse_requirement::<u8, _>("   ", &resolver()) else {
        return fail("Expected empty input error");
    };
    ensure(matches!(err, DslError::EmptyInput), "Expected empty input diagnostic")?;
    Ok(())
}
