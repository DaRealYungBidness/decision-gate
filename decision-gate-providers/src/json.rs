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
use decision_gate_core::hashing::canonical_json_bytes;
use jsonpath_lib::select;
use serde::Deserialize;
use serde_json::Value;

// ============================================================================
// SECTION: Configuration
// ============================================================================

/// Configuration for the JSON provider.
///
/// # Invariants
/// - `root` is required and bounds all file access.
/// - `root_id` is a stable identifier used in evidence anchors.
/// - `max_bytes` is enforced as a hard upper bound on file size.
/// - `allow_yaml` gates YAML parsing for `.yaml`/`.yml` files.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct JsonProviderConfig {
    /// Root directory for resolving file paths.
    pub root: PathBuf,
    /// Stable identifier for the configured root.
    pub root_id: String,
    /// Maximum file size allowed, in bytes.
    pub max_bytes: usize,
    /// Allow YAML parsing when file extension is .yaml or .yml.
    pub allow_yaml: bool,
}

impl Default for JsonProviderConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            root_id: String::new(),
            max_bytes: 1024 * 1024,
            allow_yaml: true,
        }
    }
}

// ============================================================================
// SECTION: Provider Implementation
// ============================================================================

/// Evidence provider for JSON and YAML file queries.
///
/// # Invariants
/// - Supports only the `path` check id.
/// - File reads are bounded and validated before parsing.
/// - `JSONPath` evaluation is deterministic for the same inputs.
pub struct JsonProvider {
    /// Provider configuration, including limits and root policy.
    config: JsonProviderConfig,
}

impl JsonProvider {
    /// Creates a new JSON provider with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when the configuration is invalid.
    pub fn new(mut config: JsonProviderConfig) -> Result<Self, EvidenceError> {
        let canonical_root = validate_config(&config)?;
        config.root = canonical_root;
        Ok(Self {
            config,
        })
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
            uri: format!("dg+file://{}/{}", self.config.root_id, resolved.relative),
        };
        let evidence_anchor = EvidenceAnchor {
            anchor_type: "file_path_rooted".to_string(),
            anchor_value: match canonical_anchor_value(&self.config.root_id, &resolved.relative) {
                Ok(value) => value,
                Err(error) => return Ok(error_result(error, Some(evidence_ref), None)),
            },
        };
        let content = match read_file_limited(
            &resolved.absolute,
            &resolved.relative,
            self.config.max_bytes,
        ) {
            Ok(content) => content,
            Err(error) => {
                return Ok(error_result(error, Some(evidence_ref), Some(evidence_anchor)));
            }
        };
        let (document, content_type) = match parse_document(
            &resolved.absolute,
            &resolved.relative,
            &content,
            self.config.allow_yaml,
        ) {
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

/// Canonical file resolution result.
#[derive(Debug, Clone)]
struct ResolvedPath {
    /// Canonical absolute path on disk.
    absolute: PathBuf,
    /// POSIX-style path relative to the configured root.
    relative: String,
}

/// Resolves a file path against the configured root policy.
fn resolve_path(
    config: &JsonProviderConfig,
    file: &str,
) -> Result<ResolvedPath, EvidenceProviderError> {
    if file.is_empty() {
        return Err(provider_error("path_missing", "json file path is empty", None));
    }
    let candidate = PathBuf::from(file);
    if candidate.is_absolute() {
        return Err(provider_error(
            "absolute_path_forbidden",
            "json file path must be relative to the configured root",
            Some(serde_json::json!({ "file": file })),
        ));
    }
    let root = &config.root;
    let joined = root.join(&candidate);
    let resolved = joined.canonicalize().map_err(|_| {
        provider_error(
            "file_not_found",
            "unable to resolve json file",
            Some(serde_json::json!({ "file": file })),
        )
    })?;
    if !resolved.starts_with(root) {
        return Err(provider_error(
            "path_outside_root",
            "json file path escapes root",
            Some(serde_json::json!({ "file": file })),
        ));
    }
    let relative = resolved.strip_prefix(root).map_err(|_| {
        provider_error(
            "path_invalid",
            "unable to normalize json file path",
            Some(serde_json::json!({ "file": file })),
        )
    })?;
    let relative = to_posix_relative(relative)?;
    Ok(ResolvedPath {
        absolute: resolved,
        relative,
    })
}

/// Reads a file while enforcing a maximum byte limit.
fn read_file_limited(
    path: &Path,
    relative: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, EvidenceProviderError> {
    let file = File::open(path).map_err(|err| {
        let code = if err.kind() == std::io::ErrorKind::NotFound {
            "file_not_found"
        } else {
            "file_open_failed"
        };
        provider_error(
            code,
            "unable to open json file",
            Some(serde_json::json!({ "file": relative })),
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
            Some(serde_json::json!({ "file": relative })),
        )
    })?;
    if buf.len() > max_bytes {
        return Err(provider_error(
            "size_limit_exceeded",
            "json file exceeds size limit",
            Some(serde_json::json!({
                "file": relative,
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
    relative: &str,
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
                Some(serde_json::json!({ "file": relative })),
            ));
        }
        let value: Value = serde_yaml::from_slice(content).map_err(|_| {
            provider_error(
                "invalid_yaml",
                "invalid yaml",
                Some(serde_json::json!({ "file": relative })),
            )
        })?;
        return Ok((value, "application/yaml".to_string()));
    }
    let value: Value = serde_json::from_slice(content).map_err(|_| {
        provider_error(
            "invalid_json",
            "invalid json",
            Some(serde_json::json!({ "file": relative })),
        )
    })?;
    Ok((value, "application/json".to_string()))
}

/// Validates JSON provider configuration and returns a canonical root path.
fn validate_config(config: &JsonProviderConfig) -> Result<PathBuf, EvidenceError> {
    if config.root.as_os_str().is_empty() {
        return Err(EvidenceError::Provider("json provider requires config.root".to_string()));
    }
    if config.root_id.is_empty() {
        return Err(EvidenceError::Provider("json provider requires config.root_id".to_string()));
    }
    if config.root_id.len() > 64 {
        return Err(EvidenceError::Provider(
            "json provider root_id exceeds 64 characters".to_string(),
        ));
    }
    if !config
        .root_id
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(EvidenceError::Provider(
            "json provider root_id must be lowercase ascii, digits, '-' or '_'".to_string(),
        ));
    }
    let root = config
        .root
        .canonicalize()
        .map_err(|_| EvidenceError::Provider("json provider root does not exist".to_string()))?;
    if !root.is_dir() {
        return Err(EvidenceError::Provider("json provider root is not a directory".to_string()));
    }
    Ok(root)
}

/// Builds a canonical JSON anchor value for a rooted file path.
fn canonical_anchor_value(root_id: &str, path: &str) -> Result<String, EvidenceProviderError> {
    let value = serde_json::json!({
        "root_id": root_id,
        "path": path,
    });
    let bytes = canonical_json_bytes(&value)
        .map_err(|_| provider_error("anchor_invalid", "anchor value serialization failed", None))?;
    String::from_utf8(bytes)
        .map_err(|_| provider_error("anchor_invalid", "anchor value is not valid utf-8", None))
}

/// Converts a relative path to POSIX separators, rejecting invalid components.
fn to_posix_relative(path: &Path) -> Result<String, EvidenceProviderError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(value) => {
                parts.push(value.to_string_lossy().to_string());
            }
            std::path::Component::CurDir => {}
            _ => {
                return Err(provider_error(
                    "path_invalid",
                    "json file path contains invalid components",
                    None,
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(provider_error("path_invalid", "json file path resolves to empty path", None));
    }
    Ok(parts.join("/"))
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
