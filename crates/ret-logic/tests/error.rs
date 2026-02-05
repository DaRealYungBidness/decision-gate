// crates/ret-logic/tests/error.rs
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

// ============================================================================
// SECTION: Display Output Verification for Every Variant
// ============================================================================

#[test]
fn display_group_requirement_failed() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 2,
        required: 5,
    };
    let display = format!("{}", err);
    ensure(display.contains("passed 2"), "Should contain passed count")?;
    ensure(display.contains("needed 5"), "Should contain required count")?;
    ensure(display.contains("Group requirement failed"), "Should contain variant prefix")?;
    Ok(())
}

#[test]
fn display_or_all_failed() -> TestResult {
    let err = RequirementError::OrAllFailed;
    let display = format!("{}", err);
    ensure(
        display == "All alternatives in OR requirement failed",
        format!("Unexpected display: {}", display),
    )?;
    Ok(())
}

#[test]
fn display_not_failed() -> TestResult {
    let err = RequirementError::NotFailed;
    let display = format!("{}", err);
    ensure(
        display == "NOT requirement failed: inner requirement was satisfied",
        format!("Unexpected display: {}", display),
    )?;
    Ok(())
}

#[test]
fn display_subject_not_available() -> TestResult {
    let err = RequirementError::SubjectNotAvailable;
    let display = format!("{}", err);
    ensure(
        display == "Subject not available in evaluation context",
        format!("Unexpected display: {}", display),
    )?;
    Ok(())
}

#[test]
fn display_target_not_available() -> TestResult {
    let err = RequirementError::TargetNotAvailable;
    let display = format!("{}", err);
    ensure(
        display == "Target not available in evaluation context",
        format!("Unexpected display: {}", display),
    )?;
    Ok(())
}

#[test]
fn display_world_state_unavailable() -> TestResult {
    let err = RequirementError::WorldStateUnavailable;
    let display = format!("{}", err);
    ensure(
        display == "World state unavailable or inaccessible",
        format!("Unexpected display: {}", display),
    )?;
    Ok(())
}

#[test]
fn display_condition_failed() -> TestResult {
    let err = RequirementError::ConditionFailed("Health below 50".to_string());
    let display = format!("{}", err);
    ensure(display.contains("Requirement not met"), "Should have prefix")?;
    ensure(display.contains("Health below 50"), "Should contain message")?;
    Ok(())
}

#[test]
fn display_condition_error() -> TestResult {
    let err = RequirementError::ConditionError("Component missing".to_string());
    let display = format!("{}", err);
    ensure(display.contains("Condition evaluation error"), "Should have prefix")?;
    ensure(display.contains("Component missing"), "Should contain message")?;
    Ok(())
}

#[test]
fn display_invalid_structure() -> TestResult {
    let err = RequirementError::InvalidStructure("Empty AND clause".to_string());
    let display = format!("{}", err);
    ensure(display.contains("Invalid requirement structure"), "Should have prefix")?;
    ensure(display.contains("Empty AND clause"), "Should contain message")?;
    Ok(())
}

#[test]
fn display_too_deep() -> TestResult {
    let err = RequirementError::TooDeep {
        max_depth: 100,
        actual_depth: 150,
    };
    let display = format!("{}", err);
    ensure(display.contains("150 levels"), "Should contain actual depth")?;
    ensure(display.contains("max 100"), "Should contain max depth")?;
    ensure(display.contains("too deep"), "Should mention depth issue")?;
    Ok(())
}

#[test]
fn display_other() -> TestResult {
    let err = RequirementError::Other("Custom error message".to_string());
    let display = format!("{}", err);
    ensure(display.contains("Requirement error"), "Should have prefix")?;
    ensure(display.contains("Custom error message"), "Should contain message")?;
    Ok(())
}

// ============================================================================
// SECTION: user_message() Output Verification for Every Variant
// ============================================================================

#[test]
fn user_message_group_requirement_failed_plural() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 1,
        required: 5,
    };
    let msg = err.user_message();
    ensure(msg.contains("4 more requirements"), "Should show 4 more (plural)")?;
    Ok(())
}

#[test]
fn user_message_group_requirement_failed_singular() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 4,
        required: 5,
    };
    let msg = err.user_message();
    ensure(msg.contains("1 more requirement"), "Should show 1 more")?;
    ensure(!msg.contains("requirements"), "Should be singular")?;
    Ok(())
}

#[test]
fn user_message_group_requirement_failed_zero_remaining() -> TestResult {
    // Edge case: passed == required (shouldn't normally error, but test display)
    let err = RequirementError::GroupRequirementFailed {
        passed: 5,
        required: 5,
    };
    let msg = err.user_message();
    ensure(msg.contains("0 more"), "Should show 0 more")?;
    ensure(msg.contains("requirements"), "0 should be plural")?;
    Ok(())
}

#[test]
fn user_message_group_requirement_failed_saturating() -> TestResult {
    // Edge case: passed > required (shouldn't happen, but test saturating_sub)
    let err = RequirementError::GroupRequirementFailed {
        passed: 10,
        required: 5,
    };
    let msg = err.user_message();
    ensure(msg.contains("0 more"), "Saturating should give 0")?;
    Ok(())
}

#[test]
fn user_message_or_all_failed() -> TestResult {
    let err = RequirementError::OrAllFailed;
    let msg = err.user_message();
    ensure(
        msg == "None of the alternative requirements were met",
        format!("Unexpected message: {}", msg),
    )?;
    Ok(())
}

#[test]
fn user_message_not_failed() -> TestResult {
    let err = RequirementError::NotFailed;
    let msg = err.user_message();
    ensure(
        msg == "A condition that should not be true was satisfied",
        format!("Unexpected message: {}", msg),
    )?;
    Ok(())
}

#[test]
fn user_message_subject_not_available() -> TestResult {
    let err = RequirementError::SubjectNotAvailable;
    let msg = err.user_message();
    ensure(
        msg == "Cannot evaluate requirement: no subject available",
        format!("Unexpected message: {}", msg),
    )?;
    Ok(())
}

#[test]
fn user_message_target_not_available() -> TestResult {
    let err = RequirementError::TargetNotAvailable;
    let msg = err.user_message();
    ensure(
        msg == "Cannot evaluate requirement: no target available",
        format!("Unexpected message: {}", msg),
    )?;
    Ok(())
}

#[test]
fn user_message_world_state_unavailable() -> TestResult {
    let err = RequirementError::WorldStateUnavailable;
    let msg = err.user_message();
    ensure(
        msg == "Cannot evaluate requirement: world state unavailable",
        format!("Unexpected message: {}", msg),
    )?;
    Ok(())
}

#[test]
fn user_message_condition_failed_passthrough() -> TestResult {
    let err = RequirementError::ConditionFailed("You need 50 gold".to_string());
    let msg = err.user_message();
    ensure(msg == "You need 50 gold", "ConditionFailed should pass through message unchanged")?;
    Ok(())
}

#[test]
fn user_message_condition_error_generic() -> TestResult {
    let err = RequirementError::ConditionError("Stack overflow".to_string());
    let msg = err.user_message();
    ensure(
        msg == "An internal error occurred while checking requirements",
        "ConditionError should return generic message",
    )?;
    Ok(())
}

#[test]
fn user_message_invalid_structure_generic() -> TestResult {
    let err = RequirementError::InvalidStructure("Recursive definition".to_string());
    let msg = err.user_message();
    ensure(
        msg == "Invalid requirement configuration",
        "InvalidStructure should return generic message",
    )?;
    Ok(())
}

#[test]
fn user_message_too_deep_generic() -> TestResult {
    let err = RequirementError::TooDeep {
        max_depth: 50,
        actual_depth: 100,
    };
    let msg = err.user_message();
    ensure(msg == "Requirement too complex to evaluate", "TooDeep should return generic message")?;
    Ok(())
}

#[test]
fn user_message_other_prefixed() -> TestResult {
    let err = RequirementError::Other("Custom reason".to_string());
    let msg = err.user_message();
    ensure(msg == "Requirement not met: Custom reason", format!("Unexpected message: {}", msg))?;
    Ok(())
}

// ============================================================================
// SECTION: Boundary Value Tests
// ============================================================================

#[test]
fn group_requirement_boundary_passed_zero() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 0,
        required: 1,
    };
    let msg = err.user_message();
    ensure(msg.contains("1 more requirement"), "passed=0, required=1 should need 1")?;
    ensure(!msg.contains("requirements"), "Should be singular")?;
    Ok(())
}

#[test]
fn group_requirement_boundary_required_zero() -> TestResult {
    // Weird case: required=0 should never error, but test the display
    let err = RequirementError::GroupRequirementFailed {
        passed: 0,
        required: 0,
    };
    let msg = err.user_message();
    ensure(msg.contains("0 more"), "Should handle required=0")?;
    Ok(())
}

#[test]
fn group_requirement_boundary_large_numbers() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 999,
        required: 1000,
    };
    let msg = err.user_message();
    ensure(msg.contains("1 more requirement"), "Large numbers should work")?;

    let err = RequirementError::GroupRequirementFailed {
        passed: 0,
        required: usize::MAX,
    };
    let msg = err.user_message();
    // Should not panic with saturating arithmetic
    ensure(!msg.is_empty(), "Should handle usize::MAX")?;
    Ok(())
}

#[test]
fn too_deep_boundary_values() -> TestResult {
    // Zero depths
    let err = RequirementError::TooDeep {
        max_depth: 0,
        actual_depth: 1,
    };
    let display = format!("{}", err);
    ensure(display.contains("1 levels"), "Should show actual_depth")?;
    ensure(display.contains("max 0"), "Should show max_depth")?;

    // Equal depths (edge case)
    let err = RequirementError::TooDeep {
        max_depth: 100,
        actual_depth: 100,
    };
    let display = format!("{}", err);
    ensure(display.contains("100 levels"), "Should handle equal depths")?;

    // Large depths
    let err = RequirementError::TooDeep {
        max_depth: usize::MAX,
        actual_depth: usize::MAX,
    };
    let display = format!("{}", err);
    ensure(!display.is_empty(), "Should handle usize::MAX")?;
    Ok(())
}

// ============================================================================
// SECTION: Serialization Roundtrip Tests for All Variants
// ============================================================================

#[test]
fn serde_roundtrip_group_requirement_failed() -> TestResult {
    let err = RequirementError::GroupRequirementFailed {
        passed: 42,
        required: 100,
    };
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve GroupRequirementFailed")?;
    Ok(())
}

#[test]
fn serde_roundtrip_or_all_failed() -> TestResult {
    let err = RequirementError::OrAllFailed;
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve OrAllFailed")?;
    Ok(())
}

#[test]
fn serde_roundtrip_not_failed() -> TestResult {
    let err = RequirementError::NotFailed;
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve NotFailed")?;
    Ok(())
}

#[test]
fn serde_roundtrip_subject_not_available() -> TestResult {
    let err = RequirementError::SubjectNotAvailable;
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve SubjectNotAvailable")?;
    Ok(())
}

#[test]
fn serde_roundtrip_target_not_available() -> TestResult {
    let err = RequirementError::TargetNotAvailable;
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve TargetNotAvailable")?;
    Ok(())
}

#[test]
fn serde_roundtrip_world_state_unavailable() -> TestResult {
    let err = RequirementError::WorldStateUnavailable;
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve WorldStateUnavailable")?;
    Ok(())
}

#[test]
fn serde_roundtrip_condition_failed() -> TestResult {
    let err = RequirementError::ConditionFailed("test message".to_string());
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve ConditionFailed")?;
    Ok(())
}

#[test]
fn serde_roundtrip_condition_error() -> TestResult {
    let err = RequirementError::ConditionError("test error".to_string());
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve ConditionError")?;
    Ok(())
}

#[test]
fn serde_roundtrip_invalid_structure() -> TestResult {
    let err = RequirementError::InvalidStructure("bad structure".to_string());
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve InvalidStructure")?;
    Ok(())
}

#[test]
fn serde_roundtrip_too_deep() -> TestResult {
    let err = RequirementError::TooDeep {
        max_depth: 50,
        actual_depth: 75,
    };
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve TooDeep")?;
    Ok(())
}

#[test]
fn serde_roundtrip_other() -> TestResult {
    let err = RequirementError::Other("generic error".to_string());
    let json = serde_json::to_string(&err)?;
    let back: RequirementError = serde_json::from_str(&json)?;
    ensure(err == back, "Roundtrip should preserve Other")?;
    Ok(())
}

// ============================================================================
// SECTION: Helper Method Tests
// ============================================================================

#[test]
fn helper_condition_failed() -> TestResult {
    let err = RequirementError::condition_failed("test");
    ensure(
        matches!(err, RequirementError::ConditionFailed(msg) if msg == "test"),
        "condition_failed should create ConditionFailed variant",
    )?;
    Ok(())
}

#[test]
fn helper_condition_error() -> TestResult {
    let err = RequirementError::condition_error("test");
    ensure(
        matches!(err, RequirementError::ConditionError(msg) if msg == "test"),
        "condition_error should create ConditionError variant",
    )?;
    Ok(())
}

#[test]
fn helper_other() -> TestResult {
    let err = RequirementError::other("test");
    ensure(
        matches!(err, RequirementError::Other(msg) if msg == "test"),
        "other should create Other variant",
    )?;
    Ok(())
}

#[test]
fn helper_invalid_structure() -> TestResult {
    let err = RequirementError::invalid_structure("test");
    ensure(
        matches!(err, RequirementError::InvalidStructure(msg) if msg == "test"),
        "invalid_structure should create InvalidStructure variant",
    )?;
    Ok(())
}

#[test]
fn conversion_from_string() -> TestResult {
    let err: RequirementError = "test error".to_string().into();
    ensure(
        matches!(err, RequirementError::Other(msg) if msg == "test error"),
        "String should convert to Other",
    )?;
    Ok(())
}

#[test]
fn conversion_from_str() -> TestResult {
    let err: RequirementError = "test error".into();
    ensure(
        matches!(err, RequirementError::Other(msg) if msg == "test error"),
        "&str should convert to Other",
    )?;
    Ok(())
}

#[test]
fn error_trait_implementation() -> TestResult {
    let err = RequirementError::OrAllFailed;
    // Verify it implements std::error::Error
    let _: &dyn std::error::Error = &err;
    Ok(())
}
