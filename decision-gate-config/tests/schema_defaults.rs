//! Schema default alignment tests for decision-gate-config.
// decision-gate-config/tests/schema_defaults.rs
// =============================================================================
// Module: Schema Defaults Alignment Tests
// Description: Ensure schema defaults match runtime defaults.
// Purpose: Prevent drift between config defaults and generated schema/docs.
// =============================================================================
use decision_gate_config::config_schema;
use serde_json::Value;

mod common;

type TestResult = Result<(), String>;

fn schema_default<'a>(schema: &'a Value, pointer: &str) -> Result<&'a Value, String> {
    schema.pointer(pointer).ok_or_else(|| format!("missing schema default at {pointer}"))
}

fn assert_default(schema: &Value, pointer: &str, expected: &Value) -> TestResult {
    let actual = schema_default(schema, pointer)?;
    if actual != expected {
        return Err(format!("schema default mismatch at {pointer}: {actual:?} vs {expected:?}"));
    }
    Ok(())
}

#[test]
fn schema_defaults_match_runtime_defaults() -> TestResult {
    let schema = config_schema();
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;

    let transport = serde_json::to_value(config.server.transport).map_err(|err| err.to_string())?;
    assert_default(&schema, "/properties/server/properties/transport/default", &transport)?;
    let mode = serde_json::to_value(config.server.mode).map_err(|err| err.to_string())?;
    assert_default(&schema, "/properties/server/properties/mode/default", &mode)?;
    assert_default(
        &schema,
        "/properties/server/properties/max_body_bytes/default",
        &serde_json::json!(config.server.max_body_bytes),
    )?;
    assert_default(
        &schema,
        "/properties/server/properties/limits/properties/max_inflight/default",
        &serde_json::json!(config.server.limits.max_inflight),
    )?;
    assert_default(
        &schema,
        "/properties/validation/properties/strict/default",
        &serde_json::json!(config.validation.strict),
    )?;
    assert_default(
        &schema,
        "/properties/validation/properties/allow_permissive/default",
        &serde_json::json!(config.validation.allow_permissive),
    )?;
    assert_default(
        &schema,
        "/properties/evidence/properties/allow_raw_values/default",
        &serde_json::json!(config.evidence.allow_raw_values),
    )?;
    assert_default(
        &schema,
        "/properties/evidence/properties/require_provider_opt_in/default",
        &serde_json::json!(config.evidence.require_provider_opt_in),
    )?;
    let min_lane = serde_json::to_value(config.trust.min_lane).map_err(|err| err.to_string())?;
    assert_default(&schema, "/properties/trust/properties/min_lane/default", &min_lane)?;
    assert_default(
        &schema,
        "/properties/provider_discovery/properties/max_response_bytes/default",
        &serde_json::json!(config.provider_discovery.max_response_bytes),
    )?;
    assert_default(
        &schema,
        "/properties/schema_registry/properties/max_schema_bytes/default",
        &serde_json::json!(config.schema_registry.max_schema_bytes),
    )?;
    Ok(())
}
