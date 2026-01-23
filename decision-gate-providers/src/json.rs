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
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceRef;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
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
        if query.predicate.as_str() != "path" {
            return Err(EvidenceError::Provider("unsupported json predicate".to_string()));
        }

        let (file_path, jsonpath) = extract_params(query.params.as_ref())?;
        let resolved = resolve_path(&self.config, file_path)?;
        let content = read_file_limited(&resolved, self.config.max_bytes)?;
        let (document, content_type) = parse_document(&resolved, &content, self.config.allow_yaml)?;
        let value = match jsonpath {
            Some(path) => select_jsonpath(&document, &path)?,
            None => Some(document),
        };

        Ok(EvidenceResult {
            value: value.map(EvidenceValue::Json),
            evidence_hash: None,
            evidence_ref: Some(EvidenceRef {
                uri: resolved.display().to_string(),
            }),
            evidence_anchor: Some(EvidenceAnchor {
                anchor_type: "file_path".to_string(),
                anchor_value: resolved.display().to_string(),
            }),
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
fn extract_params(params: Option<&Value>) -> Result<(&str, Option<String>), EvidenceError> {
    let params = params
        .ok_or_else(|| EvidenceError::Provider("json predicate requires params".to_string()))?;
    let Value::Object(map) = params else {
        return Err(EvidenceError::Provider("json params must be an object".to_string()));
    };
    let Value::String(file) =
        map.get("file").ok_or_else(|| EvidenceError::Provider("missing file param".to_string()))?
    else {
        return Err(EvidenceError::Provider("file param must be a string".to_string()));
    };
    let jsonpath = match map.get("jsonpath") {
        Some(Value::String(path)) => Some(path.clone()),
        Some(_) => {
            return Err(EvidenceError::Provider("jsonpath param must be a string".to_string()));
        }
        None => None,
    };
    Ok((file.as_str(), jsonpath))
}

/// Resolves a file path against the configured root policy.
fn resolve_path(config: &JsonProviderConfig, file: &str) -> Result<PathBuf, EvidenceError> {
    let candidate = PathBuf::from(file);
    if let Some(root) = &config.root {
        let root = root
            .canonicalize()
            .map_err(|_| EvidenceError::Provider("invalid json root".to_string()))?;
        let joined = if candidate.is_absolute() { candidate } else { root.join(candidate) };
        let resolved = joined
            .canonicalize()
            .map_err(|_| EvidenceError::Provider("unable to resolve json file".to_string()))?;
        if !resolved.starts_with(&root) {
            return Err(EvidenceError::Provider("json file path escapes root".to_string()));
        }
        return Ok(resolved);
    }
    candidate
        .canonicalize()
        .map_err(|_| EvidenceError::Provider("unable to resolve json file".to_string()))
}

/// Reads a file while enforcing a maximum byte limit.
fn read_file_limited(path: &Path, max_bytes: usize) -> Result<Vec<u8>, EvidenceError> {
    let file = File::open(path)
        .map_err(|_| EvidenceError::Provider("unable to open json file".to_string()))?;
    let mut buf = Vec::new();
    let limit = max_bytes.saturating_add(1);
    let limit = u64::try_from(limit)
        .map_err(|_| EvidenceError::Provider("json size limit exceeds u64".to_string()))?;
    let mut handle = file.take(limit);
    handle
        .read_to_end(&mut buf)
        .map_err(|_| EvidenceError::Provider("unable to read json file".to_string()))?;
    if buf.len() > max_bytes {
        return Err(EvidenceError::Provider("json file exceeds size limit".to_string()));
    }
    Ok(buf)
}

/// Parses a JSON or YAML document and returns the content type.
fn parse_document(
    path: &Path,
    content: &[u8],
    allow_yaml: bool,
) -> Result<(Value, String), EvidenceError> {
    let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or_default();
    let ext = ext.to_ascii_lowercase();
    if ext == "yaml" || ext == "yml" {
        if !allow_yaml {
            return Err(EvidenceError::Provider("yaml parsing is disabled".to_string()));
        }
        let value: Value = serde_yaml::from_slice(content)
            .map_err(|_| EvidenceError::Provider("invalid yaml".to_string()))?;
        return Ok((value, "application/yaml".to_string()));
    }
    let value: Value = serde_json::from_slice(content)
        .map_err(|_| EvidenceError::Provider("invalid json".to_string()))?;
    Ok((value, "application/json".to_string()))
}

/// Selects values using a `JSONPath` expression.
fn select_jsonpath(document: &Value, path: &str) -> Result<Option<Value>, EvidenceError> {
    let matches = select(document, path)
        .map_err(|_| EvidenceError::Provider("invalid jsonpath".to_string()))?;
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
