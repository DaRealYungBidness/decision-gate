// decision-gate-contract/src/authoring.rs
// ============================================================================
// Module: Authoring Formats
// Description: ScenarioSpec authoring parsing and normalization helpers.
// Purpose: Validate and canonicalize authoring inputs into RFC 8785 JSON.
// Dependencies: decision-gate-core, jsonschema, ron, serde_json
// ============================================================================

//! ## Overview
//! This module validates and normalizes `ScenarioSpec` authoring inputs. JSON is
//! the canonical format; RON is accepted for human-friendly authoring and is
//! normalized into canonical JSON (RFC 8785 / JCS).
//! Security posture: authoring inputs are untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fmt;
use std::path::Path;

use decision_gate_core::ScenarioSpec;
use decision_gate_core::hashing::HashDigest;
use decision_gate_core::hashing::canonical_json_bytes;
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use serde_json::Value;
use thiserror::Error;

use crate::schemas;

// ============================================================================
// SECTION: Authoring Formats
// ============================================================================

/// Supported authoring formats for `ScenarioSpec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringFormat {
    /// Canonical JSON authoring format.
    Json,
    /// Human-friendly RON authoring format.
    Ron,
}

impl AuthoringFormat {
    /// Returns the lowercase label for the format.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Ron => "ron",
        }
    }

    /// Returns the preferred file extension for the format.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        self.label()
    }

    /// Parses a format from a file extension.
    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "ron" => Some(Self::Ron),
            _ => None,
        }
    }
}

impl fmt::Display for AuthoringFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

// ============================================================================
// SECTION: Normalized Outputs
// ============================================================================

/// Normalized `ScenarioSpec` output with canonical JSON and hash metadata.
#[derive(Debug, Clone)]
pub struct NormalizedScenario {
    /// Parsed scenario specification.
    pub spec: ScenarioSpec,
    /// Canonical JSON bytes for the scenario spec (RFC 8785).
    pub canonical_json: Vec<u8>,
    /// Canonical spec hash used in runpacks and audits.
    pub spec_hash: HashDigest,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Errors raised while normalizing authoring inputs.
#[derive(Debug, Error)]
pub enum AuthoringError {
    /// Failed to parse the authoring input.
    #[error("failed to parse {format} input: {error}")]
    Parse {
        /// Format that failed to parse.
        format: AuthoringFormat,
        /// Underlying parse error message.
        error: String,
    },
    /// JSON Schema validation failed.
    #[error("schema validation failed: {error}")]
    Schema {
        /// Schema validation details.
        error: String,
    },
    /// Failed to deserialize into core `ScenarioSpec` types.
    #[error("failed to deserialize ScenarioSpec: {error}")]
    Deserialize {
        /// Deserialization error details.
        error: String,
    },
    /// `ScenarioSpec` semantic validation failed.
    #[error("ScenarioSpec validation failed: {error}")]
    Spec {
        /// `ScenarioSpec` validation error details.
        error: String,
    },
    /// Canonical JSON serialization failed.
    #[error("canonicalization failed: {error}")]
    Canonicalization {
        /// Canonicalization error details.
        error: String,
    },
}

// ============================================================================
// SECTION: Public API
// ============================================================================

/// Detects the authoring format from a file path.
#[must_use]
pub fn detect_format(path: &Path) -> Option<AuthoringFormat> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(AuthoringFormat::from_extension)
}

/// Normalize `ScenarioSpec` authoring input into canonical JSON bytes.
///
/// # Errors
///
/// Returns [`AuthoringError`] when parsing, validation, or canonicalization
/// fails.
#[must_use = "use the normalized scenario output or handle the error"]
pub fn normalize_scenario(
    input: &str,
    format: AuthoringFormat,
) -> Result<NormalizedScenario, AuthoringError> {
    let value = parse_value(input, format)?;
    validate_scenario_schema(&value)?;
    let spec: ScenarioSpec =
        serde_json::from_value(value).map_err(|err| AuthoringError::Deserialize {
            error: err.to_string(),
        })?;
    spec.validate().map_err(|err| AuthoringError::Spec {
        error: err.to_string(),
    })?;
    let canonical_json =
        canonical_json_bytes(&spec).map_err(|err| AuthoringError::Canonicalization {
            error: err.to_string(),
        })?;
    let spec_hash = spec.canonical_hash().map_err(|err| AuthoringError::Canonicalization {
        error: err.to_string(),
    })?;
    Ok(NormalizedScenario {
        spec,
        canonical_json,
        spec_hash,
    })
}

/// Build markdown documentation for authoring formats.
#[must_use]
pub fn authoring_markdown() -> String {
    let mut out = String::new();
    out.push_str("# Decision Gate Authoring Formats\n\n");
    out.push_str("Decision Gate accepts ScenarioSpec authoring input in JSON or RON. ");
    out.push_str("JSON is the canonical format used for hashing, schemas, and runpacks. ");
    out.push_str("RON exists only as a human-friendly authoring layer and must be ");
    out.push_str("normalized to canonical JSON before execution.\n\n");
    out.push_str("## Canonical JSON\n\n");
    out.push_str("- Canonical JSON uses RFC 8785 (JCS) for deterministic ordering.\n");
    out.push_str("- ScenarioSpec hashes are computed over canonical JSON bytes.\n");
    out.push_str("- Canonical JSON is emitted by `decision-gate authoring normalize`.\n\n");
    out.push_str("## Supported Inputs\n\n");
    out.push_str("- JSON: canonical format for storage, hashing, and validation.\n");
    out.push_str("- RON: authoring-only format normalized to canonical JSON.\n");
    out.push_str("- YAML: not supported by default (add only with explicit requirement).\n\n");
    out.push_str("## Normalization Pipeline\n\n");
    out.push_str("1. Parse JSON or RON into a structured value.\n");
    out.push_str("2. Validate against `schemas/scenario.schema.json`.\n");
    out.push_str("3. Run ScenarioSpec semantic validation (IDs, predicates, gates).\n");
    out.push_str("4. Canonicalize to JSON (RFC 8785).\n");
    out.push_str("5. Compute the canonical spec hash.\n\n");
    out.push_str("## CLI Usage\n\n");
    out.push_str("Validate RON authoring input:\n\n");
    out.push_str("```bash\n");
    out.push_str("decision-gate authoring validate --input examples/scenario.ron --format ron\n");
    out.push_str("```\n\n");
    out.push_str("Normalize to canonical JSON:\n\n");
    out.push_str("```bash\n");
    out.push_str(
        "decision-gate authoring normalize --input examples/scenario.ron --format ron \\\n",
    );
    out.push_str("  --output examples/scenario.json\n");
    out.push_str("```\n\n");
    out.push_str("## References\n\n");
    out.push_str("- `examples/scenario.ron`: authoring example in RON.\n");
    out.push_str("- `examples/scenario.json`: canonical JSON output.\n");
    out.push_str("- `schemas/scenario.schema.json`: JSON Schema for ScenarioSpec.\n");
    out
}

// ============================================================================
// SECTION: Validation Helpers
// ============================================================================

/// Parse authoring input into a JSON value for schema validation.
fn parse_value(input: &str, format: AuthoringFormat) -> Result<Value, AuthoringError> {
    match format {
        AuthoringFormat::Json => serde_json::from_str(input).map_err(|err| AuthoringError::Parse {
            format,
            error: err.to_string(),
        }),
        AuthoringFormat::Ron => ron::from_str(input).map_err(|err| AuthoringError::Parse {
            format,
            error: err.to_string(),
        }),
    }
}

/// Validate `ScenarioSpec` input against the JSON schema.
fn validate_scenario_schema(instance: &Value) -> Result<(), AuthoringError> {
    let schema = schemas::scenario_schema();
    let compiled = compile_schema(&schema)?;
    match compiled.validate(instance) {
        Ok(()) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors.map(|err| err.to_string()).collect();
            Err(AuthoringError::Schema {
                error: messages.join("; "),
            })
        }
    }
}

/// Compile the `ScenarioSpec` JSON schema for validation.
fn compile_schema(schema: &Value) -> Result<JSONSchema, AuthoringError> {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.compile(schema).map_err(|err| AuthoringError::Schema {
        error: err.to_string(),
    })
}
