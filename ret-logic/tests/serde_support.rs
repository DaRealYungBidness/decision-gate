// ret-logic/tests/serde_support.rs
// ============================================================================
// Module: Serialization Tests
// Description: Tests for serde support, validation, and file operations.
// Purpose: Validate requirement serialization and validation helpers.
// Dependencies: ret_logic::serde_support
// ============================================================================
//! ## Overview
//! Integration tests for serde helpers and validators.

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

use ret_logic::Requirement;
use ret_logic::serde_support::RequirementSerializer;
use ret_logic::serde_support::RequirementValidator;
use ret_logic::serde_support::SerdeConfig;
use ret_logic::serde_support::SerdeError;
use ret_logic::serde_support::convenience;
use serde::Deserialize;
use serde::Serialize;
use support::TestResult;
use support::ensure;

// ============================================================================
// SECTION: Mock Condition Type
// ============================================================================

/// Lightweight condition type for serialization tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum MockCondition {
    /// Always returns true.
    AlwaysTrue,
    /// Always returns false.
    AlwaysFalse,
    /// Value greater-than-or-equal condition.
    ValueGte(i32),
    /// Value less-than-or-equal condition.
    ValueLte(i32),
    /// Value equality condition.
    ValueEq(i32),
    /// Flags must include all bits.
    HasAllFlags(u64),
    /// Flags must include any bit.
    HasAnyFlags(u64),
    /// Flags must include none of the bits.
    HasNoneFlags(u64),
    /// Row index is even.
    RowIndexEven,
    /// Row index is less than threshold.
    RowIndexLt(usize),
}

fn ron_roundtrip(req: &Requirement<MockCondition>) -> TestResult<Requirement<MockCondition>> {
    let ron = convenience::to_ron(req)?;
    Ok(convenience::from_ron(&ron)?)
}

fn json_roundtrip(req: &Requirement<MockCondition>) -> TestResult<Requirement<MockCondition>> {
    let json = convenience::to_json(req)?;
    Ok(convenience::from_json(&json)?)
}

// ============================================================================
// SECTION: SerdeError Tests
// ============================================================================

/// Tests serde error display invalid structure.
#[test]
fn test_serde_error_display_invalid_structure() -> TestResult {
    let err = SerdeError::InvalidStructure("test message".to_string());
    let msg = err.to_string();
    ensure(msg.contains("Invalid requirement structure"), "Expected invalid structure message")?;
    ensure(msg.contains("test message"), "Expected message payload to be included")?;
    Ok(())
}

/// Tests serde error display missing field.
#[test]
fn test_serde_error_display_missing_field() -> TestResult {
    let err = SerdeError::MissingField("field_name".to_string());
    let msg = err.to_string();
    ensure(msg.contains("Missing required field"), "Expected missing field message")?;
    ensure(msg.contains("field_name"), "Expected field name to be included")?;
    Ok(())
}

/// Tests serde error display invalid value.
#[test]
fn test_serde_error_display_invalid_value() -> TestResult {
    let err = SerdeError::InvalidValue {
        field: "test_field".to_string(),
        value: "bad".to_string(),
        expected: "good".to_string(),
    };
    let msg = err.to_string();
    ensure(msg.contains("test_field"), "Expected field name to be included")?;
    ensure(msg.contains("bad"), "Expected invalid value to be included")?;
    ensure(msg.contains("good"), "Expected expected value to be included")?;
    Ok(())
}

/// Tests serde error display circular reference.
#[test]
fn test_serde_error_display_circular_reference() -> TestResult {
    let err = SerdeError::CircularReference;
    ensure(err.to_string().contains("Circular reference"), "Expected circular reference message")?;
    Ok(())
}

/// Tests serde error display too deep.
#[test]
fn test_serde_error_display_too_deep() -> TestResult {
    let err = SerdeError::TooDeep {
        max_depth: 10,
        actual_depth: 15,
    };
    let msg = err.to_string();
    ensure(msg.contains("15"), "Expected actual depth to be included")?;
    ensure(msg.contains("10"), "Expected max depth to be included")?;
    Ok(())
}

/// Tests serde error display invalid group.
#[test]
fn test_serde_error_display_invalid_group() -> TestResult {
    let err = SerdeError::InvalidGroup {
        min: 5,
        total: 3,
    };
    let msg = err.to_string();
    ensure(msg.contains("min 5"), "Expected group min to be included")?;
    ensure(msg.contains("total 3"), "Expected group total to be included")?;
    Ok(())
}

/// Tests serde error is std error.
#[test]
fn test_serde_error_is_std_error() -> TestResult {
    let err = SerdeError::CircularReference;
    let err_ref: &dyn std::error::Error = &err;
    ensure(err_ref.source().is_none(), "Expected SerdeError to have no source")?;
    Ok(())
}

// ============================================================================
// SECTION: SerdeConfig Tests
// ============================================================================

/// Tests serde config default.
#[test]
fn test_serde_config_default() -> TestResult {
    let config = SerdeConfig::default();
    ensure(config.max_depth == 32, "Expected default max depth")?;
    ensure(config.validate_on_deserialize, "Expected validation to be enabled by default")?;
    ensure(config.allow_empty_logical, "Expected empty logical groups to be allowed by default")?;
    Ok(())
}

/// Tests serde config custom.
#[test]
fn test_serde_config_custom() -> TestResult {
    let config = SerdeConfig {
        max_depth: 16,
        validate_on_deserialize: false,
        allow_empty_logical: false,
    };
    ensure(config.max_depth == 16, "Expected custom max depth")?;
    ensure(!config.validate_on_deserialize, "Expected validation to be disabled")?;
    ensure(!config.allow_empty_logical, "Expected empty logical groups to be disallowed")?;
    Ok(())
}

// ============================================================================
// SECTION: RequirementValidator Tests
// ============================================================================

/// Tests validator with defaults.
#[test]
fn test_validator_with_defaults() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::condition(MockCondition::AlwaysTrue);
    ensure(validator.validate(&req).is_ok(), "Expected default validator to accept condition")?;
    Ok(())
}

/// Tests validator validates condition.
#[test]
fn test_validator_validates_condition() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::condition(MockCondition::ValueGte(50));
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept ValueGte condition")?;
    Ok(())
}

/// Tests validator validates and.
#[test]
fn test_validator_validates_and() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::and(vec![
        Requirement::condition(MockCondition::AlwaysTrue),
        Requirement::condition(MockCondition::AlwaysFalse),
    ]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept AND")?;
    Ok(())
}

/// Tests validator validates empty and.
#[test]
fn test_validator_validates_empty_and() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req: Requirement<MockCondition> = Requirement::and(vec![]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept empty AND")?;
    Ok(())
}

/// Tests validator rejects empty and when configured.
#[test]
fn test_validator_rejects_empty_and_when_configured() -> TestResult {
    let config = SerdeConfig {
        allow_empty_logical: false,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);
    let req: Requirement<MockCondition> = Requirement::and(vec![]);
    ensure(
        matches!(validator.validate(&req), Err(SerdeError::InvalidStructure(_))),
        "Expected validator to reject empty AND when configured",
    )?;
    Ok(())
}

/// Tests validator validates or.
#[test]
fn test_validator_validates_or() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::or(vec![Requirement::condition(MockCondition::AlwaysTrue)]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept OR")?;
    Ok(())
}

/// Tests validator validates empty or.
#[test]
fn test_validator_validates_empty_or() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req: Requirement<MockCondition> = Requirement::or(vec![]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept empty OR")?;
    Ok(())
}

/// Tests validator rejects empty or when configured.
#[test]
fn test_validator_rejects_empty_or_when_configured() -> TestResult {
    let config = SerdeConfig {
        allow_empty_logical: false,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);
    let req: Requirement<MockCondition> = Requirement::or(vec![]);
    ensure(
        matches!(validator.validate(&req), Err(SerdeError::InvalidStructure(_))),
        "Expected validator to reject empty OR when configured",
    )?;
    Ok(())
}

/// Tests validator validates not.
#[test]
fn test_validator_validates_not() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::negate(Requirement::condition(MockCondition::AlwaysTrue));
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept NOT")?;
    Ok(())
}

/// Tests validator validates require group.
#[test]
fn test_validator_validates_require_group() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::condition(MockCondition::AlwaysTrue),
            Requirement::condition(MockCondition::AlwaysFalse),
            Requirement::condition(MockCondition::ValueGte(10)),
        ],
    );
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept RequireGroup")?;
    Ok(())
}

/// Tests validator rejects invalid group.
#[test]
fn test_validator_rejects_invalid_group() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::require_group(
        5, // min > total
        vec![
            Requirement::condition(MockCondition::AlwaysTrue),
            Requirement::condition(MockCondition::AlwaysFalse),
        ],
    );
    ensure(
        matches!(
            validator.validate(&req),
            Err(SerdeError::InvalidGroup {
                min: 5,
                total: 2
            })
        ),
        "Expected validator to reject invalid group min",
    )?;
    Ok(())
}

/// Tests validator rejects group min zero with elements.
#[test]
fn test_validator_rejects_group_min_zero_with_elements() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req =
        Requirement::require_group(0, vec![Requirement::condition(MockCondition::AlwaysTrue)]);
    ensure(
        matches!(validator.validate(&req), Err(SerdeError::InvalidStructure(_))),
        "Expected validator to reject min=0 with elements",
    )?;
    Ok(())
}

/// Tests validator validates nested.
#[test]
fn test_validator_validates_nested() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::and(vec![
        Requirement::or(vec![
            Requirement::condition(MockCondition::AlwaysTrue),
            Requirement::negate(Requirement::condition(MockCondition::AlwaysFalse)),
        ]),
        Requirement::require_group(
            1,
            vec![
                Requirement::condition(MockCondition::ValueGte(10)),
                Requirement::condition(MockCondition::ValueLte(100)),
            ],
        ),
    ]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept nested tree")?;
    Ok(())
}

/// Tests validator rejects too deep.
#[test]
fn test_validator_rejects_too_deep() -> TestResult {
    let config = SerdeConfig {
        max_depth: 3,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);

    // Build a 5-level deep tree
    let req =
        Requirement::and(vec![Requirement::and(vec![Requirement::and(vec![Requirement::and(
            vec![Requirement::condition(MockCondition::AlwaysTrue)],
        )])])]);

    ensure(
        matches!(validator.validate(&req), Err(SerdeError::TooDeep { .. })),
        "Expected validator to reject overly deep trees",
    )?;
    Ok(())
}

/// Tests validator accepts at max depth.
#[test]
fn test_validator_accepts_at_max_depth() -> TestResult {
    let config = SerdeConfig {
        max_depth: 3,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);

    // Build a 3-level deep tree
    let req = Requirement::and(vec![Requirement::and(vec![Requirement::condition(
        MockCondition::AlwaysTrue,
    )])]);

    ensure(validator.validate(&req).is_ok(), "Expected validator to accept max-depth tree")?;
    Ok(())
}

// ============================================================================
// SECTION: RON Serialization Tests
// ============================================================================

/// Tests ron roundtrip condition.
#[test]
fn test_ron_roundtrip_condition() -> TestResult {
    let req = Requirement::condition(MockCondition::ValueGte(42));
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve condition")?;
    Ok(())
}

/// Tests ron roundtrip and.
#[test]
fn test_ron_roundtrip_and() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::condition(MockCondition::AlwaysTrue),
        Requirement::condition(MockCondition::AlwaysFalse),
    ]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve AND")?;
    Ok(())
}

/// Tests ron roundtrip or.
#[test]
fn test_ron_roundtrip_or() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::condition(MockCondition::ValueEq(10)),
        Requirement::condition(MockCondition::ValueEq(20)),
    ]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve OR")?;
    Ok(())
}

/// Tests ron roundtrip not.
#[test]
fn test_ron_roundtrip_not() -> TestResult {
    let req = Requirement::negate(Requirement::condition(MockCondition::HasAllFlags(0xFF)));
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve NOT")?;
    Ok(())
}

/// Tests ron roundtrip require group.
#[test]
fn test_ron_roundtrip_require_group() -> TestResult {
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::condition(MockCondition::AlwaysTrue),
            Requirement::condition(MockCondition::AlwaysFalse),
            Requirement::condition(MockCondition::ValueGte(50)),
        ],
    );
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve RequireGroup")?;
    Ok(())
}

/// Tests ron roundtrip complex nested.
#[test]
fn test_ron_roundtrip_complex_nested() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::or(vec![
            Requirement::condition(MockCondition::ValueGte(10)),
            Requirement::condition(MockCondition::ValueLte(0)),
        ]),
        Requirement::negate(Requirement::condition(MockCondition::HasNoneFlags(0b11))),
        Requirement::require_group(
            1,
            vec![
                Requirement::condition(MockCondition::RowIndexEven),
                Requirement::condition(MockCondition::RowIndexLt(100)),
            ],
        ),
    ]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve nested requirement")?;
    Ok(())
}

/// Tests ron from invalid string.
#[test]
fn test_ron_from_invalid_string() -> TestResult {
    let result: Result<Requirement<MockCondition>, _> = convenience::from_ron("not valid ron {{{");
    ensure(result.is_err(), "Expected invalid RON to return an error")?;
    Ok(())
}

// ============================================================================
// SECTION: JSON Serialization Tests
// ============================================================================

/// Tests json roundtrip condition.
#[test]
fn test_json_roundtrip_condition() -> TestResult {
    let req = Requirement::condition(MockCondition::ValueGte(42));
    let parsed = json_roundtrip(&req)?;
    ensure(req == parsed, "Expected JSON roundtrip to preserve condition")?;
    Ok(())
}

/// Tests json roundtrip and.
#[test]
fn test_json_roundtrip_and() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::condition(MockCondition::AlwaysTrue),
        Requirement::condition(MockCondition::AlwaysFalse),
    ]);
    let parsed = json_roundtrip(&req)?;
    ensure(req == parsed, "Expected JSON roundtrip to preserve AND")?;
    Ok(())
}

/// Tests json roundtrip nested.
#[test]
fn test_json_roundtrip_nested() -> TestResult {
    let req = Requirement::and(vec![Requirement::or(vec![
        Requirement::condition(MockCondition::AlwaysTrue),
        Requirement::negate(Requirement::condition(MockCondition::AlwaysFalse)),
    ])]);
    let parsed = json_roundtrip(&req)?;
    ensure(req == parsed, "Expected JSON roundtrip to preserve nested requirement")?;
    Ok(())
}

/// Tests json from invalid string.
#[test]
fn test_json_from_invalid_string() -> TestResult {
    let result: Result<Requirement<MockCondition>, _> = convenience::from_json("{not: valid}");
    ensure(result.is_err(), "Expected invalid JSON to return an error")?;
    Ok(())
}

// ============================================================================
// SECTION: Convenience Function Tests
// ============================================================================

/// Tests convenience validate.
#[test]
fn test_convenience_validate() -> TestResult {
    let req = Requirement::and(vec![Requirement::condition(MockCondition::AlwaysTrue)]);
    ensure(convenience::validate(&req).is_ok(), "Expected convenience validate to succeed")?;
    Ok(())
}

/// Tests convenience validate invalid.
#[test]
fn test_convenience_validate_invalid() -> TestResult {
    let req =
        Requirement::require_group(10, vec![Requirement::condition(MockCondition::AlwaysTrue)]);
    ensure(convenience::validate(&req).is_err(), "Expected convenience validate to fail")?;
    Ok(())
}

/// Tests convenience is valid.
#[test]
fn test_convenience_is_valid() -> TestResult {
    let valid = Requirement::and(vec![Requirement::condition(MockCondition::AlwaysTrue)]);
    let invalid =
        Requirement::require_group(10, vec![Requirement::condition(MockCondition::AlwaysTrue)]);

    ensure(convenience::is_valid(&valid), "Expected valid requirement to pass is_valid")?;
    ensure(!convenience::is_valid(&invalid), "Expected invalid requirement to fail is_valid")?;
    Ok(())
}

// ============================================================================
// SECTION: RequirementSerializer Tests
// ============================================================================

/// Tests serializer with defaults.
#[test]
fn test_serializer_with_defaults() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let req = Requirement::condition(MockCondition::AlwaysTrue);

    let ron = serializer.to_ron(&req)?;
    let parsed: Requirement<MockCondition> = serializer.from_ron(&ron)?;
    ensure(req == parsed, "Expected serializer RON roundtrip to match")?;
    Ok(())
}

/// Tests serializer default impl.
#[test]
fn test_serializer_default_impl() -> TestResult {
    let serializer = RequirementSerializer::default();
    let req = Requirement::condition(MockCondition::AlwaysTrue);

    ensure(serializer.validate(&req).is_ok(), "Expected default serializer to validate")?;
    Ok(())
}

/// Tests serializer custom config.
#[test]
fn test_serializer_custom_config() -> TestResult {
    let config = SerdeConfig {
        max_depth: 2,
        validate_on_deserialize: true,
        allow_empty_logical: false,
    };
    let serializer = RequirementSerializer::new(config);

    // Valid requirement
    let valid_req = Requirement::condition(MockCondition::AlwaysTrue);
    ensure(serializer.to_ron(&valid_req).is_ok(), "Expected valid requirement to serialize")?;

    // Too deep
    let deep_req = Requirement::and(vec![Requirement::and(vec![Requirement::and(vec![
        Requirement::condition(MockCondition::AlwaysTrue),
    ])])]);
    ensure(serializer.to_ron(&deep_req).is_err(), "Expected deep requirement to fail")?;
    Ok(())
}

/// Tests serializer to json.
#[test]
fn test_serializer_to_json() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let req = Requirement::or(vec![
        Requirement::condition(MockCondition::ValueGte(10)),
        Requirement::condition(MockCondition::ValueLte(0)),
    ]);

    let json = serializer.to_json(&req)?;
    let parsed: Requirement<MockCondition> = serializer.from_json(&json)?;
    ensure(req == parsed, "Expected serializer JSON roundtrip to match")?;
    Ok(())
}

/// Tests serializer validates on serialize.
#[test]
fn test_serializer_validates_on_serialize() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let invalid_req =
        Requirement::require_group(10, vec![Requirement::condition(MockCondition::AlwaysTrue)]);

    ensure(
        serializer.to_ron(&invalid_req).is_err(),
        "Expected serializer to validate on serialize",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Input Size Limits
// ============================================================================

/// Default serialized requirement size limit used by the serializer.
const MAX_SERIALIZED_REQUIREMENT_BYTES: usize = 1024 * 1024;

/// Tests serializer rejects oversized RON input.
#[test]
fn test_serializer_rejects_oversized_ron_input() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let oversized = "a".repeat(MAX_SERIALIZED_REQUIREMENT_BYTES + 1);
    let err = serializer
        .from_ron::<MockCondition>(&oversized)
        .expect_err("Expected oversized RON input to be rejected");
    ensure(err.to_string().contains("size limit"), "Expected size limit error message")?;
    Ok(())
}

/// Tests serializer rejects oversized JSON input.
#[test]
fn test_serializer_rejects_oversized_json_input() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let oversized = "a".repeat(MAX_SERIALIZED_REQUIREMENT_BYTES + 1);
    let err = serializer
        .from_json::<MockCondition>(&oversized)
        .expect_err("Expected oversized JSON input to be rejected");
    ensure(err.to_string().contains("size limit"), "Expected size limit error message")?;
    Ok(())
}

// ============================================================================
// SECTION: Edge Cases
// ============================================================================

/// Tests ron empty and.
#[test]
fn test_ron_empty_and() -> TestResult {
    let req: Requirement<MockCondition> = Requirement::and(vec![]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected empty AND to roundtrip")?;
    Ok(())
}

/// Tests ron empty or.
#[test]
fn test_ron_empty_or() -> TestResult {
    let req: Requirement<MockCondition> = Requirement::or(vec![]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected empty OR to roundtrip")?;
    Ok(())
}

/// Tests ron all condition variants.
#[test]
fn test_ron_all_condition_variants() -> TestResult {
    let conditions = vec![
        MockCondition::AlwaysTrue,
        MockCondition::AlwaysFalse,
        MockCondition::ValueGte(100),
        MockCondition::ValueLte(-50),
        MockCondition::ValueEq(0),
        MockCondition::HasAllFlags(0xDEAD_BEEF),
        MockCondition::HasAnyFlags(0b10101),
        MockCondition::HasNoneFlags(0xFF00),
        MockCondition::RowIndexEven,
        MockCondition::RowIndexLt(1000),
    ];

    for pred in conditions {
        let label = format!("{pred:?}");
        let req = Requirement::condition(pred);
        let parsed = ron_roundtrip(&req)?;
        ensure(req == parsed, format!("Failed for condition: {label}"))?;
    }
    Ok(())
}

/// Tests ron large group.
#[test]
fn test_ron_large_group() -> TestResult {
    let reqs: Vec<_> =
        (0 .. 50).map(|i| Requirement::condition(MockCondition::ValueGte(i))).collect();
    let req = Requirement::require_group(25, reqs);

    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected large group to roundtrip")?;
    Ok(())
}

/// Tests json pretty format.
#[test]
fn test_json_pretty_format() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let req = Requirement::and(vec![
        Requirement::condition(MockCondition::AlwaysTrue),
        Requirement::condition(MockCondition::AlwaysFalse),
    ]);

    let json = serializer.to_json(&req)?;
    // Pretty printed JSON should have newlines
    ensure(json.contains('\n'), "Expected pretty JSON output to include newlines")?;
    Ok(())
}
