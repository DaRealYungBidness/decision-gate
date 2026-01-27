// decision-gate-providers/tests/json_provider.rs
// ============================================================================
// Module: JSON Provider Tests
// Description: Comprehensive tests for JSON/YAML file evidence provider.
// Purpose: Validate path traversal prevention, size limits, and parsing safety.
// Dependencies: decision-gate-providers, decision-gate-core, tempfile, serde_json
// ============================================================================

//! ## Overview
//! Tests the JSON provider for:
//! - Happy path: `JSONPath` selection, YAML parsing
//! - Adversarial: Path traversal prevention (critical security)
//! - Boundary enforcement: File size limits
//! - Error handling: Invalid JSON/YAML, missing files, invalid paths
//! - Edge cases: Empty files, deeply nested documents
//!
//! Security posture: File system is a trust boundary. Path traversal attacks
//! must be prevented to avoid unauthorized file access.
//! See: `Docs/security/threat_model.md` - TM-FILE-001

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

mod common;

use std::fs;
use std::path::PathBuf;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderId;
use decision_gate_providers::JsonProvider;
use decision_gate_providers::JsonProviderConfig;
use serde_json::Value;
use serde_json::json;
use tempfile::tempdir;

use crate::common::oversized_string;
use crate::common::path_traversal_vectors;
use crate::common::sample_context;

// ============================================================================
// SECTION: Happy Path Tests
// ============================================================================

/// Tests that JSON provider selects values via `JSONPath`.
#[test]
fn json_provider_selects_jsonpath() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"nested":{"value":"ok"}}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json", "jsonpath": "$.nested.value"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "ok");
}

/// Tests that JSON provider parses YAML files.
#[test]
fn json_provider_parses_yaml() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    fs::write(&path, "version: 2").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "config.yaml", "jsonpath": "$.version"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_i64(), Some(2));
}

/// Tests that .yml extension is also recognized as YAML.
#[test]
fn json_provider_parses_yml_extension() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.yml");
    fs::write(&path, "name: test").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "config.yml", "jsonpath": "$.name"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "test");
}

/// Tests that `JSONPath` returning multiple values returns an array.
#[test]
fn json_provider_multiple_jsonpath_matches() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"items":[{"id":1},{"id":2},{"id":3}]}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json", "jsonpath": "$.items[*].id"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Array(arr)) = result.value.unwrap() else {
        panic!("expected array evidence");
    };
    assert_eq!(arr.len(), 3);
}

/// Tests that `JSONPath` with no match returns None.
#[test]
fn json_provider_jsonpath_no_match_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"a":"b"}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json", "jsonpath": "$.nonexistent"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    assert!(result.value.is_none());
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "jsonpath_not_found");
}

/// Tests reading entire file without `JSONPath`.
#[test]
fn json_provider_reads_entire_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"key":"value"}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Object(obj)) = result.value.unwrap() else {
        panic!("expected object evidence");
    };
    assert_eq!(obj.get("key").unwrap(), "value");
}

/// Tests evidence anchor and ref are set correctly.
#[test]
fn json_provider_sets_evidence_metadata() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"x":1}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let anchor = result.evidence_anchor.unwrap();
    assert_eq!(anchor.anchor_type, "file_path");
    assert!(anchor.anchor_value.contains("data.json"));

    let evidence_ref = result.evidence_ref.unwrap();
    assert!(evidence_ref.uri.contains("data.json"));
}

// ============================================================================
// SECTION: Adversarial Tests - Path Traversal Prevention
// ============================================================================

/// Tests that basic path traversal attempts are blocked.
///
/// Threat model: TM-FILE-001 - File system traversal via untrusted input.
/// This is a CRITICAL security test.
#[test]
fn json_path_traversal_basic_blocked() {
    let dir = tempdir().unwrap();
    let safe_path = dir.path().join("safe.json");
    fs::write(&safe_path, r#"{"safe":true}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });

    // Attempt to escape the root directory
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "../../../etc/passwd"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    assert!(result.value.is_none());
    let error = result.error.expect("missing error");
    assert!(
        error.code == "path_outside_root" || error.code == "file_not_found",
        "Expected path_outside_root or file_not_found, got: {}",
        error.code
    );
}

/// Tests that all known path traversal vectors are blocked.
///
/// Threat model: TM-FILE-001 - File system traversal via untrusted input.
#[test]
fn json_path_traversal_vectors_blocked() {
    let dir = tempdir().unwrap();
    let safe_path = dir.path().join("safe.json");
    fs::write(&safe_path, r#"{"safe":true}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });

    for vector in path_traversal_vectors() {
        let query = EvidenceQuery {
            provider_id: ProviderId::new("json"),
            predicate: "path".to_string(),
            params: Some(json!({"file": vector})),
        };

        let result = provider.query(&query, &sample_context()).unwrap();
        let error = result.error.expect("missing error");
        assert!(
            error.code == "path_outside_root" || error.code == "file_not_found",
            "Expected path_outside_root or file_not_found for {vector}, got: {}",
            error.code
        );
    }
}

/// Tests that absolute paths outside root are blocked.
#[test]
fn json_absolute_path_outside_root_blocked() {
    let dir = tempdir().unwrap();
    let safe_path = dir.path().join("safe.json");
    fs::write(&safe_path, r#"{"safe":true}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });

    // Try an absolute path that's definitely outside the root
    #[cfg(unix)]
    let absolute = "/etc/passwd";
    #[cfg(windows)]
    let absolute = "C:\\Windows\\System32\\config\\sam";

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": absolute})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert!(
        error.code == "path_outside_root" || error.code == "file_not_found",
        "Expected path_outside_root or file_not_found, got: {}",
        error.code
    );
}

// ============================================================================
// SECTION: Boundary Enforcement - Size Limits
// ============================================================================

/// Tests that files exceeding `max_bytes` are rejected.
///
/// Threat model: Resource exhaustion via large file uploads.
#[test]
fn json_file_exceeds_size_limit_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("large.json");
    // Create file larger than limit
    let large_content = format!(r#"{{"data":"{}"}}"#, oversized_string(200));
    fs::write(&path, large_content).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        max_bytes: 100, // Small limit
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "large.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "size_limit_exceeded");
}

/// Tests that files at exactly the limit are accepted.
#[test]
fn json_file_at_size_limit_accepted() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("exact.json");
    // Create content that fits exactly
    let content = r#"{"a":"b"}"#; // 9 bytes
    fs::write(&path, content).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        max_bytes: 9,
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "exact.json"})),
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Error Path Tests - Invalid Content
// ============================================================================

/// Tests that invalid JSON is rejected.
#[test]
fn json_invalid_json_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("invalid.json");
    fs::write(&path, r#"{"broken": }"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "invalid.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "invalid_json");
}

/// Tests that invalid YAML is rejected.
#[test]
fn json_invalid_yaml_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("invalid.yaml");
    fs::write(&path, "key: : value").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "invalid.yaml"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "invalid_yaml");
}

/// Tests that YAML is rejected when `allow_yaml` is false.
#[test]
fn json_yaml_disabled_rejects_yaml_files() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    fs::write(&path, "valid: yaml").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        allow_yaml: false,
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "config.yaml"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "yaml_disabled");
}

/// Tests that invalid `JSONPath` expressions are rejected.
#[test]
fn json_invalid_jsonpath_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"key":"value"}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json", "jsonpath": "$[invalid"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "invalid_jsonpath");
}

// ============================================================================
// SECTION: Error Path Tests - Invalid Parameters
// ============================================================================

/// Tests that unsupported predicates are rejected.
#[test]
fn json_unsupported_predicate_rejected() {
    let provider = JsonProvider::new(JsonProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "read".to_string(),
        params: Some(json!({"file": "test.json"})),
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported"));
}

/// Tests that missing params are rejected.
#[test]
fn json_missing_params_rejected() {
    let provider = JsonProvider::new(JsonProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: None,
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "params_missing");
}

/// Tests that non-object params are rejected.
#[test]
fn json_params_not_object_rejected() {
    let provider = JsonProvider::new(JsonProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!("not_an_object")),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "params_invalid");
}

/// Tests that missing file param is rejected.
#[test]
fn json_missing_file_param_rejected() {
    let provider = JsonProvider::new(JsonProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"other": "value"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "params_missing");
}

/// Tests that non-string file param is rejected.
#[test]
fn json_file_param_not_string_rejected() {
    let provider = JsonProvider::new(JsonProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": 12345})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "params_invalid");
}

/// Tests that non-string jsonpath param is rejected.
#[test]
fn json_jsonpath_param_not_string_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"key":"value"}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json", "jsonpath": 123})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "params_invalid");
}

/// Tests that missing files return an error.
#[test]
fn json_missing_file_returns_error() {
    let dir = tempdir().unwrap();
    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "nonexistent.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "file_not_found");
}

/// Tests that invalid root directory returns an error.
#[test]
fn json_invalid_root_rejected() {
    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(PathBuf::from("/nonexistent/directory/that/does/not/exist")),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "test.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let error = result.error.expect("missing error");
    assert_eq!(error.code, "invalid_root");
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests that empty JSON objects are handled correctly.
#[test]
fn json_empty_object_handling() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.json");
    fs::write(&path, "{}").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "empty.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Object(obj)) = result.value.unwrap() else {
        panic!("expected object evidence");
    };
    assert!(obj.is_empty());
}

/// Tests that empty YAML documents are handled correctly.
#[test]
fn json_empty_yaml_handling() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.yaml");
    fs::write(&path, "").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "empty.yaml"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    // Empty YAML parses as null
    assert!(result.value.is_some());
}

/// Tests `content_type` is set correctly for JSON.
#[test]
fn json_content_type_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"x":1}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    assert_eq!(result.content_type, Some("application/json".to_string()));
}

/// Tests `content_type` is set correctly for YAML.
#[test]
fn json_content_type_yaml() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.yaml");
    fs::write(&path, "x: 1").unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data.yaml"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    assert_eq!(result.content_type, Some("application/yaml".to_string()));
}

/// Tests deeply nested JSON is handled correctly.
#[test]
fn json_deeply_nested_document() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("deep.json");
    // Create nested JSON
    let content = r#"{"a":{"b":{"c":{"d":{"e":"deep"}}}}}"#;
    fs::write(&path, content).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "deep.json", "jsonpath": "$.a.b.c.d.e"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "deep");
}

/// Tests files with special characters in name are handled.
#[test]
fn json_special_filename_handling() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data-with-dash.json");
    fs::write(&path, r#"{"ok":true}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "data-with-dash.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    assert!(result.value.is_some());
}

/// Tests subdirectory access within root is allowed.
#[test]
fn json_subdirectory_access_allowed() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let path = subdir.join("data.json");
    fs::write(&path, r#"{"in":"subdir"}"#).unwrap();

    let provider = JsonProvider::new(JsonProviderConfig {
        root: Some(dir.path().to_path_buf()),
        ..JsonProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "subdir/data.json"})),
    };

    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::Object(obj)) = result.value.unwrap() else {
        panic!("expected object evidence");
    };
    assert_eq!(obj.get("in").unwrap(), "subdir");
}
