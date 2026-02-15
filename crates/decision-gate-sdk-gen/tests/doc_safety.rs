// crates/decision-gate-sdk-gen/tests/doc_safety.rs
// ============================================================================
// Module: SDK Generator Doc Safety Tests
// Description: Ensures generated docs defuse comment terminators from schemas.
// Purpose: Prevent docstring/comment injection in generated SDK outputs.
// Dependencies: decision-gate-sdk-gen, serde_json
// ============================================================================

//! ## Overview
//! Integration tests that exercise schema documentation rendering with hostile
//! strings. These tests ensure comment terminators are defused in generated
//! SDK docs so untrusted schema content cannot break output structure.
//!
//! ### Security Posture
//! Schemas are treated as untrusted input per `Docs/security/threat_model.md`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use decision_gate_sdk_gen::SdkGenError;
use decision_gate_sdk_gen::SdkGenerator;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

// ============================================================================
// CONSTANTS: Temporary file tracking
// ============================================================================

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(label: &str) -> Self {
        let mut path = std::env::temp_dir();
        let attempt = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!("decision-gate-sdk-gen-{label}-{}-{}.json", std::process::id(), attempt));
        Self {
            path,
        }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn write_tooling_fixture(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let tooling = serde_json::json!([
        {
            "name": "scenario_define",
            "description": "Example tool.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["*/"]
                    }
                },
                "required": ["mode"],
                "additionalProperties": false
            },
            "output_schema": {
                "type": "object",
                "additionalProperties": false
            },
            "examples": [],
            "notes": []
        }
    ]);
    let payload = serde_json::to_vec_pretty(&tooling)?;
    fs::write(path, payload)?;
    Ok(())
}

fn write_tooling_fixture_with_property(
    path: &PathBuf,
    property_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut properties = serde_json::Map::new();
    properties.insert(
        property_name.to_string(),
        serde_json::json!({
            "type": "string"
        }),
    );
    let tooling = serde_json::json!([
        {
            "name": "scenario_define",
            "description": "Example tool.",
            "input_schema": {
                "type": "object",
                "properties": serde_json::Value::Object(properties),
                "required": [property_name],
                "additionalProperties": false
            },
            "output_schema": {
                "type": "object",
                "additionalProperties": false
            },
            "examples": [],
            "notes": []
        }
    ]);
    let payload = serde_json::to_vec_pretty(&tooling)?;
    fs::write(path, payload)?;
    Ok(())
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn schema_constraints_defuse_comment_terminators() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempFile::new("doc-safety");
    write_tooling_fixture(&temp.path)?;
    let generator = SdkGenerator::load(&temp.path)?;
    let python = generator.generate_python()?;
    let typescript = generator.generate_typescript()?;
    let expected = "Allowed values: \"* /\"";
    if !python.contains(expected) {
        return Err(std::io::Error::other("Python docs did not defuse */").into());
    }
    if !typescript.contains(expected) {
        return Err(std::io::Error::other("TypeScript docs did not defuse */").into());
    }
    Ok(())
}

#[test]
fn python_generation_rejects_invalid_property_identifiers() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = TempFile::new("python-invalid-property");
    write_tooling_fixture_with_property(&temp.path, "bad-name")?;
    let generator = SdkGenerator::load(&temp.path)?;
    match generator.generate_python() {
        Err(SdkGenError::Tooling(message)) => {
            if !message.contains("bad-name") {
                return Err(std::io::Error::other("error missing property name context").into());
            }
            Ok(())
        }
        Err(other) => Err(std::io::Error::other(format!("unexpected error: {other}")).into()),
        Ok(_) => Err(std::io::Error::other(
            "expected invalid Python identifier error for schema property",
        )
        .into()),
    }
}

#[test]
fn python_generation_rejects_python_keyword_properties() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempFile::new("python-keyword-property");
    write_tooling_fixture_with_property(&temp.path, "class")?;
    let generator = SdkGenerator::load(&temp.path)?;
    match generator.generate_python() {
        Err(SdkGenError::Tooling(message)) => {
            if !message.contains("class") {
                return Err(std::io::Error::other("error missing keyword property context").into());
            }
            Ok(())
        }
        Err(other) => Err(std::io::Error::other(format!("unexpected error: {other}")).into()),
        Ok(_) => {
            Err(std::io::Error::other("expected Python keyword rejection for schema property")
                .into())
        }
    }
}

#[test]
fn typescript_generation_quotes_non_identifier_property_names()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TempFile::new("typescript-quoted-property");
    write_tooling_fixture_with_property(&temp.path, "bad-name")?;
    let generator = SdkGenerator::load(&temp.path)?;
    let typescript = generator.generate_typescript()?;
    if !typescript.contains("\"bad-name\": string;") {
        return Err(std::io::Error::other("TypeScript output did not quote property name").into());
    }
    Ok(())
}
