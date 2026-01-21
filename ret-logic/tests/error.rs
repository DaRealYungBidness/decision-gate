// ret-logic/tests/error.rs
// ============================================================================
// Module: Requirement Error Tests
// Description: Regression coverage for `RequirementError` behaviors.
// Purpose: Ensure the error constructors, conversions, display, and serialization
//          are stable and provide actionable diagnostics.
// Dependencies: serde_json (for round-trip verification), ret_logic::error
// ============================================================================
//! ## Overview
//! Integration tests for requirement error diagnostics.
//! These tests exercise the documented helpers on [`RequirementError`] to guarantee
//! that user-facing messaging, conversions, and serialization contracts remain
//! predictable for downstream consumers.

// ============================================================================
// SECTION: Test Support
// ============================================================================

mod support;

use ret_logic::RequirementError;
use support::TestResult;
use support::ensure;

#[test]
fn test_error_creation() -> TestResult {
    let err1 = RequirementError::predicate_failed("Health too low");
    ensure(
        matches!(err1, RequirementError::PredicateFailed(_)),
        "Expected PredicateFailed for predicate_failed helper",
    )?;

    let err2 = RequirementError::other("Custom error");
    ensure(matches!(err2, RequirementError::Other(_)), "Expected Other for other helper")?;

    let err3 = RequirementError::GroupRequirementFailed {
        passed: 2,
        required: 3,
    };
    ensure(
        matches!(err3, RequirementError::GroupRequirementFailed { .. }),
        "Expected GroupRequirementFailed variant",
    )?;
    Ok(())
}

#[test]
fn test_user_messages() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 2,
        required: 5,
    };
    let msg = err.user_message();
    ensure(msg.contains("3 more"), "Expected count delta in user message")?;

    let err = RequirementError::PredicateFailed("You need more strength".to_string());
    let msg = err.user_message();
    ensure(
        msg == "You need more strength",
        "Expected predicate message to pass through unchanged",
    )?;
    Ok(())
}

#[test]
fn test_display() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 1,
        required: 3,
    };
    let display = format!("{err}");
    ensure(display.contains("passed 1"), "Expected display to include passed count")?;
    ensure(display.contains("needed 3"), "Expected display to include needed count")?;
    Ok(())
}

#[test]
fn test_conversions() -> TestResult {
    let err: RequirementError = "Test error".into();
    ensure(matches!(err, RequirementError::Other(_)), "Expected &str conversion to map to Other")?;

    let err: RequirementError = "Test error".to_string().into();
    ensure(
        matches!(err, RequirementError::Other(_)),
        "Expected String conversion to map to Other",
    )?;
    Ok(())
}

#[test]
fn test_serialization() -> TestResult {
    let err = RequirementError::PredicateFailed("Test".to_string());
    let serialized = serde_json::to_string(&err)?;
    let deserialized: RequirementError = serde_json::from_str(&serialized)?;
    ensure(err == deserialized, "Expected serde roundtrip to preserve error")?;
    Ok(())
}
