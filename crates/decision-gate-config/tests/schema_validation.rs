//! Schema validation tests for decision-gate-config.
// crates/decision-gate-config/tests/schema_validation.rs
// =============================================================================
// Module: Schema Validation Tests
// Description: Comprehensive tests for schema completeness and correctness.
// Purpose: Ensure JSON schema accurately represents config model and constraints.
// =============================================================================

use decision_gate_config::config_schema;
use jsonschema::Draft;
use jsonschema::Validator;
use serde_json::Value;
use serde_json::json;

type TestResult = Result<(), String>;

fn compile_schema(schema: &Value) -> Result<Validator, String> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(schema)
        .map_err(|err| format!("failed to compile schema: {err}"))
}

/// Helper to get schema property by pointer
fn schema_property<'a>(schema: &'a Value, pointer: &str) -> Result<&'a Value, String> {
    schema.pointer(pointer).ok_or_else(|| format!("missing schema property at {pointer}"))
}

// ============================================================================
// SECTION: Schema Completeness
// ============================================================================

#[test]
fn schema_contains_all_top_level_fields() -> TestResult {
    let schema = config_schema();
    let properties = schema_property(&schema, "/properties")?;

    let required_fields = vec![
        "server",
        "namespace",
        "dev",
        "trust",
        "evidence",
        "anchors",
        "provider_discovery",
        "validation",
        "policy",
        "run_state_store",
        "schema_registry",
        "providers",
        "runpack_storage",
    ];

    for field in required_fields {
        if properties.get(field).is_none() {
            return Err(format!("schema missing top-level field: {field}"));
        }
    }

    Ok(())
}

#[test]
fn schema_server_section_complete() -> TestResult {
    let schema = config_schema();
    let server_props = schema_property(&schema, "/properties/server/properties")?;

    let required_fields =
        vec!["transport", "mode", "bind", "max_body_bytes", "limits", "auth", "tls", "audit"];

    for field in required_fields {
        if server_props.get(field).is_none() {
            return Err(format!("schema missing server field: {field}"));
        }
    }

    Ok(())
}

#[test]
fn schema_validation_section_complete() -> TestResult {
    let schema = config_schema();
    let validation_props = schema_property(&schema, "/properties/validation/properties")?;

    let required_fields =
        vec!["strict", "profile", "allow_permissive", "enable_lexicographic", "enable_deep_equals"];

    for field in required_fields {
        if validation_props.get(field).is_none() {
            return Err(format!("schema missing validation field: {field}"));
        }
    }

    Ok(())
}

#[test]
fn schema_auth_section_complete() -> TestResult {
    let schema = config_schema();
    let auth_props = schema_property(&schema, "/properties/server/properties/auth/oneOf")?;

    // Auth can be null or an object with specific properties
    if !auth_props.is_array() {
        return Err("auth schema should have oneOf with null and object".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema Constraint Correctness
// ============================================================================

#[test]
fn schema_max_items_matches_max_auth_tokens() -> TestResult {
    let schema = config_schema();

    // Check bearer_tokens maxItems
    let bearer_tokens_schema = schema_property(
        &schema,
        "/properties/server/properties/auth/oneOf/1/properties/bearer_tokens",
    )?;
    let max_items = bearer_tokens_schema
        .get("maxItems")
        .and_then(serde_json::Value::as_u64)
        .ok_or("bearer_tokens missing maxItems")?;

    if max_items != 64 {
        return Err(format!("bearer_tokens maxItems should be 64, got {max_items}"));
    }

    // Check mtls_subjects maxItems
    let mtls_subjects_schema = schema_property(
        &schema,
        "/properties/server/properties/auth/oneOf/1/properties/mtls_subjects",
    )?;
    let max_items = mtls_subjects_schema
        .get("maxItems")
        .and_then(serde_json::Value::as_u64)
        .ok_or("mtls_subjects missing maxItems")?;

    if max_items != 64 {
        return Err(format!("mtls_subjects maxItems should be 64, got {max_items}"));
    }

    Ok(())
}

#[test]
fn schema_max_items_matches_max_auth_tool_rules() -> TestResult {
    let schema = config_schema();

    let allowed_tools_schema = schema_property(
        &schema,
        "/properties/server/properties/auth/oneOf/1/properties/allowed_tools",
    )?;
    let max_items = allowed_tools_schema
        .get("maxItems")
        .and_then(serde_json::Value::as_u64)
        .ok_or("allowed_tools missing maxItems")?;

    if max_items != 128 {
        return Err(format!("allowed_tools maxItems should be 128, got {max_items}"));
    }

    Ok(())
}

#[test]
fn schema_max_length_matches_max_auth_token_length() -> TestResult {
    let schema = config_schema();

    let bearer_token_items_schema = schema_property(
        &schema,
        "/properties/server/properties/auth/oneOf/1/properties/bearer_tokens/items",
    )?;
    let max_length = bearer_token_items_schema
        .get("maxLength")
        .and_then(serde_json::Value::as_u64)
        .ok_or("bearer_token items missing maxLength")?;

    if max_length != 256 {
        return Err(format!("bearer_token maxLength should be 256, got {max_length}"));
    }

    Ok(())
}

#[test]
fn schema_bearer_token_pattern_rejects_whitespace_and_controls() -> TestResult {
    let schema = config_schema();
    let bearer_token_items_schema = schema_property(
        &schema,
        "/properties/server/properties/auth/oneOf/1/properties/bearer_tokens/items",
    )?;
    let pattern = bearer_token_items_schema
        .get("pattern")
        .and_then(serde_json::Value::as_str)
        .ok_or("bearer_token items missing pattern")?;

    if pattern != "^[^\\s\\x00-\\x1F\\x7F]+$" {
        return Err(format!("unexpected bearer token pattern: {pattern}"));
    }
    Ok(())
}

#[test]
fn schema_max_length_matches_max_auth_subject_length() -> TestResult {
    let schema = config_schema();

    let mtls_subject_items_schema = schema_property(
        &schema,
        "/properties/server/properties/auth/oneOf/1/properties/mtls_subjects/items",
    )?;
    let max_length = mtls_subject_items_schema
        .get("maxLength")
        .and_then(serde_json::Value::as_u64)
        .ok_or("mtls_subject items missing maxLength")?;

    if max_length != 512 {
        return Err(format!("mtls_subject maxLength should be 512, got {max_length}"));
    }

    Ok(())
}

#[test]
fn schema_timeout_minimum_maximum_correct() -> TestResult {
    let schema = config_schema();

    // Check provider connect_timeout_ms
    let connect_timeout_schema = schema_property(
        &schema,
        "/properties/providers/items/properties/timeouts/properties/connect_timeout_ms",
    )?;
    let minimum = connect_timeout_schema
        .get("minimum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("connect_timeout_ms missing minimum")?;
    let maximum = connect_timeout_schema
        .get("maximum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("connect_timeout_ms missing maximum")?;

    if minimum != 100 {
        return Err(format!("connect_timeout_ms minimum should be 100, got {minimum}"));
    }
    if maximum != 10_000 {
        return Err(format!("connect_timeout_ms maximum should be 10000, got {maximum}"));
    }

    // Check provider request_timeout_ms
    let request_timeout_schema = schema_property(
        &schema,
        "/properties/providers/items/properties/timeouts/properties/request_timeout_ms",
    )?;
    let minimum = request_timeout_schema
        .get("minimum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("request_timeout_ms missing minimum")?;
    let maximum = request_timeout_schema
        .get("maximum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("request_timeout_ms missing maximum")?;

    if minimum != 500 {
        return Err(format!("request_timeout_ms minimum should be 500, got {minimum}"));
    }
    if maximum != 30_000 {
        return Err(format!("request_timeout_ms maximum should be 30000, got {maximum}"));
    }

    Ok(())
}

#[test]
fn schema_provider_url_pattern_requires_http_or_https() -> TestResult {
    let schema = config_schema();
    let url_schema =
        schema_property(&schema, "/properties/providers/items/properties/url/oneOf/1/pattern")?;
    let pattern = url_schema.as_str().ok_or("provider url pattern must be a string")?;
    if pattern != "^https?://\\S+$" {
        return Err(format!("unexpected provider url pattern: {pattern}"));
    }
    Ok(())
}

#[test]
fn schema_runpack_endpoint_pattern_requires_http_or_https() -> TestResult {
    let schema = config_schema();
    let endpoint_schema = schema_property(
        &schema,
        "/properties/runpack_storage/oneOf/1/properties/endpoint/oneOf/1/pattern",
    )?;
    let pattern = endpoint_schema.as_str().ok_or("runpack endpoint pattern must be a string")?;
    if pattern != "^https?://\\S+$" {
        return Err(format!("unexpected runpack endpoint pattern: {pattern}"));
    }
    Ok(())
}

#[test]
fn schema_rate_limit_constraints_correct() -> TestResult {
    let schema = config_schema();

    // Check window_ms
    let window_ms_schema = schema_property(
        &schema,
        "/properties/server/properties/limits/properties/rate_limit/oneOf/1/properties/window_ms",
    )?;
    let minimum = window_ms_schema
        .get("minimum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("window_ms missing minimum")?;
    let maximum = window_ms_schema
        .get("maximum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("window_ms missing maximum")?;

    if minimum != 100 {
        return Err(format!("window_ms minimum should be 100, got {minimum}"));
    }
    if maximum != 60_000 {
        return Err(format!("window_ms maximum should be 60000, got {maximum}"));
    }

    // Check max_requests
    let max_requests_schema = schema_property(
        &schema,
        "/properties/server/properties/limits/properties/rate_limit/oneOf/1/properties/\
         max_requests",
    )?;
    let minimum = max_requests_schema
        .get("minimum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("max_requests missing minimum")?;
    let maximum = max_requests_schema
        .get("maximum")
        .and_then(serde_json::Value::as_u64)
        .ok_or("max_requests missing maximum")?;

    if minimum != 1 {
        return Err(format!("max_requests minimum should be 1, got {minimum}"));
    }
    if maximum != 100_000 {
        return Err(format!("max_requests maximum should be 100000, got {maximum}"));
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema Conditional Validation
// ============================================================================

#[test]
fn schema_bearer_token_mode_requires_tokens_min_items() -> TestResult {
    let schema = config_schema();

    let auth_schema = schema_property(&schema, "/properties/server/properties/auth/oneOf/1")?;
    let all_of =
        auth_schema.get("allOf").and_then(|v| v.as_array()).ok_or("auth schema missing allOf")?;

    // Check for conditional validation: bearer_token mode requires bearer_tokens
    let has_bearer_token_condition = all_of.iter().any(|condition| {
        condition
            .get("if")
            .and_then(|v| v.get("properties"))
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.get("const"))
            .and_then(|v| v.as_str())
            == Some("bearer_token")
    });

    if !has_bearer_token_condition {
        return Err("schema missing conditional validation for bearer_token mode".to_string());
    }

    Ok(())
}

#[test]
fn schema_mtls_mode_requires_subjects_min_items() -> TestResult {
    let schema = config_schema();

    let auth_schema = schema_property(&schema, "/properties/server/properties/auth/oneOf/1")?;
    let all_of =
        auth_schema.get("allOf").and_then(|v| v.as_array()).ok_or("auth schema missing allOf")?;

    // Check for conditional validation: mtls mode requires mtls_subjects
    let has_mtls_condition = all_of.iter().any(|condition| {
        condition
            .get("if")
            .and_then(|v| v.get("properties"))
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.get("const"))
            .and_then(|v| v.as_str())
            == Some("mtls")
    });

    if !has_mtls_condition {
        return Err("schema missing conditional validation for mtls mode".to_string());
    }

    Ok(())
}

#[test]
fn schema_http_sse_transport_requires_bind() -> TestResult {
    let schema = config_schema();

    let server_schema = schema_property(&schema, "/properties/server")?;
    let all_of = server_schema
        .get("allOf")
        .and_then(|v| v.as_array())
        .ok_or("server schema missing allOf")?;

    // Check for conditional validation: http/sse requires bind
    let has_transport_condition = all_of.iter().any(|condition| {
        condition
            .get("if")
            .and_then(|v| v.get("properties"))
            .and_then(|v| v.get("transport"))
            .and_then(|v| v.get("enum"))
            .is_some()
    });

    if !has_transport_condition {
        return Err("schema missing conditional validation for http/sse transport".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema additionalProperties=false Enforcement
// ============================================================================

#[test]
fn schema_all_objects_have_additional_properties_false() -> TestResult {
    let schema = config_schema();

    // Check top-level schema
    let additional_properties = schema
        .get("additionalProperties")
        .and_then(serde_json::Value::as_bool)
        .ok_or("top-level schema missing additionalProperties")?;

    if additional_properties {
        return Err("top-level schema should have additionalProperties=false".to_string());
    }

    // Check server section
    let server_schema = schema_property(&schema, "/properties/server")?;
    let additional_properties = server_schema
        .get("additionalProperties")
        .and_then(serde_json::Value::as_bool)
        .ok_or("server schema missing additionalProperties")?;

    if additional_properties {
        return Err("server schema should have additionalProperties=false".to_string());
    }

    Ok(())
}

#[test]
fn schema_rejects_unknown_top_level_field() -> TestResult {
    let schema = config_schema();
    let json_schema = compile_schema(&schema)?;

    let invalid_config = json!({
        "server": {},
        "unknown_field": "value"
    });

    if json_schema.validate(&invalid_config).is_ok() {
        return Err("schema should reject unknown top-level field".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema Structure
// ============================================================================

#[test]
fn schema_has_correct_schema_field() -> TestResult {
    let schema = config_schema();

    let schema_field =
        schema.get("$schema").and_then(|v| v.as_str()).ok_or("schema missing $schema field")?;

    if !schema_field.contains("json-schema.org") {
        return Err(format!("$schema field should reference json-schema.org, got {schema_field}"));
    }

    Ok(())
}

#[test]
fn schema_has_correct_id_field() -> TestResult {
    let schema = config_schema();

    let id_field = schema.get("$id").and_then(|v| v.as_str()).ok_or("schema missing $id field")?;

    if !id_field.contains("decision-gate") {
        return Err(format!("$id field should reference decision-gate, got {id_field}"));
    }

    Ok(())
}

#[test]
fn schema_has_title_and_description() -> TestResult {
    let schema = config_schema();

    let title = schema.get("title").and_then(|v| v.as_str()).ok_or("schema missing title")?;

    let description =
        schema.get("description").and_then(|v| v.as_str()).ok_or("schema missing description")?;

    if title.is_empty() {
        return Err("schema title is empty".to_string());
    }

    if description.is_empty() {
        return Err("schema description is empty".to_string());
    }

    Ok(())
}

#[test]
fn schema_types_correct() -> TestResult {
    let schema = config_schema();

    // Top-level should be object
    let schema_type = schema.get("type").and_then(|v| v.as_str()).ok_or("schema missing type")?;

    if schema_type != "object" {
        return Err(format!("top-level schema type should be object, got {schema_type}"));
    }

    // Server should be object
    let server_schema = schema_property(&schema, "/properties/server")?;
    let server_type =
        server_schema.get("type").and_then(|v| v.as_str()).ok_or("server schema missing type")?;

    if server_type != "object" {
        return Err(format!("server schema type should be object, got {server_type}"));
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema Generation Determinism
// ============================================================================

#[test]
fn schema_generation_is_deterministic() -> TestResult {
    let schema1 = config_schema();
    let schema2 = config_schema();

    let json1 = serde_json::to_string(&schema1)
        .map_err(|err| format!("failed to serialize schema1: {err}"))?;
    let json2 = serde_json::to_string(&schema2)
        .map_err(|err| format!("failed to serialize schema2: {err}"))?;

    if json1 != json2 {
        return Err("schema generation is not deterministic".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Invalid Configs Rejected by Schema
// ============================================================================

#[test]
fn schema_rejects_config_with_wrong_type() -> TestResult {
    let schema = config_schema();
    let json_schema = compile_schema(&schema)?;

    let invalid_config = json!({
        "server": {
            "max_body_bytes": "not a number"
        }
    });

    if json_schema.validate(&invalid_config).is_ok() {
        return Err("schema should reject wrong type".to_string());
    }

    Ok(())
}

#[test]
fn schema_rejects_config_violating_min_items() -> TestResult {
    let schema = config_schema();
    let json_schema = compile_schema(&schema)?;

    let invalid_config = json!({
        "server": {
            "auth": {
                "mode": "bearer_token",
                "bearer_tokens": []
            }
        }
    });

    if json_schema.validate(&invalid_config).is_ok() {
        return Err("schema should reject empty bearer_tokens with bearer_token mode".to_string());
    }

    Ok(())
}

#[test]
fn schema_rejects_config_violating_max_items() -> TestResult {
    let schema = config_schema();
    let json_schema = compile_schema(&schema)?;

    let too_many_tokens: Vec<String> = (0 .. 65).map(|i| format!("token{i}")).collect();
    let invalid_config = json!({
        "server": {
            "auth": {
                "mode": "bearer_token",
                "bearer_tokens": too_many_tokens
            }
        }
    });

    if json_schema.validate(&invalid_config).is_ok() {
        return Err("schema should reject too many bearer_tokens".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema Enum Coverage
// ============================================================================

#[test]
fn schema_transport_enum_complete() -> TestResult {
    let schema = config_schema();

    let transport_schema = schema_property(&schema, "/properties/server/properties/transport")?;
    let enum_values = transport_schema
        .get("enum")
        .and_then(|v| v.as_array())
        .ok_or("transport schema missing enum")?;

    let required_values = vec!["stdio", "http", "sse"];
    for value in required_values {
        if !enum_values.iter().any(|v| v.as_str() == Some(value)) {
            return Err(format!("transport enum missing value: {value}"));
        }
    }

    Ok(())
}

#[test]
fn schema_auth_mode_enum_complete() -> TestResult {
    let schema = config_schema();

    let auth_mode_schema =
        schema_property(&schema, "/properties/server/properties/auth/oneOf/1/properties/mode")?;
    let enum_values = auth_mode_schema
        .get("enum")
        .and_then(|v| v.as_array())
        .ok_or("auth mode schema missing enum")?;

    let required_values = vec!["local_only", "bearer_token", "mtls"];
    for value in required_values {
        if !enum_values.iter().any(|v| v.as_str() == Some(value)) {
            return Err(format!("auth mode enum missing value: {value}"));
        }
    }

    Ok(())
}
