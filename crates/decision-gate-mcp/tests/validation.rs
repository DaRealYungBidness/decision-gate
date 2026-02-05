// crates/decision-gate-mcp/tests/validation.rs
// ============================================================================
// Module: Strict Validation Tests
// Description: Tests for type-class inference, comparator allowances, and
//              fail-closed validation logic.
// Purpose: Ensure security-critical validation rejects ambiguous inputs.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Tests strict comparator validation including:
//! - Type class inference from JSON schemas
//! - Comparator allowance computation
//! - Schema variant resolution (oneOf, anyOf)
//! - x-decision-gate metadata parsing
//! - Feature toggle enforcement
//! - Expected value validation
//!
//! Security posture: All validation must fail closed on ambiguous inputs.
//! Threat model: TM-VAL-001 - Comparator bypass via schema manipulation.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions are permitted."
)]

mod common;

use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_mcp::config::ValidationConfig;
use decision_gate_mcp::config::ValidationProfile;
use decision_gate_mcp::validation::StrictValidator;
use serde_json::json;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Creates a minimal scenario spec with a single condition for testing validation.
fn spec_with_condition(
    comparator: Comparator,
    expected: Option<serde_json::Value>,
) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("test"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: Vec::new(),
        conditions: vec![ConditionSpec {
            condition_id: "test_pred".into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new("test"),
                check_id: "test".to_string(),
                params: None,
            },
            comparator,
            expected,
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

/// Creates a default validation config with strict=true.
const fn strict_config() -> ValidationConfig {
    ValidationConfig {
        strict: true,
        profile: ValidationProfile::StrictCoreV1,
        allow_permissive: false,
        enable_lexicographic: false,
        enable_deep_equals: false,
    }
}

/// Creates a validation config with lexicographic enabled.
const fn config_with_lexicographic() -> ValidationConfig {
    ValidationConfig {
        strict: true,
        profile: ValidationProfile::StrictCoreV1,
        allow_permissive: false,
        enable_lexicographic: true,
        enable_deep_equals: false,
    }
}

/// Creates a validation config with `deep_equals` enabled.
const fn config_with_deep_equals() -> ValidationConfig {
    ValidationConfig {
        strict: true,
        profile: ValidationProfile::StrictCoreV1,
        allow_permissive: false,
        enable_lexicographic: false,
        enable_deep_equals: true,
    }
}

/// Creates a validation config with strict=false.
const fn permissive_config() -> ValidationConfig {
    ValidationConfig {
        strict: false,
        profile: ValidationProfile::StrictCoreV1,
        allow_permissive: true,
        enable_lexicographic: false,
        enable_deep_equals: false,
    }
}

// ============================================================================
// SECTION: Type Class Inference Tests
// ============================================================================

#[test]
fn type_class_boolean_allows_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({"type": "boolean"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "boolean should allow equals: {result:?}");
}

#[test]
fn type_class_boolean_forbids_greater_than() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!(true)));
    let schema = json!({"type": "boolean"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "boolean should forbid greater_than");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not allowed for schema type"),
        "error should mention schema type: {err}"
    );
}

#[test]
fn type_class_integer_allows_numeric_ordering() {
    let validator = StrictValidator::new(strict_config());
    for comparator in [
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
    ] {
        let spec = spec_with_condition(comparator, Some(json!(10)));
        let schema = json!({"type": "integer"});
        let result = validator.validate_precheck(&spec, &schema);
        assert!(result.is_ok(), "integer should allow {comparator:?}: {result:?}");
    }
}

#[test]
fn type_class_integer_forbids_lexicographic() {
    let validator = StrictValidator::new(strict_config());
    for comparator in [
        Comparator::LexGreaterThan,
        Comparator::LexGreaterThanOrEqual,
        Comparator::LexLessThan,
        Comparator::LexLessThanOrEqual,
    ] {
        let spec = spec_with_condition(comparator, Some(json!(10)));
        let schema = json!({"type": "integer"});
        let result = validator.validate_precheck(&spec, &schema);
        assert!(result.is_err(), "integer should forbid lexicographic {comparator:?}");
    }
}

#[test]
fn type_class_number_allows_numeric_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!(std::f64::consts::PI)));
    let schema = json!({"type": "number"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "number should allow greater_than: {result:?}");
}

#[test]
fn type_class_string_allows_contains() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Contains, Some(json!("needle")));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "string should allow contains: {result:?}");
}

#[test]
fn dynamic_schema_allows_contains_without_type() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Contains, Some(json!("needle")));
    let schema = json!({
        "x-decision-gate": {
            "dynamic_type": true
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "dynamic schema should allow contains: {result:?}");
}

#[test]
fn type_class_string_forbids_numeric_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!("abc")));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "string should forbid numeric greater_than");
}

#[test]
fn type_class_string_optsin_lexicographic() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::LexGreaterThan, Some(json!("abc")));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "string lexicographic should require opt-in: {result:?}");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("disabled by config")
            || err.to_string().contains("requires explicit opt-in"),
        "error should mention opt-in or disabled: {err}"
    );
}

#[test]
fn type_class_string_date_format_allows_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!("2024-01-01")));
    let schema = json!({"type": "string", "format": "date"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "date format should allow ordering: {result:?}");
}

#[test]
fn type_class_string_datetime_format_allows_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::LessThan, Some(json!("2024-01-01T00:00:00Z")));
    let schema = json!({"type": "string", "format": "date-time"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "date-time format should allow ordering: {result:?}");
}

#[test]
fn type_class_string_uuid_format_allows_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(
        Comparator::Equals,
        Some(json!("550e8400-e29b-41d4-a716-446655440000")),
    );
    let schema = json!({"type": "string", "format": "uuid"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "uuid format should allow equals: {result:?}");
}

#[test]
fn type_class_string_uuid_format_forbids_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(
        Comparator::GreaterThan,
        Some(json!("550e8400-e29b-41d4-a716-446655440000")),
    );
    let schema = json!({"type": "string", "format": "uuid"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "uuid format should forbid ordering");
}

#[test]
fn type_class_enum_allows_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!("active")));
    let schema = json!({"enum": ["active", "inactive"]});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "enum should allow equals: {result:?}");
}

#[test]
fn type_class_enum_allows_in_set() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::InSet, Some(json!(["active", "pending"])));
    let schema = json!({"enum": ["active", "inactive", "pending"]});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "enum should allow in_set: {result:?}");
}

#[test]
fn type_class_enum_forbids_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!("active")));
    let schema = json!({"enum": ["active", "inactive"]});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "enum should forbid ordering");
}

#[test]
fn type_class_array_scalar_allows_exists() {
    let validator = StrictValidator::new(strict_config());
    // Use Exists to verify ArrayScalar type class inference
    let spec = spec_with_condition(Comparator::Exists, None);
    // Wrap in object with property for the condition
    let schema = json!({
        "type": "object",
        "properties": {
            "test_pred": {"type": "array", "items": {"type": "string"}}
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "array of scalars should allow exists: {result:?}");
}

#[test]
fn type_class_string_in_array_allows_contains() {
    let validator = StrictValidator::new(strict_config());
    // String type allows Contains comparator
    let spec = spec_with_condition(Comparator::Contains, Some(json!("needle")));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "string should allow contains: {result:?}");
}

#[test]
fn type_class_array_scalar_optsin_deep_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::DeepEquals, Some(json!(["a", "b"])));
    let schema = json!({"type": "array", "items": {"type": "string"}});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "array deep_equals should require opt-in: {result:?}");
}

#[test]
fn type_class_array_complex_forbids_contains() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Contains, Some(json!({"key": "value"})));
    let schema = json!({"type": "array", "items": {"type": "object"}});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "array of objects should forbid contains");
}

#[test]
fn type_class_array_complex_allows_exists() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Exists, None);
    let schema = json!({"type": "array", "items": {"type": "object"}});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "array of objects should allow exists: {result:?}");
}

#[test]
fn type_class_object_allows_exists() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Exists, None);
    // Wrap in object with property for the condition
    let schema = json!({
        "type": "object",
        "properties": {
            "test_pred": {"type": "object"}
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "object should allow exists: {result:?}");
}

#[test]
fn type_class_object_forbids_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!({"key": "value"})));
    let schema = json!({"type": "object"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "object should forbid equals");
}

#[test]
fn type_class_object_optsin_deep_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::DeepEquals, Some(json!({"key": "value"})));
    let schema = json!({"type": "object"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "object deep_equals should require opt-in: {result:?}");
}

#[test]
fn type_class_null_allows_equals() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(null)));
    let schema = json!({"type": "null"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "null should allow equals: {result:?}");
}

#[test]
fn type_class_null_allows_exists() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Exists, None);
    let schema = json!({"type": "null"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "null should allow exists: {result:?}");
}

#[test]
fn type_class_null_forbids_ordering() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!(null)));
    let schema = json!({"type": "null"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "null should forbid ordering");
}

// ============================================================================
// SECTION: Schema Variant Resolution Tests
// ============================================================================

#[test]
fn one_of_filters_null_variant() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Contains, Some(json!("needle")));
    let schema = json!({
        "oneOf": [
            {"type": "null"},
            {"type": "string"}
        ]
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "oneOf should filter null and use string: {result:?}");
}

#[test]
fn any_of_filters_null_variant() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(42)));
    let schema = json!({
        "anyOf": [
            {"type": "null"},
            {"type": "integer"}
        ]
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "anyOf should filter null and use integer: {result:?}");
}

#[test]
fn one_of_intersects_allowances() {
    let validator = StrictValidator::new(strict_config());
    // Exists is allowed for both string and integer, and doesn't need expected
    let spec = spec_with_condition(Comparator::Exists, None);
    let schema = json!({
        "oneOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "oneOf should allow exists (common to both): {result:?}");
}

#[test]
fn one_of_forbids_non_intersecting_comparator() {
    let validator = StrictValidator::new(strict_config());
    // Contains is only allowed for string, not integer
    let spec = spec_with_condition(Comparator::Contains, Some(json!("test")));
    let schema = json!({
        "oneOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "oneOf should forbid contains (not allowed for integer)");
}

#[test]
fn one_of_empty_rejects() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({"oneOf": []});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "empty oneOf should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("at least one option"),
        "error should mention empty oneOf: {err}"
    );
}

#[test]
fn any_of_empty_rejects() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({"anyOf": []});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "empty anyOf should fail");
}

#[test]
fn both_one_of_and_any_of_rejects() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({
        "oneOf": [{"type": "string"}],
        "anyOf": [{"type": "integer"}]
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "schema with both oneOf and anyOf should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("both oneOf and anyOf"),
        "error should mention conflict: {err}"
    );
}

#[test]
fn union_type_array_intersects() {
    let validator = StrictValidator::new(strict_config());
    // Equals is allowed for both string and null
    let spec = spec_with_condition(Comparator::Equals, Some(json!("test")));
    let schema = json!({"type": ["string", "null"]});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "type array union should allow equals: {result:?}");
}

#[test]
fn null_only_variants_preserved() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(null)));
    let schema = json!({"oneOf": [{"type": "null"}]});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "null-only oneOf should preserve null type: {result:?}");
}

// ============================================================================
// SECTION: Metadata Override Tests
// ============================================================================

#[test]
fn x_decision_gate_overrides_allowed_comparators() {
    let validator = StrictValidator::new(strict_config());
    // Normally equals is allowed for integer, but override restricts to only in_set
    let spec = spec_with_condition(Comparator::Equals, Some(json!(42)));
    let schema = json!({
        "type": "integer",
        "x-decision-gate": {
            "allowed_comparators": ["in_set"]
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "override should restrict to in_set only");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not in allowed_comparators"),
        "error should mention allowed_comparators: {err}"
    );
}

#[test]
fn x_decision_gate_must_be_object() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(42)));
    let schema = json!({
        "type": "integer",
        "x-decision-gate": "invalid"
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "non-object x-decision-gate should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("must be an object"),
        "error should mention object requirement: {err}"
    );
}

#[test]
fn allowed_comparators_must_be_non_empty() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(42)));
    let schema = json!({
        "type": "integer",
        "x-decision-gate": {
            "allowed_comparators": []
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "empty allowed_comparators should fail");
    let err = result.unwrap_err();
    assert!(err.to_string().contains("must not be empty"), "error should mention empty: {err}");
}

#[test]
fn allowed_comparators_invalid_variant_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(42)));
    let schema = json!({
        "type": "integer",
        "x-decision-gate": {
            "allowed_comparators": ["invalid_comparator"]
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "invalid comparator name should fail");
    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid"), "error should mention invalid comparator: {err}");
}

#[test]
fn override_cannot_enable_forbidden_comparator() {
    let validator = StrictValidator::new(strict_config());
    // GreaterThan is forbidden for boolean, override cannot enable it
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!(true)));
    let schema = json!({
        "type": "boolean",
        "x-decision-gate": {
            "allowed_comparators": ["greater_than"]
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "override cannot enable forbidden comparator");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not valid for schema"),
        "error should mention not valid for schema: {err}"
    );
}

#[test]
fn override_can_enable_optin_comparator() {
    let validator = StrictValidator::new(config_with_lexicographic());
    // LexGreaterThan is opt-in for string, override can enable it
    let spec = spec_with_condition(Comparator::LexGreaterThan, Some(json!("abc")));
    let schema = json!({
        "type": "string",
        "x-decision-gate": {
            "allowed_comparators": ["lex_greater_than"]
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "override should enable opt-in comparator: {result:?}");
}

// ============================================================================
// SECTION: Feature Toggle Tests
// ============================================================================

#[test]
fn lexicographic_disabled_by_default() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::LexGreaterThan, Some(json!("abc")));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "lexicographic should be disabled by default");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("disabled by config")
            || err.to_string().contains("requires explicit opt-in"),
        "error should mention disabled: {err}"
    );
}

#[test]
fn lexicographic_enabled_allows() {
    let validator = StrictValidator::new(config_with_lexicographic());
    let spec = spec_with_condition(Comparator::LexGreaterThan, Some(json!("abc")));
    let schema = json!({
        "type": "string",
        "x-decision-gate": {
            "allowed_comparators": ["lex_greater_than"]
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "lexicographic should be allowed when enabled: {result:?}");
}

#[test]
fn deep_equals_disabled_by_default() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::DeepEquals, Some(json!({"key": "value"})));
    let schema = json!({"type": "object"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "deep_equals should be disabled by default");
}

#[test]
fn deep_equals_enabled_allows() {
    let validator = StrictValidator::new(config_with_deep_equals());
    let spec = spec_with_condition(Comparator::DeepEquals, Some(json!({"key": "value"})));
    // Wrap in object with property for the condition
    let schema = json!({
        "type": "object",
        "properties": {
            "test_pred": {
                "type": "object",
                "x-decision-gate": {
                    "allowed_comparators": ["deep_equals"]
                }
            }
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "deep_equals should be allowed when enabled: {result:?}");
}

// ============================================================================
// SECTION: Expected Value Validation Tests
// ============================================================================

#[test]
fn exists_rejects_expected_value() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Exists, Some(json!(true)));
    let schema = json!({"type": "boolean"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "exists should reject expected value");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("does not accept expected values"),
        "error should mention expected value rejection: {err}"
    );
}

#[test]
fn not_exists_rejects_expected_value() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::NotExists, Some(json!(false)));
    let schema = json!({"type": "boolean"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "not_exists should reject expected value");
}

#[test]
fn in_set_requires_array_expected() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::InSet, Some(json!("not-an-array")));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "in_set should require array expected");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("requires expected array"),
        "error should mention array requirement: {err}"
    );
}

#[test]
fn in_set_validates_each_element() {
    let validator = StrictValidator::new(strict_config());
    // Expected array contains a number, but schema is string
    let spec = spec_with_condition(Comparator::InSet, Some(json!(["valid", 123])));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "in_set should validate each element against schema");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("expected value invalid"),
        "error should mention invalid value: {err}"
    );
}

#[test]
fn equals_validates_expected_against_schema() {
    let validator = StrictValidator::new(strict_config());
    // Expected is string, but schema is integer
    let spec = spec_with_condition(Comparator::Equals, Some(json!("not-an-integer")));
    let schema = json!({"type": "integer"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "equals should validate expected against schema");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("expected value invalid"),
        "error should mention invalid value: {err}"
    );
}

#[test]
fn ordering_comparator_rejects_null_expected() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!(null)));
    let schema = json!({"type": "integer"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "ordering should reject null expected");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("does not accept null expected"),
        "error should mention null rejection: {err}"
    );
}

#[test]
fn contains_rejects_null_expected() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Contains, Some(json!(null)));
    let schema = json!({"type": "string"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "contains should reject null expected");
}

#[test]
fn deep_equals_rejects_null_expected() {
    let validator = StrictValidator::new(config_with_deep_equals());
    let spec = spec_with_condition(Comparator::DeepEquals, Some(json!(null)));
    let schema = json!({
        "type": "object",
        "x-decision-gate": {
            "allowed_comparators": ["deep_equals"]
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "deep_equals should reject null expected");
}

// ============================================================================
// SECTION: Error Path Tests (Security-Critical Fail-Closed Behavior)
// ============================================================================

#[test]
fn schema_missing_type_fails_closed() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({}); // No type declaration
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "schema without type should fail closed");
    let err = result.unwrap_err();
    assert!(err.to_string().contains("missing type"), "error should mention missing type: {err}");
}

#[test]
fn schema_type_invalid_value_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({"type": 123}); // Type should be string or array
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "invalid type value should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("must be string or array"),
        "error should mention type format: {err}"
    );
}

#[test]
fn schema_type_empty_array_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({"type": []});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "empty type array should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("must not be empty"),
        "error should mention empty array: {err}"
    );
}

#[test]
fn enum_empty_values_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!("value")));
    let schema = json!({"enum": []});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "empty enum should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("at least one value"),
        "error should mention empty enum: {err}"
    );
}

#[test]
fn enum_mixed_scalar_types_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!("value")));
    let schema = json!({"enum": ["string", 123, true]}); // Mixed types
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "mixed type enum should fail (TM-VAL-001)");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("same scalar type"),
        "error should mention type consistency: {err}"
    );
}

#[test]
fn enum_complex_values_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!({"key": "value"})));
    let schema = json!({"enum": [{"key": "value"}, {"other": "obj"}]}); // Complex values
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "complex enum values should fail (TM-VAL-001)");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("scalar types"),
        "error should mention scalar requirement: {err}"
    );
}

#[test]
fn additional_properties_true_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({
        "type": "object",
        "additionalProperties": true
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "untyped additionalProperties should fail closed");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("untyped additionalProperties"),
        "error should mention additionalProperties: {err}"
    );
}

#[test]
fn unknown_schema_type_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    let schema = json!({"type": "unknown_type"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "unknown schema type should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("unsupported schema type"),
        "error should mention unsupported type: {err}"
    );
}

#[test]
fn condition_missing_from_data_shape_fails() {
    let validator = StrictValidator::new(strict_config());
    let spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    // Object schema without a property for "test_pred"
    let schema = json!({
        "type": "object",
        "properties": {
            "other_property": {"type": "boolean"}
        }
    });
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "missing condition in schema should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("missing from data shape"),
        "error should mention missing condition: {err}"
    );
}

#[test]
fn non_object_with_multiple_conditions_fails() {
    let validator = StrictValidator::new(strict_config());
    // Create a spec with two conditions
    let mut spec = spec_with_condition(Comparator::Equals, Some(json!(true)));
    spec.conditions.push(ConditionSpec {
        condition_id: "second_pred".into(),
        query: EvidenceQuery {
            provider_id: ProviderId::new("test"),
            check_id: "test".to_string(),
            params: None,
        },
        comparator: Comparator::Equals,
        expected: Some(json!(false)),
        policy_tags: Vec::new(),
        trust: None,
    });
    // Non-object schema (scalar)
    let schema = json!({"type": "boolean"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_err(), "non-object schema with multiple conditions should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("exactly one condition"),
        "error should mention single condition requirement: {err}"
    );
}

// ============================================================================
// SECTION: Strict Mode Toggle Tests
// ============================================================================

#[test]
fn strict_disabled_skips_validation() {
    let validator = StrictValidator::new(permissive_config());
    // This would normally fail (ordering on boolean)
    let spec = spec_with_condition(Comparator::GreaterThan, Some(json!(true)));
    let schema = json!({"type": "boolean"});
    let result = validator.validate_precheck(&spec, &schema);
    assert!(result.is_ok(), "strict=false should skip validation: {result:?}");
}

#[test]
fn enabled_returns_true_when_strict() {
    let validator = StrictValidator::new(strict_config());
    assert!(validator.enabled(), "enabled() should return true when strict");
}

#[test]
fn enabled_returns_false_when_permissive() {
    let validator = StrictValidator::new(permissive_config());
    assert!(!validator.enabled(), "enabled() should return false when not strict");
}
