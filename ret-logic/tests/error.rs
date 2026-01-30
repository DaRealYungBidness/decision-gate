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

use ret_logic::RequirementError;
use support::TestResult;
use support::ensure;

/// Tests error creation.
#[test]
fn test_error_creation() -> TestResult {
    let err1 = RequirementError::condition_failed("Health too low");
    ensure(
        matches!(err1, RequirementError::ConditionFailed(_)),
        "Expected ConditionFailed for condition_failed helper",
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

/// Tests user messages.
#[test]
fn test_user_messages() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 2,
        required: 5,
    };
    let msg = err.user_message();
    ensure(msg.contains("3 more"), "Expected count delta in user message")?;

    let err = RequirementError::ConditionFailed("You need more strength".to_string());
    let msg = err.user_message();
    ensure(
        msg == "You need more strength",
        "Expected condition message to pass through unchanged",
    )?;
    Ok(())
}

/// Tests display.
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

/// Tests conversions.
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

/// Tests serialization.
#[test]
fn test_serialization() -> TestResult {
    let err = RequirementError::ConditionFailed("Test".to_string());
    let serialized = serde_json::to_string(&err)?;
    let deserialized: RequirementError = serde_json::from_str(&serialized)?;
    ensure(err == deserialized, "Expected serde roundtrip to preserve error")?;
    Ok(())
}
