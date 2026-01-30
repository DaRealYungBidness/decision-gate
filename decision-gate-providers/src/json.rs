// decision-gate-providers/src/json.rs
// ============================================================================
// Module: JSON Evidence Provider
// Description: Evidence provider for JSON and YAML file queries.
// Purpose: Resolve JSONPath expressions against local configuration files.
// Dependencies: decision-gate-core, jsonpath_lib, serde_json, serde_yaml
// ============================================================================

//! ## Overview
//! The JSON provider loads JSON or YAML files and evaluates `JSONPath` expressions
//! against their contents. It enforces path restrictions and size limits to
//! avoid resource exhaustion.
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceProviderError;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceRef;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::TrustLane;
use jsonpath_lib::select;
use serde::Deserialize;
use serde_json::Value;

// ============================================================================
// SECTION: Configuration
// ============================================================================

/// Configuration for the JSON provider.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct JsonProviderConfig {
    /// Optional root directory for resolving file paths.
    pub root: Option<PathBuf>,
    /// Maximum file size allowed, in bytes.
    pub max_bytes: usize,
    /// Allow YAML parsing when file extension is .yaml or .yml.
    pub allow_yaml: bool,
}

impl Default for JsonProviderConfig {
    fn default() -> Self {
        Self {
            root: None,
            max_bytes: 1024 * 1024,
            allow_yaml: true,
        }
    }
}

// ============================================================================
// SECTION: Provider Implementation
// ============================================================================

/// Evidence provider for JSON and YAML file queries.
pub struct JsonProvider {
    /// Provider configuration, including limits and root policy.
    config: JsonProviderConfig,
}

impl JsonProvider {
    /// Creates a new JSON provider with the given configuration.
    #[must_use]
    pub const fn new(config: JsonProviderConfig) -> Self {
        Self {
            config,
        }
    }
}

impl EvidenceProvider for JsonProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        if query.check_id.as_str() != "path" {
            return Err(EvidenceError::Provider("unsupported json check".to_string()));
        }

        let (file_path, jsonpath) = match extract_params(query.params.as_ref()) {
            Ok(params) => params,
            Err(error) => return Ok(error_result(error, None, None)),
        };
        let resolved = match resolve_path(&self.config, file_path) {
            Ok(path) => path,
            Err(error) => return Ok(error_result(error, None, None)),
        };
        let evidence_ref = EvidenceRef {
            uri: resolved.display().to_string(),
        };
        let evidence_anchor = EvidenceAnchor {
            anchor_type: "file_path".to_string(),
            anchor_value: resolved.display().to_string(),
        };
        let content = match read_file_limited(&resolved, self.config.max_bytes) {
            Ok(content) => content,
            Err(error) => {
                return Ok(error_result(error, Some(evidence_ref), Some(evidence_anchor)));
            }
        };
        let (document, content_type) =
            match parse_document(&resolved, &content, self.config.allow_yaml) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Ok(error_result(error, Some(evidence_ref), Some(evidence_anchor)));
                }
            };
        let value = match jsonpath {
            Some(path) => match select_jsonpath(&document, &path) {
                Ok(Some(value)) => Some(value),
                Ok(None) => {
                    let error = provider_error(
                        "jsonpath_not_found",
                        format!("jsonpath not found: {path}"),
                        Some(serde_json::json!({ "jsonpath": path })),
                    );
                    return Ok(error_result(error, Some(evidence_ref), Some(evidence_anchor)));
                }
                Err(error) => {
                    return Ok(error_result(error, Some(evidence_ref), Some(evidence_anchor)));
                }
            },
            None => Some(document),
        };

        Ok(EvidenceResult {
            value: value.map(EvidenceValue::Json),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: Some(evidence_ref),
            evidence_anchor: Some(evidence_anchor),
            signature: None,
            content_type: Some(content_type),
        })
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Extracts file path and optional `JSONPath` from query parameters.
fn extract_params(params: Option<&Value>) -> Result<(&str, Option<String>), EvidenceProviderError> {
    let params = params
        .ok_or_else(|| provider_error("params_missing", "json check requires params", None))?;
    let Value::Object(map) = params else {
        return Err(provider_error("params_invalid", "json params must be an object", None));
    };
    let Value::String(file) = map
        .get("file")
        .ok_or_else(|| provider_error("params_missing", "missing file param", None))?
    else {
        return Err(provider_error("params_invalid", "file param must be a string", None));
    };
    let jsonpath = match map.get("jsonpath") {
        Some(Value::String(path)) => Some(path.clone()),
        Some(_) => {
            return Err(provider_error("params_invalid", "jsonpath param must be a string", None));
        }
        None => None,
    };
    Ok((file.as_str(), jsonpath))
}

/// Resolves a file path against the configured root policy.
fn resolve_path(config: &JsonProviderConfig, file: &str) -> Result<PathBuf, EvidenceProviderError> {
    let candidate = PathBuf::from(file);
    if let Some(root) = &config.root {
        let root = root
            .canonicalize()
            .map_err(|_| provider_error("invalid_root", "invalid json root", None))?;
        let joined = if candidate.is_absolute() { candidate } else { root.join(candidate) };
        let resolved = joined.canonicalize().map_err(|_| {
            provider_error(
                "file_not_found",
                "unable to resolve json file",
                Some(serde_json::json!({ "file": file })),
            )
        })?;
        if !resolved.starts_with(&root) {
            return Err(provider_error(
                "path_outside_root",
                "json file path escapes root",
                Some(serde_json::json!({ "file": file })),
            ));
        }
        return Ok(resolved);
    }
    candidate.canonicalize().map_err(|_| {
        provider_error(
            "file_not_found",
            "unable to resolve json file",
            Some(serde_json::json!({ "file": file })),
        )
    })
}

/// Reads a file while enforcing a maximum byte limit.
fn read_file_limited(path: &Path, max_bytes: usize) -> Result<Vec<u8>, EvidenceProviderError> {
    let file = File::open(path).map_err(|err| {
        let code = if err.kind() == std::io::ErrorKind::NotFound {
            "file_not_found"
        } else {
            "file_open_failed"
        };
        provider_error(
            code,
            "unable to open json file",
            Some(serde_json::json!({ "file": path.display().to_string() })),
        )
    })?;
    let mut buf = Vec::new();
    let limit = max_bytes.saturating_add(1);
    let limit = u64::try_from(limit)
        .map_err(|_| provider_error("size_limit_invalid", "json size limit exceeds u64", None))?;
    let mut handle = file.take(limit);
    handle.read_to_end(&mut buf).map_err(|_| {
        provider_error(
            "file_read_failed",
            "unable to read json file",
            Some(serde_json::json!({ "file": path.display().to_string() })),
        )
    })?;
    if buf.len() > max_bytes {
        return Err(provider_error(
            "size_limit_exceeded",
            "json file exceeds size limit",
            Some(serde_json::json!({
                "file": path.display().to_string(),
                "max_bytes": max_bytes,
                "actual_bytes": buf.len()
            })),
        ));
    }
    Ok(buf)
}

/// Parses a JSON or YAML document and returns the content type.
fn parse_document(
    path: &Path,
    content: &[u8],
    allow_yaml: bool,
) -> Result<(Value, String), EvidenceProviderError> {
    let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or_default();
    let ext = ext.to_ascii_lowercase();
    if ext == "yaml" || ext == "yml" {
        if !allow_yaml {
            return Err(provider_error(
                "yaml_disabled",
                "yaml parsing is disabled",
                Some(serde_json::json!({ "file": path.display().to_string() })),
            ));
        }
        let value: Value = serde_yaml::from_slice(content).map_err(|_| {
            provider_error(
                "invalid_yaml",
                "invalid yaml",
                Some(serde_json::json!({ "file": path.display().to_string() })),
            )
        })?;
        return Ok((value, "application/yaml".to_string()));
    }
    let value: Value = serde_json::from_slice(content).map_err(|_| {
        provider_error(
            "invalid_json",
            "invalid json",
            Some(serde_json::json!({ "file": path.display().to_string() })),
        )
    })?;
    Ok((value, "application/json".to_string()))
}

/// Selects values using a `JSONPath` expression.
fn select_jsonpath(document: &Value, path: &str) -> Result<Option<Value>, EvidenceProviderError> {
    let matches = select(document, path).map_err(|_| {
        provider_error(
            "invalid_jsonpath",
            "invalid jsonpath",
            Some(serde_json::json!({ "jsonpath": path })),
        )
    })?;
    if matches.is_empty() {
        return Ok(None);
    }
    if matches.len() == 1 {
        return Ok(Some(matches[0].clone()));
    }
    let mut values = Vec::with_capacity(matches.len());
    for value in matches {
        values.push(value.clone());
    }
    Ok(Some(Value::Array(values)))
}

/// Builds structured provider error metadata for JSON evidence failures.
fn provider_error(
    code: &str,
    message: impl Into<String>,
    details: Option<Value>,
) -> EvidenceProviderError {
    EvidenceProviderError {
        code: code.to_string(),
        message: message.into(),
        details,
    }
}

/// Builds an error-only evidence result while preserving references.
#[allow(
    clippy::missing_const_for_fn,
    reason = "EvidenceResult owns heap data and cannot be const."
)]
fn error_result(
    error: EvidenceProviderError,
    evidence_ref: Option<EvidenceRef>,
    evidence_anchor: Option<EvidenceAnchor>,
) -> EvidenceResult {
    EvidenceResult {
        value: None,
        lane: TrustLane::Verified,
        error: Some(error),
        evidence_hash: None,
        evidence_ref,
        evidence_anchor,
        signature: None,
        content_type: None,
    }
}
