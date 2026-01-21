// ret-logic/tests/serde_support.rs
// ============================================================================
// Module: Serialization Tests
// Description: Tests for serde support, validation, and file operations.
// ============================================================================
//! ## Overview
//! Integration tests for serde helpers and validators.

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

// ========================================================================
// Mock Predicate Type
// ========================================================================

/// Lightweight predicate type for serialization tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum MockPredicate {
    /// Always returns true.
    AlwaysTrue,
    /// Always returns false.
    AlwaysFalse,
    /// Value greater-than-or-equal predicate.
    ValueGte(i32),
    /// Value less-than-or-equal predicate.
    ValueLte(i32),
    /// Value equality predicate.
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

fn ron_roundtrip(req: &Requirement<MockPredicate>) -> TestResult<Requirement<MockPredicate>> {
    let ron = convenience::to_ron(req)?;
    Ok(convenience::from_ron(&ron)?)
}

fn json_roundtrip(req: &Requirement<MockPredicate>) -> TestResult<Requirement<MockPredicate>> {
    let json = convenience::to_json(req)?;
    Ok(convenience::from_json(&json)?)
}

// ============================================================================
// SECTION: SerdeError Tests
// ============================================================================

#[test]
fn test_serde_error_display_invalid_structure() -> TestResult {
    let err = SerdeError::InvalidStructure("test message".to_string());
    let msg = err.to_string();
    ensure(msg.contains("Invalid requirement structure"), "Expected invalid structure message")?;
    ensure(msg.contains("test message"), "Expected message payload to be included")?;
    Ok(())
}

#[test]
fn test_serde_error_display_missing_field() -> TestResult {
    let err = SerdeError::MissingField("field_name".to_string());
    let msg = err.to_string();
    ensure(msg.contains("Missing required field"), "Expected missing field message")?;
    ensure(msg.contains("field_name"), "Expected field name to be included")?;
    Ok(())
}

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

#[test]
fn test_serde_error_display_circular_reference() -> TestResult {
    let err = SerdeError::CircularReference;
    ensure(err.to_string().contains("Circular reference"), "Expected circular reference message")?;
    Ok(())
}

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

#[test]
fn test_serde_config_default() -> TestResult {
    let config = SerdeConfig::default();
    ensure(config.max_depth == 32, "Expected default max depth")?;
    ensure(config.validate_on_deserialize, "Expected validation to be enabled by default")?;
    ensure(config.allow_empty_logical, "Expected empty logical groups to be allowed by default")?;
    Ok(())
}

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

#[test]
fn test_validator_with_defaults() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);
    ensure(validator.validate(&req).is_ok(), "Expected default validator to accept predicate")?;
    Ok(())
}

#[test]
fn test_validator_validates_predicate() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::predicate(MockPredicate::ValueGte(50));
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept ValueGte predicate")?;
    Ok(())
}

#[test]
fn test_validator_validates_and() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept AND")?;
    Ok(())
}

#[test]
fn test_validator_validates_empty_and() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req: Requirement<MockPredicate> = Requirement::and(vec![]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept empty AND")?;
    Ok(())
}

#[test]
fn test_validator_rejects_empty_and_when_configured() -> TestResult {
    let config = SerdeConfig {
        allow_empty_logical: false,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);
    let req: Requirement<MockPredicate> = Requirement::and(vec![]);
    ensure(
        matches!(validator.validate(&req), Err(SerdeError::InvalidStructure(_))),
        "Expected validator to reject empty AND when configured",
    )?;
    Ok(())
}

#[test]
fn test_validator_validates_or() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::or(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept OR")?;
    Ok(())
}

#[test]
fn test_validator_validates_empty_or() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req: Requirement<MockPredicate> = Requirement::or(vec![]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept empty OR")?;
    Ok(())
}

#[test]
fn test_validator_rejects_empty_or_when_configured() -> TestResult {
    let config = SerdeConfig {
        allow_empty_logical: false,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);
    let req: Requirement<MockPredicate> = Requirement::or(vec![]);
    ensure(
        matches!(validator.validate(&req), Err(SerdeError::InvalidStructure(_))),
        "Expected validator to reject empty OR when configured",
    )?;
    Ok(())
}

#[test]
fn test_validator_validates_not() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::not(Requirement::predicate(MockPredicate::AlwaysTrue));
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept NOT")?;
    Ok(())
}

#[test]
fn test_validator_validates_require_group() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::ValueGte(10)),
        ],
    );
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept RequireGroup")?;
    Ok(())
}

#[test]
fn test_validator_rejects_invalid_group() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::require_group(
        5, // min > total
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
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

#[test]
fn test_validator_rejects_group_min_zero_with_elements() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req =
        Requirement::require_group(0, vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    ensure(
        matches!(validator.validate(&req), Err(SerdeError::InvalidStructure(_))),
        "Expected validator to reject min=0 with elements",
    )?;
    Ok(())
}

#[test]
fn test_validator_validates_nested() -> TestResult {
    let validator = RequirementValidator::with_defaults();
    let req = Requirement::and(vec![
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::not(Requirement::predicate(MockPredicate::AlwaysFalse)),
        ]),
        Requirement::require_group(
            1,
            vec![
                Requirement::predicate(MockPredicate::ValueGte(10)),
                Requirement::predicate(MockPredicate::ValueLte(100)),
            ],
        ),
    ]);
    ensure(validator.validate(&req).is_ok(), "Expected validator to accept nested tree")?;
    Ok(())
}

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
            vec![Requirement::predicate(MockPredicate::AlwaysTrue)],
        )])])]);

    ensure(
        matches!(validator.validate(&req), Err(SerdeError::TooDeep { .. })),
        "Expected validator to reject overly deep trees",
    )?;
    Ok(())
}

#[test]
fn test_validator_accepts_at_max_depth() -> TestResult {
    let config = SerdeConfig {
        max_depth: 3,
        ..Default::default()
    };
    let validator = RequirementValidator::new(config);

    // Build a 3-level deep tree
    let req = Requirement::and(vec![Requirement::and(vec![Requirement::predicate(
        MockPredicate::AlwaysTrue,
    )])]);

    ensure(validator.validate(&req).is_ok(), "Expected validator to accept max-depth tree")?;
    Ok(())
}

// ============================================================================
// SECTION: RON Serialization Tests
// ============================================================================

#[test]
fn test_ron_roundtrip_predicate() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueGte(42));
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve predicate")?;
    Ok(())
}

#[test]
fn test_ron_roundtrip_and() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve AND")?;
    Ok(())
}

#[test]
fn test_ron_roundtrip_or() -> TestResult {
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::ValueEq(10)),
        Requirement::predicate(MockPredicate::ValueEq(20)),
    ]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve OR")?;
    Ok(())
}

#[test]
fn test_ron_roundtrip_not() -> TestResult {
    let req = Requirement::not(Requirement::predicate(MockPredicate::HasAllFlags(0xFF)));
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve NOT")?;
    Ok(())
}

#[test]
fn test_ron_roundtrip_require_group() -> TestResult {
    let req = Requirement::require_group(
        2,
        vec![
            Requirement::predicate(MockPredicate::AlwaysTrue),
            Requirement::predicate(MockPredicate::AlwaysFalse),
            Requirement::predicate(MockPredicate::ValueGte(50)),
        ],
    );
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve RequireGroup")?;
    Ok(())
}

#[test]
fn test_ron_roundtrip_complex_nested() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::or(vec![
            Requirement::predicate(MockPredicate::ValueGte(10)),
            Requirement::predicate(MockPredicate::ValueLte(0)),
        ]),
        Requirement::not(Requirement::predicate(MockPredicate::HasNoneFlags(0b11))),
        Requirement::require_group(
            1,
            vec![
                Requirement::predicate(MockPredicate::RowIndexEven),
                Requirement::predicate(MockPredicate::RowIndexLt(100)),
            ],
        ),
    ]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected RON roundtrip to preserve nested requirement")?;
    Ok(())
}

#[test]
fn test_ron_from_invalid_string() -> TestResult {
    let result: Result<Requirement<MockPredicate>, _> = convenience::from_ron("not valid ron {{{");
    ensure(result.is_err(), "Expected invalid RON to return an error")?;
    Ok(())
}

// ============================================================================
// SECTION: JSON Serialization Tests
// ============================================================================

#[test]
fn test_json_roundtrip_predicate() -> TestResult {
    let req = Requirement::predicate(MockPredicate::ValueGte(42));
    let parsed = json_roundtrip(&req)?;
    ensure(req == parsed, "Expected JSON roundtrip to preserve predicate")?;
    Ok(())
}

#[test]
fn test_json_roundtrip_and() -> TestResult {
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);
    let parsed = json_roundtrip(&req)?;
    ensure(req == parsed, "Expected JSON roundtrip to preserve AND")?;
    Ok(())
}

#[test]
fn test_json_roundtrip_nested() -> TestResult {
    let req = Requirement::and(vec![Requirement::or(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::not(Requirement::predicate(MockPredicate::AlwaysFalse)),
    ])]);
    let parsed = json_roundtrip(&req)?;
    ensure(req == parsed, "Expected JSON roundtrip to preserve nested requirement")?;
    Ok(())
}

#[test]
fn test_json_from_invalid_string() -> TestResult {
    let result: Result<Requirement<MockPredicate>, _> = convenience::from_json("{not: valid}");
    ensure(result.is_err(), "Expected invalid JSON to return an error")?;
    Ok(())
}

// ============================================================================
// SECTION: Convenience Function Tests
// ============================================================================

#[test]
fn test_convenience_validate() -> TestResult {
    let req = Requirement::and(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    ensure(convenience::validate(&req).is_ok(), "Expected convenience validate to succeed")?;
    Ok(())
}

#[test]
fn test_convenience_validate_invalid() -> TestResult {
    let req =
        Requirement::require_group(10, vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    ensure(convenience::validate(&req).is_err(), "Expected convenience validate to fail")?;
    Ok(())
}

#[test]
fn test_convenience_is_valid() -> TestResult {
    let valid = Requirement::and(vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);
    let invalid =
        Requirement::require_group(10, vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);

    ensure(convenience::is_valid(&valid), "Expected valid requirement to pass is_valid")?;
    ensure(!convenience::is_valid(&invalid), "Expected invalid requirement to fail is_valid")?;
    Ok(())
}

// ============================================================================
// SECTION: RequirementSerializer Tests
// ============================================================================

#[test]
fn test_serializer_with_defaults() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);

    let ron = serializer.to_ron(&req)?;
    let parsed: Requirement<MockPredicate> = serializer.from_ron(&ron)?;
    ensure(req == parsed, "Expected serializer RON roundtrip to match")?;
    Ok(())
}

#[test]
fn test_serializer_default_impl() -> TestResult {
    let serializer = RequirementSerializer::default();
    let req = Requirement::predicate(MockPredicate::AlwaysTrue);

    ensure(serializer.validate(&req).is_ok(), "Expected default serializer to validate")?;
    Ok(())
}

#[test]
fn test_serializer_custom_config() -> TestResult {
    let config = SerdeConfig {
        max_depth: 2,
        validate_on_deserialize: true,
        allow_empty_logical: false,
    };
    let serializer = RequirementSerializer::new(config);

    // Valid requirement
    let valid_req = Requirement::predicate(MockPredicate::AlwaysTrue);
    ensure(serializer.to_ron(&valid_req).is_ok(), "Expected valid requirement to serialize")?;

    // Too deep
    let deep_req = Requirement::and(vec![Requirement::and(vec![Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
    ])])]);
    ensure(serializer.to_ron(&deep_req).is_err(), "Expected deep requirement to fail")?;
    Ok(())
}

#[test]
fn test_serializer_to_json() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let req = Requirement::or(vec![
        Requirement::predicate(MockPredicate::ValueGte(10)),
        Requirement::predicate(MockPredicate::ValueLte(0)),
    ]);

    let json = serializer.to_json(&req)?;
    let parsed: Requirement<MockPredicate> = serializer.from_json(&json)?;
    ensure(req == parsed, "Expected serializer JSON roundtrip to match")?;
    Ok(())
}

#[test]
fn test_serializer_validates_on_serialize() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let invalid_req =
        Requirement::require_group(10, vec![Requirement::predicate(MockPredicate::AlwaysTrue)]);

    ensure(
        serializer.to_ron(&invalid_req).is_err(),
        "Expected serializer to validate on serialize",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Edge Cases
// ============================================================================

#[test]
fn test_ron_empty_and() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::and(vec![]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected empty AND to roundtrip")?;
    Ok(())
}

#[test]
fn test_ron_empty_or() -> TestResult {
    let req: Requirement<MockPredicate> = Requirement::or(vec![]);
    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected empty OR to roundtrip")?;
    Ok(())
}

#[test]
fn test_ron_all_predicate_variants() -> TestResult {
    let predicates = vec![
        MockPredicate::AlwaysTrue,
        MockPredicate::AlwaysFalse,
        MockPredicate::ValueGte(100),
        MockPredicate::ValueLte(-50),
        MockPredicate::ValueEq(0),
        MockPredicate::HasAllFlags(0xDEAD_BEEF),
        MockPredicate::HasAnyFlags(0b10101),
        MockPredicate::HasNoneFlags(0xFF00),
        MockPredicate::RowIndexEven,
        MockPredicate::RowIndexLt(1000),
    ];

    for pred in predicates {
        let label = format!("{pred:?}");
        let req = Requirement::predicate(pred);
        let parsed = ron_roundtrip(&req)?;
        ensure(req == parsed, format!("Failed for predicate: {label}"))?;
    }
    Ok(())
}

#[test]
fn test_ron_large_group() -> TestResult {
    let reqs: Vec<_> =
        (0 .. 50).map(|i| Requirement::predicate(MockPredicate::ValueGte(i))).collect();
    let req = Requirement::require_group(25, reqs);

    let parsed = ron_roundtrip(&req)?;
    ensure(req == parsed, "Expected large group to roundtrip")?;
    Ok(())
}

#[test]
fn test_json_pretty_format() -> TestResult {
    let serializer = RequirementSerializer::with_defaults();
    let req = Requirement::and(vec![
        Requirement::predicate(MockPredicate::AlwaysTrue),
        Requirement::predicate(MockPredicate::AlwaysFalse),
    ]);

    let json = serializer.to_json(&req)?;
    // Pretty printed JSON should have newlines
    ensure(json.contains('\n'), "Expected pretty JSON output to include newlines")?;
    Ok(())
}
