// decision-gate-mcp/src/validation.rs
// ============================================================================
// Module: Strict Comparator Validation
// Description: Enforces strict comparator compatibility with schema types.
// Purpose: Reject invalid condition definitions before runtime evaluation.
// Dependencies: decision-gate-core, jsonschema
// ============================================================================

//! ## Overview
//! Strict validation enforces comparator/type compatibility and domain-specific
//! comparator allowlists. It is default-on and fails closed when schema
//! metadata is ambiguous or invalid.
//! Security posture: schema validation is a trust boundary and must fail closed
//! on ambiguous inputs; see `Docs/security/threat_model.md`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use decision_gate_core::Comparator;
use decision_gate_core::ConditionSpec;
use decision_gate_core::ScenarioSpec;
use jsonschema::Draft;
use jsonschema::Validator;
use serde_json::Value;
use thiserror::Error;

use crate::capabilities::CapabilityRegistry;
use crate::config::ValidationConfig;

/// Strict validation error.
///
/// # Invariants
/// - Variants are stable for validation error classification.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Invalid condition definition or schema constraints.
    #[error("{0}")]
    Invalid(String),
}

/// Strict comparator validation controller.
///
/// # Invariants
/// - Behavior is fully determined by the stored configuration.
#[derive(Clone)]
pub struct StrictValidator {
    /// Active validation settings.
    config: ValidationConfig,
}

impl StrictValidator {
    /// Creates a strict validator from configuration.
    #[must_use]
    pub const fn new(config: ValidationConfig) -> Self {
        Self {
            config,
        }
    }

    /// Returns true when strict validation is enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.config.strict
    }

    /// Validates a scenario spec against provider result schemas.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] when condition comparators are invalid or schemas are invalid.
    pub fn validate_spec(
        &self,
        spec: &ScenarioSpec,
        capabilities: &CapabilityRegistry,
    ) -> Result<(), ValidationError> {
        if !self.config.strict {
            return Ok(());
        }
        for condition in &spec.conditions {
            let contract = capabilities
                .check_contract(
                    condition.query.provider_id.as_str(),
                    condition.query.check_id.as_str(),
                )
                .map_err(|err| ValidationError::Invalid(err.to_string()))?;
            self.validate_condition_schema(condition, &contract.result_schema)?;
        }
        Ok(())
    }

    /// Validates a scenario spec against an asserted data shape schema.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] when condition comparators are invalid or schemas are invalid.
    pub fn validate_precheck(
        &self,
        spec: &ScenarioSpec,
        data_shape: &Value,
    ) -> Result<(), ValidationError> {
        if !self.config.strict {
            return Ok(());
        }

        let variants = schema_variants(data_shape)?;
        for condition in &spec.conditions {
            let condition_schema_variants =
                condition_schema_variants(condition, spec.conditions.len(), &variants)?;
            for variant in condition_schema_variants {
                self.validate_condition_schema(condition, variant)?;
            }
        }
        Ok(())
    }

    /// Validates a condition against a single schema fragment.
    fn validate_condition_schema(
        &self,
        condition: &ConditionSpec,
        schema: &Value,
    ) -> Result<(), ValidationError> {
        let allowed_override = allowed_comparators_override(schema)?;
        validate_allowed_override(schema, allowed_override.as_deref(), &self.config)?;

        let allowances = comparator_allowances(schema)?;
        let allowance = allowances
            .get(&condition.comparator)
            .copied()
            .unwrap_or(ComparatorAllowance::Forbidden);

        if !comparator_enabled(&self.config, condition.comparator) {
            return Err(ValidationError::Invalid(format!(
                "condition {} comparator {} is disabled by config",
                condition.condition_id.as_str(),
                comparator_label(condition.comparator)
            )));
        }

        if allowance == ComparatorAllowance::Forbidden {
            return Err(ValidationError::Invalid(format!(
                "condition {} comparator {} not allowed for schema type",
                condition.condition_id.as_str(),
                comparator_label(condition.comparator)
            )));
        }

        if allowance == ComparatorAllowance::OptIn {
            let Some(override_list) = allowed_override.as_ref() else {
                return Err(ValidationError::Invalid(format!(
                    "condition {} comparator {} requires explicit opt-in",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                )));
            };
            if !override_list.contains(&condition.comparator) {
                return Err(ValidationError::Invalid(format!(
                    "condition {} comparator {} not in allowed_comparators",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                )));
            }
        }

        if let Some(override_list) = allowed_override.as_ref()
            && !override_list.contains(&condition.comparator)
        {
            return Err(ValidationError::Invalid(format!(
                "condition {} comparator {} not in allowed_comparators",
                condition.condition_id.as_str(),
                comparator_label(condition.comparator)
            )));
        }

        let compiled = compile_schema(schema)?;
        validate_expected_value(condition, &compiled)?;
        Ok(())
    }
}

/// Comparator allowance state for a schema type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComparatorAllowance {
    /// Comparator is always allowed.
    Allowed,
    /// Comparator is allowed only with explicit opt-in.
    OptIn,
    /// Comparator is not allowed.
    Forbidden,
}

/// High-level JSON schema type classifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeClass {
    /// Schema allows any JSON value (dynamic typing).
    Dynamic,
    /// Boolean schema type.
    Boolean,
    /// Integer schema type.
    Integer,
    /// Number schema type.
    Number,
    /// String schema type.
    String,
    /// Enum of scalar values.
    Enum,
    /// Array of scalar values.
    ArrayScalar,
    /// Array of complex values.
    ArrayComplex,
    /// Object schema type.
    Object,
    /// Null schema type.
    Null,
    /// RFC3339 date string.
    Date,
    /// RFC3339 date-time string.
    DateTime,
    /// UUID string.
    Uuid,
}

/// Canonical list of comparators.
const ALL_COMPARATORS: [Comparator; 16] = [
    Comparator::Equals,
    Comparator::NotEquals,
    Comparator::GreaterThan,
    Comparator::GreaterThanOrEqual,
    Comparator::LessThan,
    Comparator::LessThanOrEqual,
    Comparator::LexGreaterThan,
    Comparator::LexGreaterThanOrEqual,
    Comparator::LexLessThan,
    Comparator::LexLessThanOrEqual,
    Comparator::Contains,
    Comparator::InSet,
    Comparator::DeepEquals,
    Comparator::DeepNotEquals,
    Comparator::Exists,
    Comparator::NotExists,
];

/// Computes comparator allowances for a schema definition.
fn comparator_allowances(
    schema: &Value,
) -> Result<BTreeMap<Comparator, ComparatorAllowance>, ValidationError> {
    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        return intersect_allowances(options);
    }
    if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
        return intersect_allowances(options);
    }

    let type_classes = schema_type_classes(schema)?;
    let mut allowances = allowances_for_type(type_classes[0]);
    for class in type_classes.iter().skip(1) {
        let next = allowances_for_type(*class);
        allowances = merge_allowances(&allowances, &next);
    }
    Ok(allowances)
}

/// Intersects comparator allowances across schema union variants.
fn intersect_allowances(
    options: &[Value],
) -> Result<BTreeMap<Comparator, ComparatorAllowance>, ValidationError> {
    let filtered = filter_null_variants(options)?;
    let mut iter = filtered.into_iter();
    let Some(first) = iter.next() else {
        return Err(ValidationError::Invalid(
            "schema union must have at least one option".to_string(),
        ));
    };
    let mut allowances = comparator_allowances(first)?;
    for option in iter {
        let next = comparator_allowances(option)?;
        allowances = merge_allowances(&allowances, &next);
    }
    Ok(allowances)
}

/// Merges comparator allowances across two allowance maps.
fn merge_allowances(
    left: &BTreeMap<Comparator, ComparatorAllowance>,
    right: &BTreeMap<Comparator, ComparatorAllowance>,
) -> BTreeMap<Comparator, ComparatorAllowance> {
    let mut merged = BTreeMap::new();
    for comparator in ALL_COMPARATORS {
        let left_allow = left.get(&comparator).copied().unwrap_or(ComparatorAllowance::Forbidden);
        let right_allow = right.get(&comparator).copied().unwrap_or(ComparatorAllowance::Forbidden);
        merged.insert(comparator, combine_allowances(left_allow, right_allow));
    }
    merged
}

/// Combines two allowance states into the most restrictive outcome.
const fn combine_allowances(
    left: ComparatorAllowance,
    right: ComparatorAllowance,
) -> ComparatorAllowance {
    match (left, right) {
        (ComparatorAllowance::Forbidden, _) | (_, ComparatorAllowance::Forbidden) => {
            ComparatorAllowance::Forbidden
        }
        (ComparatorAllowance::OptIn, _) | (_, ComparatorAllowance::OptIn) => {
            ComparatorAllowance::OptIn
        }
        _ => ComparatorAllowance::Allowed,
    }
}

/// Builds comparator allowances for a specific type classification.
fn allowances_for_type(kind: TypeClass) -> BTreeMap<Comparator, ComparatorAllowance> {
    let mut allowances = BTreeMap::new();
    for comparator in ALL_COMPARATORS {
        let allowance = match kind {
            TypeClass::Dynamic => ComparatorAllowance::Allowed,
            TypeClass::Boolean => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::InSet
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::Integer | TypeClass::Number => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::GreaterThan
                | Comparator::GreaterThanOrEqual
                | Comparator::LessThan
                | Comparator::LessThanOrEqual
                | Comparator::InSet
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::String => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::Contains
                | Comparator::InSet
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                Comparator::LexGreaterThan
                | Comparator::LexGreaterThanOrEqual
                | Comparator::LexLessThan
                | Comparator::LexLessThanOrEqual => ComparatorAllowance::OptIn,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::Enum => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::InSet
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::ArrayScalar => match comparator {
                Comparator::Contains | Comparator::Exists | Comparator::NotExists => {
                    ComparatorAllowance::Allowed
                }
                Comparator::DeepEquals | Comparator::DeepNotEquals => ComparatorAllowance::OptIn,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::ArrayComplex => match comparator {
                Comparator::Exists | Comparator::NotExists => ComparatorAllowance::Allowed,
                Comparator::DeepEquals | Comparator::DeepNotEquals => ComparatorAllowance::OptIn,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::Object => match comparator {
                Comparator::Exists | Comparator::NotExists => ComparatorAllowance::Allowed,
                Comparator::DeepEquals | Comparator::DeepNotEquals => ComparatorAllowance::OptIn,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::Null => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::Date | TypeClass::DateTime => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::GreaterThan
                | Comparator::GreaterThanOrEqual
                | Comparator::LessThan
                | Comparator::LessThanOrEqual
                | Comparator::InSet
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                _ => ComparatorAllowance::Forbidden,
            },
            TypeClass::Uuid => match comparator {
                Comparator::Equals
                | Comparator::NotEquals
                | Comparator::InSet
                | Comparator::Exists
                | Comparator::NotExists => ComparatorAllowance::Allowed,
                _ => ComparatorAllowance::Forbidden,
            },
        };
        allowances.insert(comparator, allowance);
    }
    allowances
}

/// Resolves schema type classes for comparator validation.
fn schema_type_classes(schema: &Value) -> Result<Vec<TypeClass>, ValidationError> {
    if let Some(meta) = schema.get("x-decision-gate") {
        let meta = meta.as_object().ok_or_else(|| {
            ValidationError::Invalid("x-decision-gate must be an object".to_string())
        })?;
        if let Some(dynamic) = meta.get("dynamic_type") {
            let dynamic = dynamic.as_bool().ok_or_else(|| {
                ValidationError::Invalid("x-decision-gate.dynamic_type must be boolean".to_string())
            })?;
            if dynamic {
                return Ok(vec![TypeClass::Dynamic]);
            }
        }
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if values.is_empty() {
            return Err(ValidationError::Invalid(
                "enum must contain at least one value".to_string(),
            ));
        }
        let mut kinds = BTreeSet::new();
        for value in values {
            let kind = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            kinds.insert(kind);
        }
        if kinds.contains("array") || kinds.contains("object") {
            return Err(ValidationError::Invalid("enum values must be scalar types".to_string()));
        }
        if kinds.len() > 1 {
            return Err(ValidationError::Invalid(
                "enum values must share the same scalar type".to_string(),
            ));
        }
        if kinds.contains("null") {
            return Ok(vec![TypeClass::Null]);
        }
        return Ok(vec![TypeClass::Enum]);
    }

    let schema_type = schema
        .get("type")
        .ok_or_else(|| ValidationError::Invalid("schema missing type declaration".to_string()))?;
    match schema_type {
        Value::String(kind) => Ok(vec![type_class_for_kind(kind, schema)?]),
        Value::Array(kinds) => {
            let mut classes = Vec::new();
            for kind in kinds {
                let Some(kind) = kind.as_str() else {
                    return Err(ValidationError::Invalid(
                        "schema type array must contain strings".to_string(),
                    ));
                };
                if kind == "null" {
                    classes.push(TypeClass::Null);
                    continue;
                }
                classes.push(type_class_for_kind(kind, schema)?);
            }
            if classes.is_empty() {
                return Err(ValidationError::Invalid(
                    "schema type array must not be empty".to_string(),
                ));
            }
            if classes.len() > 1 {
                classes.retain(|class| *class != TypeClass::Null);
            }
            Ok(classes)
        }
        _ => Err(ValidationError::Invalid("schema type must be string or array".to_string())),
    }
}

/// Maps a schema type string to a type classification.
fn type_class_for_kind(kind: &str, schema: &Value) -> Result<TypeClass, ValidationError> {
    match kind {
        "boolean" => Ok(TypeClass::Boolean),
        "integer" => Ok(TypeClass::Integer),
        "number" => Ok(TypeClass::Number),
        "string" => Ok(string_type_class(schema)),
        "array" => Ok(array_type_class(schema)),
        "object" => Ok(TypeClass::Object),
        "null" => Ok(TypeClass::Null),
        _ => Err(ValidationError::Invalid(format!("unsupported schema type: {kind}"))),
    }
}

/// Determines the type class for string schemas.
fn string_type_class(schema: &Value) -> TypeClass {
    let format = schema.get("format").and_then(Value::as_str);
    match format {
        Some("date-time") => TypeClass::DateTime,
        Some("date") => TypeClass::Date,
        Some("uuid") => TypeClass::Uuid,
        _ => TypeClass::String,
    }
}

/// Determines the type class for array schemas.
fn array_type_class(schema: &Value) -> TypeClass {
    let Some(items) = schema.get("items") else {
        return TypeClass::ArrayComplex;
    };
    let Ok(classes) = schema_type_classes(items) else {
        return TypeClass::ArrayComplex;
    };
    if classes.iter().copied().all(is_scalar_type) {
        TypeClass::ArrayScalar
    } else {
        TypeClass::ArrayComplex
    }
}

/// Returns true for scalar type classes.
const fn is_scalar_type(class: TypeClass) -> bool {
    matches!(
        class,
        TypeClass::Boolean
            | TypeClass::Integer
            | TypeClass::Number
            | TypeClass::String
            | TypeClass::Enum
            | TypeClass::Date
            | TypeClass::DateTime
            | TypeClass::Uuid
            | TypeClass::Null
    )
}

/// Reads allowed comparator overrides from schema metadata.
fn allowed_comparators_override(
    schema: &Value,
) -> Result<Option<Vec<Comparator>>, ValidationError> {
    let Some(meta) = schema.get("x-decision-gate") else {
        return Ok(None);
    };
    let Some(meta) = meta.as_object() else {
        return Err(ValidationError::Invalid("x-decision-gate must be an object".to_string()));
    };
    let Some(allowed) = meta.get("allowed_comparators") else {
        return Ok(None);
    };
    let allowed = serde_json::from_value::<Vec<Comparator>>(allowed.clone())
        .map_err(|err| ValidationError::Invalid(format!("allowed_comparators invalid: {err}")))?;
    if allowed.is_empty() {
        return Err(ValidationError::Invalid("allowed_comparators must not be empty".to_string()));
    }
    Ok(Some(allowed))
}

/// Validates allowed comparator overrides against schema allowances.
fn validate_allowed_override(
    schema: &Value,
    allowed: Option<&[Comparator]>,
    config: &ValidationConfig,
) -> Result<(), ValidationError> {
    let Some(allowed) = allowed else {
        return Ok(());
    };
    let allowances = comparator_allowances(schema)?;
    for comparator in allowed {
        if !comparator_enabled(config, *comparator) {
            return Err(ValidationError::Invalid(format!(
                "allowed_comparators includes disabled comparator {}",
                comparator_label(*comparator)
            )));
        }
        let allowance =
            allowances.get(comparator).copied().unwrap_or(ComparatorAllowance::Forbidden);
        if allowance == ComparatorAllowance::Forbidden {
            return Err(ValidationError::Invalid(format!(
                "allowed_comparators includes comparator {} not valid for schema",
                comparator_label(*comparator)
            )));
        }
    }
    Ok(())
}

/// Validates condition expected values against the compiled schema.
fn validate_expected_value(
    condition: &ConditionSpec,
    schema: &Validator,
) -> Result<(), ValidationError> {
    match condition.comparator {
        Comparator::Exists | Comparator::NotExists => {
            if condition.expected.is_some() {
                return Err(ValidationError::Invalid(format!(
                    "condition {} comparator {} does not accept expected values",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                )));
            }
            Ok(())
        }
        Comparator::InSet => {
            let expected = condition.expected.as_ref().ok_or_else(|| {
                ValidationError::Invalid(format!(
                    "condition {} comparator {} requires expected array",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                ))
            })?;
            let Value::Array(values) = expected else {
                return Err(ValidationError::Invalid(format!(
                    "condition {} comparator {} requires expected array",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                )));
            };
            for value in values {
                let messages: Vec<String> =
                    schema.iter_errors(value).map(|err| err.to_string()).collect();
                if !messages.is_empty() {
                    return Err(ValidationError::Invalid(format!(
                        "condition {} expected value invalid: {}",
                        condition.condition_id.as_str(),
                        messages.join("; ")
                    )));
                }
            }
            Ok(())
        }
        _ => {
            let expected = condition.expected.as_ref().ok_or_else(|| {
                ValidationError::Invalid(format!(
                    "condition {} comparator {} requires expected value",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                ))
            })?;
            if expected.is_null()
                && matches!(
                    condition.comparator,
                    Comparator::GreaterThan
                        | Comparator::GreaterThanOrEqual
                        | Comparator::LessThan
                        | Comparator::LessThanOrEqual
                        | Comparator::LexGreaterThan
                        | Comparator::LexGreaterThanOrEqual
                        | Comparator::LexLessThan
                        | Comparator::LexLessThanOrEqual
                        | Comparator::Contains
                        | Comparator::DeepEquals
                        | Comparator::DeepNotEquals
                )
            {
                return Err(ValidationError::Invalid(format!(
                    "condition {} comparator {} does not accept null expected values",
                    condition.condition_id.as_str(),
                    comparator_label(condition.comparator)
                )));
            }
            let messages: Vec<String> =
                schema.iter_errors(expected).map(|err| err.to_string()).collect();
            if !messages.is_empty() {
                return Err(ValidationError::Invalid(format!(
                    "condition {} expected value invalid: {}",
                    condition.condition_id.as_str(),
                    messages.join("; ")
                )));
            }
            Ok(())
        }
    }
}

/// Returns schema variants, filtering out null-only options.
fn schema_variants(schema: &Value) -> Result<Vec<&Value>, ValidationError> {
    let one_of = schema.get("oneOf");
    let any_of = schema.get("anyOf");
    if one_of.is_some() && any_of.is_some() {
        return Err(ValidationError::Invalid(
            "schema cannot define both oneOf and anyOf".to_string(),
        ));
    }
    if let Some(options) = one_of.and_then(Value::as_array) {
        if options.is_empty() {
            return Err(ValidationError::Invalid(
                "schema oneOf must contain at least one option".to_string(),
            ));
        }
        return filter_null_variants(options);
    }
    if let Some(options) = any_of.and_then(Value::as_array) {
        if options.is_empty() {
            return Err(ValidationError::Invalid(
                "schema anyOf must contain at least one option".to_string(),
            ));
        }
        return filter_null_variants(options);
    }
    Ok(vec![schema])
}

/// Filters union variants to prefer non-null schemas when present.
fn filter_null_variants(options: &[Value]) -> Result<Vec<&Value>, ValidationError> {
    let mut null_only = Vec::new();
    let mut non_null = Vec::new();
    for option in options {
        if is_null_schema(option)? {
            null_only.push(option);
        } else {
            non_null.push(option);
        }
    }
    if non_null.is_empty() { Ok(null_only) } else { Ok(non_null) }
}

/// Returns true when a schema exclusively represents null values.
fn is_null_schema(schema: &Value) -> Result<bool, ValidationError> {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if values.is_empty() {
            return Err(ValidationError::Invalid(
                "enum must contain at least one value".to_string(),
            ));
        }
        return Ok(values.iter().all(Value::is_null));
    }
    if let Some(schema_type) = schema.get("type") {
        match schema_type {
            Value::String(kind) => return Ok(kind == "null"),
            Value::Array(kinds) => {
                let mut has_null = false;
                let mut has_other = false;
                for kind in kinds {
                    let Some(kind) = kind.as_str() else {
                        return Err(ValidationError::Invalid(
                            "schema type array must contain strings".to_string(),
                        ));
                    };
                    if kind == "null" {
                        has_null = true;
                    } else {
                        has_other = true;
                    }
                }
                return Ok(has_null && !has_other);
            }
            _ => {
                return Err(ValidationError::Invalid(
                    "schema type must be string or array".to_string(),
                ));
            }
        }
    }
    Ok(false)
}

/// Resolves condition-specific schema variants from a union.
fn condition_schema_variants<'a>(
    condition: &ConditionSpec,
    condition_count: usize,
    variants: &'a [&'a Value],
) -> Result<Vec<&'a Value>, ValidationError> {
    let mut schemas = Vec::with_capacity(variants.len());
    for variant in variants {
        let schema = schema_for_condition(condition, condition_count, variant)?;
        schemas.push(schema);
    }
    Ok(schemas)
}

/// Selects the schema fragment for a specific condition.
fn schema_for_condition<'a>(
    condition: &ConditionSpec,
    condition_count: usize,
    schema: &'a Value,
) -> Result<&'a Value, ValidationError> {
    if schema_is_object(schema) {
        if let Some(properties) = schema.get("properties").and_then(Value::as_object)
            && let Some(property_schema) = properties.get(condition.condition_id.as_str())
        {
            return Ok(property_schema);
        }
        if let Some(additional) = schema.get("additionalProperties") {
            if let Some(object_schema) = additional.as_object() {
                let _ = object_schema;
                return Ok(additional);
            }
            if additional == &Value::Bool(true) {
                return Err(ValidationError::Invalid(format!(
                    "schema allows untyped additionalProperties for condition {}",
                    condition.condition_id.as_str()
                )));
            }
        }
        return Err(ValidationError::Invalid(format!(
            "condition {} missing from data shape schema",
            condition.condition_id.as_str()
        )));
    }

    if condition_count == 1 {
        return Ok(schema);
    }

    Err(ValidationError::Invalid(
        "non-object data shape requires exactly one condition".to_string(),
    ))
}

/// Returns true if a schema represents an object.
fn schema_is_object(schema: &Value) -> bool {
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        return kind == "object";
    }
    if let Some(kinds) = schema.get("type").and_then(Value::as_array) {
        return kinds.iter().any(|kind| kind.as_str() == Some("object"));
    }
    schema.get("properties").is_some()
}

/// Compiles a JSON schema for validation.
fn compile_schema(schema: &Value) -> Result<Validator, ValidationError> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(schema)
        .map_err(|err| ValidationError::Invalid(format!("invalid schema: {err}")))
}

/// Returns true if a comparator is enabled by configuration.
const fn comparator_enabled(config: &ValidationConfig, comparator: Comparator) -> bool {
    match comparator {
        Comparator::LexGreaterThan
        | Comparator::LexGreaterThanOrEqual
        | Comparator::LexLessThan
        | Comparator::LexLessThanOrEqual => config.enable_lexicographic,
        Comparator::DeepEquals | Comparator::DeepNotEquals => config.enable_deep_equals,
        _ => true,
    }
}

/// Returns a stable comparator label for error messages.
const fn comparator_label(comparator: Comparator) -> &'static str {
    match comparator {
        Comparator::Equals => "equals",
        Comparator::NotEquals => "not_equals",
        Comparator::GreaterThan => "greater_than",
        Comparator::GreaterThanOrEqual => "greater_than_or_equal",
        Comparator::LessThan => "less_than",
        Comparator::LessThanOrEqual => "less_than_or_equal",
        Comparator::LexGreaterThan => "lex_greater_than",
        Comparator::LexGreaterThanOrEqual => "lex_greater_than_or_equal",
        Comparator::LexLessThan => "lex_less_than",
        Comparator::LexLessThanOrEqual => "lex_less_than_or_equal",
        Comparator::Contains => "contains",
        Comparator::InSet => "in_set",
        Comparator::DeepEquals => "deep_equals",
        Comparator::DeepNotEquals => "deep_not_equals",
        Comparator::Exists => "exists",
        Comparator::NotExists => "not_exists",
    }
}
