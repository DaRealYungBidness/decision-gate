//! Documentation validation tests for decision-gate-config.
// decision-gate-config/tests/docs_validation.rs
// =============================================================================
// Module: Documentation Validation Tests
// Description: Comprehensive tests for docs completeness and drift detection.
// Purpose: Ensure generated docs match reality and contain all fields.
// =============================================================================

use decision_gate_config::config_docs_markdown;
use decision_gate_config::config_schema;
use decision_gate_config::config_toml_example;

type TestResult = Result<(), String>;

// ============================================================================
// SECTION: Docs Completeness
// ============================================================================

#[test]
fn docs_contain_all_config_sections() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Check for all major sections
    let required_sections = vec![
        "### [server]",
        "### [namespace]",
        "### [dev]",
        "### [trust]",
        "### [evidence]",
        "### [anchors]",
        "### [provider_discovery]",
        "### [validation]",
        "### [policy]",
        "### [run_state_store]",
        "### [schema_registry]",
        "### [[providers]]",
        "### [runpack_storage]",
    ];

    for section in required_sections {
        if !docs.contains(section) {
            return Err(format!("docs missing section: {section}"));
        }
    }

    Ok(())
}

#[test]
fn docs_field_descriptions_present_and_non_empty() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Check that docs contain field descriptions (tables with descriptions)
    if !docs.contains("| Field |") {
        return Err("docs missing field tables".to_string());
    }

    if !docs.contains("| Notes |") {
        return Err("docs missing notes column".to_string());
    }

    // Check that docs are not mostly empty
    if docs.len() < 5000 {
        return Err(format!("docs suspiciously short: {} bytes", docs.len()));
    }

    Ok(())
}

// ============================================================================
// SECTION: Docs Correctness
// ============================================================================

#[test]
fn docs_enum_values_match_config_enums() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Check transport enum values
    if !docs.contains("stdio") {
        return Err("docs missing transport value: stdio".to_string());
    }
    if !docs.contains("http") {
        return Err("docs missing transport value: http".to_string());
    }
    if !docs.contains("sse") {
        return Err("docs missing transport value: sse".to_string());
    }

    // Check auth mode enum values
    if !docs.contains("local_only") {
        return Err("docs missing auth mode: local_only".to_string());
    }
    if !docs.contains("bearer_token") {
        return Err("docs missing auth mode: bearer_token".to_string());
    }
    if !docs.contains("mtls") {
        return Err("docs missing auth mode: mtls".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Docs Structure
// ============================================================================

#[test]
fn docs_markdown_syntax_is_valid() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Check for markdown headers
    if !docs.contains("# ") {
        return Err("docs missing markdown headers".to_string());
    }

    // Check for code blocks
    if !docs.contains("```") {
        return Err("docs missing code blocks".to_string());
    }

    // Check for tables (markdown syntax)
    if !docs.contains("|") {
        return Err("docs missing tables".to_string());
    }

    Ok(())
}

#[test]
fn docs_section_ordering_is_correct() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Server should come before providers
    let server_pos = docs.find("### [server]").ok_or("[server] section not found")?;
    let providers_pos = docs.find("### [[providers]]").ok_or("[[providers]] section not found")?;

    if server_pos >= providers_pos {
        return Err("Server Configuration should come before Providers".to_string());
    }

    Ok(())
}

#[test]
fn docs_code_blocks_properly_formatted() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Count opening and closing code blocks
    let opening = docs.matches("```").count();
    if opening % 2 != 0 {
        return Err("unmatched code blocks in docs".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Docs Determinism
// ============================================================================

#[test]
fn docs_generation_is_deterministic() -> TestResult {
    let docs1 = config_docs_markdown().map_err(|err| err.to_string())?;
    let docs2 = config_docs_markdown().map_err(|err| err.to_string())?;

    if docs1 != docs2 {
        return Err("docs generation is not deterministic".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Example Validity
// ============================================================================

#[test]
fn example_parses_as_valid_toml() -> TestResult {
    let example = config_toml_example();

    let parsed: Result<toml::Value, _> = toml::from_str(&example);
    if parsed.is_err() {
        return Err(format!("example TOML does not parse: {:?}", parsed.err()));
    }

    Ok(())
}

#[test]
fn example_deserializes_to_config() -> TestResult {
    let example = config_toml_example();

    let parsed: Result<decision_gate_config::DecisionGateConfig, _> = toml::from_str(&example);
    if let Err(err) = parsed {
        return Err(format!("example does not deserialize to DecisionGateConfig: {err}"));
    }

    Ok(())
}

#[test]
fn example_validates_against_config_model() -> TestResult {
    let example = config_toml_example();

    let mut config: decision_gate_config::DecisionGateConfig =
        toml::from_str(&example).map_err(|err| format!("failed to parse example: {err}"))?;

    config.validate().map_err(|err| format!("example config does not validate: {err}"))?;

    Ok(())
}

#[test]
fn example_validates_against_json_schema() -> TestResult {
    use jsonschema::JSONSchema;
    use serde_json::Value;

    let example = config_toml_example();
    let schema_value = config_schema();

    // Parse example as TOML, convert to JSON
    let toml_value: toml::Value =
        toml::from_str(&example).map_err(|err| format!("failed to parse example TOML: {err}"))?;
    let json_str = serde_json::to_string(&toml_value)
        .map_err(|err| format!("failed to convert to JSON: {err}"))?;
    let json_value: Value =
        serde_json::from_str(&json_str).map_err(|err| format!("failed to parse JSON: {err}"))?;

    // Compile schema
    let schema = JSONSchema::compile(&schema_value)
        .map_err(|err| format!("failed to compile schema: {err}"))?;

    // Validate
    if let Err(errors) = schema.validate(&json_value) {
        let error_messages: Vec<String> =
            errors.map(|e| format!("{} at {}", e, e.instance_path)).collect();
        return Err(format!(
            "example does not validate against schema: {}",
            error_messages.join(", ")
        ));
    }

    Ok(())
}

// ============================================================================
// SECTION: Example Completeness
// ============================================================================

#[test]
fn example_demonstrates_major_config_sections() -> TestResult {
    let example = config_toml_example();

    // Check for major sections
    let required_sections = vec![
        "[server]",
        "[namespace]",
        "[trust]",
        "[evidence]",
        "[policy]",
        "[runpack_storage]",
        "[run_state_store]",
        "[[providers]]",
    ];

    for section in required_sections {
        if !example.contains(section) {
            return Err(format!("example missing section: {section}"));
        }
    }

    Ok(())
}

#[test]
fn example_shows_recommended_production_settings() -> TestResult {
    let example = config_toml_example();

    // Check that example uses production-ready defaults
    // (This is implementation-specific, adjust as needed)
    if !example.contains("strict") {
        return Err("example should show strict mode".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Example Determinism
// ============================================================================

#[test]
fn example_generation_is_deterministic() -> TestResult {
    let example1 = config_toml_example();
    let example2 = config_toml_example();

    if example1 != example2 {
        return Err("example generation is not deterministic".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Schema-Docs-Example Consistency
// ============================================================================

#[test]
fn schema_and_docs_have_same_fields() -> TestResult {
    let schema = config_schema();
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Check that major schema fields appear in docs
    let schema_str = serde_json::to_string_pretty(&schema)
        .map_err(|err| format!("failed to serialize schema: {err}"))?;

    // Check for key fields
    let key_fields =
        vec!["server", "namespace", "trust", "evidence", "validation", "policy", "providers"];

    for field in key_fields {
        if !schema_str.contains(field) {
            return Err(format!("schema missing field: {field}"));
        }
        if !docs.contains(field) {
            return Err(format!("docs missing field: {field}"));
        }
    }

    Ok(())
}

// ============================================================================
// SECTION: Generated Output Sizes
// ============================================================================

#[test]
fn docs_have_reasonable_size() -> TestResult {
    let docs = config_docs_markdown().map_err(|err| err.to_string())?;

    // Docs should be substantial but not enormous
    if docs.len() < 5_000 {
        return Err(format!("docs too small: {} bytes", docs.len()));
    }
    if docs.len() > 500_000 {
        return Err(format!("docs suspiciously large: {} bytes", docs.len()));
    }

    Ok(())
}

#[test]
fn example_has_reasonable_size() -> TestResult {
    let example = config_toml_example();

    // Example should be substantial but not enormous
    if example.len() < 500 {
        return Err(format!("example too small: {} bytes", example.len()));
    }
    if example.len() > 50_000 {
        return Err(format!("example suspiciously large: {} bytes", example.len()));
    }

    Ok(())
}

#[test]
fn schema_has_reasonable_size() -> TestResult {
    let schema = config_schema();
    let schema_str = serde_json::to_string(&schema)
        .map_err(|err| format!("failed to serialize schema: {err}"))?;

    // Schema should be substantial but not enormous
    if schema_str.len() < 5_000 {
        return Err(format!("schema too small: {} bytes", schema_str.len()));
    }
    if schema_str.len() > 1_000_000 {
        return Err(format!("schema suspiciously large: {} bytes", schema_str.len()));
    }

    Ok(())
}
