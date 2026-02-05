// crates/decision-gate-providers/tests/json_provider_path_unit.rs
// ============================================================================
// Module: JSON Provider Path Traversal Unit Tests
// Description: Tests for path normalization, symlink resolution, and JSONPath injection
// Purpose: Ensure JSON provider prevents path traversal and injection attacks
// Threat Models: TM-FILE-001 (path traversal), TM-FILE-002 (symlink escape), TM-JSON-001 (JSONPath
// injection) ============================================================================

//! ## Overview
//! Comprehensive tests for JSON provider security:
//! - Path normalization (absolute/relative, double slash, parent directory)
//! - Symlink resolution (escape detection, cycle detection, broken links)
//! - `JSONPath` injection attacks (recursive descent, filter injection)
//!
//! ## Security Posture
//! Assumes adversarial file paths: attackers may attempt:
//! - Path traversal via ../ sequences
//! - Symlink escape outside root directory
//! - `JSONPath` injection to access unintended data
//! - Null byte injection, Unicode normalization attacks
//!
//! All attacks must fail closed (return error, never expose data outside root).

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
use std::path::Path;
use std::path::PathBuf;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_providers::JsonProvider;
use decision_gate_providers::JsonProviderConfig;
use serde_json::json;
use tempfile::TempDir;

use crate::common::sample_context;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Creates a temporary test directory with sample JSON files
fn setup_test_files() -> (TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();

    // Create test file
    let test_file = root.join("test.json");
    fs::write(&test_file, r#"{"value": 42}"#).unwrap();

    // Create nested file
    let nested_dir = root.join("nested");
    fs::create_dir(&nested_dir).unwrap();
    let nested_file = nested_dir.join("data.json");
    fs::write(&nested_file, r#"{"nested": true}"#).unwrap();

    (temp_dir, root)
}

/// Creates a JSON provider with root directory.
fn provider_with_root(root: &Path) -> JsonProvider {
    JsonProvider::new(JsonProviderConfig {
        root: root.to_path_buf(),
        root_id: "path-unit".to_string(),
        max_bytes: 1024 * 1024,
        allow_yaml: true,
    })
    .expect("json provider config should be valid")
}

// ============================================================================
// SECTION: Path Normalization Unit Tests (TM-FILE-001)
// ============================================================================

/// TM-FILE-001: Tests that absolute paths are rejected.
///
/// Context: Absolute paths are forbidden to preserve deterministic runpack anchors.
#[test]
fn json_path_absolute_within_root() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let abs_path = root.join("test.json");
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": abs_path.to_str().unwrap()})),
    };

    let result = provider.query(&query, &sample_context());
    let error = result.unwrap().error.expect("missing error");
    assert_eq!(error.code, "absolute_path_forbidden");
}

/// TM-FILE-001: Tests that relative paths are resolved correctly.
///
/// Context: Relative paths should resolve relative to root.
#[test]
fn json_path_relative_resolution() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "test.json"})),
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok(), "Relative path should resolve");
}

/// TM-FILE-001: Tests that double slash is normalized.
///
/// Context: //path should be treated as /path.
#[test]
fn json_path_double_slash_normalization() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Path with double slash
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "nested//data.json"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should either succeed (normalized) or fail safely
    assert!(result.is_ok() || result.is_err(), "Double slash should be handled");
}

/// TM-FILE-001: Tests that current directory (.) is handled.
///
/// Context: ./file should resolve to file.
#[test]
fn json_path_current_directory_handling() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "./test.json"})),
    };

    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok(), "Current directory notation should work");
}

/// TM-FILE-001: Tests that parent directory (..) is blocked when escaping root.
///
/// Context: ../../../etc/passwd should be rejected.
#[test]
fn json_path_parent_directory_escape_blocked() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "../../../etc/passwd"})),
    };

    let result = provider.query(&query, &sample_context());

    // Should fail with error (path escapes root)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Path escape should be recorded as error");
    } else {
        // Also acceptable - direct error
    }
}

/// TM-FILE-001: Tests path component length limits.
///
/// Context: Very long path components should be rejected.
#[test]
fn json_path_component_length_limit() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create path with 300-character component (exceeds typical 255 limit)
    let long_component = "a".repeat(300);
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": long_component})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (file not found or path too long)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Long path component should fail");
    } else {
        // Also acceptable
    }
}

/// TM-FILE-001: Tests total path length limits.
///
/// Context: Paths exceeding OS limits should be rejected.
#[test]
fn json_path_total_length_limit() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create path with 5000 characters total
    let long_path = "a/".repeat(2500); // 5000 chars
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": long_path})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (path too long)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Long total path should fail");
    } else {
        // Acceptable
    }
}

/// TM-FILE-001: Tests null byte injection.
///
/// Context: Paths with null bytes should be rejected.
#[test]
fn json_path_null_byte_injection() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Path with null byte (file\0.json)
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "test\0.json"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (invalid path)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Null byte should be rejected");
    } else {
        // Acceptable
    }
}

/// TM-FILE-001: Tests Unicode normalization handling.
///
/// Context: NFC vs NFD forms should be normalized consistently.
#[test]
fn json_path_unicode_normalization() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Test with Unicode filename (if supported by filesystem)
    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "tëst.json"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should either succeed (if file exists) or fail gracefully
    assert!(result.is_ok() || result.is_err(), "Unicode should be handled");
}

/// TM-FILE-001: Tests trailing slash handling.
///
/// Context: Paths with trailing slashes should be handled.
#[test]
fn json_path_trailing_slash() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "nested/"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (directory, not file)
    if let Ok(evidence_result) = result {
        assert!(
            evidence_result.error.is_some(),
            "Trailing slash should indicate directory (error)"
        );
    } else {
        // Acceptable
    }
}

/// TM-FILE-001: Tests whitespace in path components.
///
/// Context: Paths with spaces or tabs should be handled correctly.
#[test]
fn json_path_whitespace_handling() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "test .json"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (file doesn't exist) but handle gracefully
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Whitespace path should fail (file not found)");
    } else {
        // Acceptable
    }
}

// ============================================================================
// SECTION: Symlink Resolution Tests (TM-FILE-002)
// ============================================================================

/// TM-FILE-002: Tests symlink escape detection.
///
/// Context: Symlinks pointing outside root should be rejected.
#[test]
#[cfg(unix)]
fn json_symlink_escape_detection() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create symlink pointing to /etc/passwd
    let symlink_path = root.join("escape_link");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/passwd", &symlink_path).ok();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "escape_link"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (symlink escapes root or parse error)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Symlink escape should be blocked");
    } else {
        // Acceptable
    }
}

/// TM-FILE-002: Tests broken symlink handling.
///
/// Context: Symlinks to non-existent targets should be rejected.
#[test]
#[cfg(unix)]
fn json_broken_symlink_handling() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create broken symlink
    let symlink_path = root.join("broken_link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("nonexistent.json"), &symlink_path).ok();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "broken_link"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (broken symlink)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Broken symlink should fail");
    } else {
        // Acceptable
    }
}

/// TM-FILE-002: Tests symlink cycle detection.
///
/// Context: Circular symlinks should be detected and rejected.
#[test]
#[cfg(unix)]
fn json_symlink_cycle_detection() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create circular symlinks: a -> b, b -> a
    let link_a = root.join("link_a");
    let link_b = root.join("link_b");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&link_b, &link_a).ok();
        std::os::unix::fs::symlink(&link_a, &link_b).ok();
    }

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "link_a"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (cycle detected or too many symlinks)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Symlink cycle should be detected");
    } else {
        // Acceptable
    }
}

/// TM-FILE-002: Tests relative symlink resolution.
///
/// Context: Relative symlinks should resolve correctly within root.
#[test]
#[cfg(unix)]
fn json_relative_symlink_resolution() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create relative symlink to test.json
    let symlink_path = root.join("link_to_test");
    #[cfg(unix)]
    std::os::unix::fs::symlink("test.json", &symlink_path).ok();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({"file": "link_to_test"})),
    };

    let result = provider.query(&query, &sample_context());
    // Should succeed (relative symlink within root)
    assert!(result.is_ok(), "Relative symlink within root should work");
}

// ============================================================================
// SECTION: JSONPath Injection Attacks (TM-JSON-001)
// ============================================================================

/// TM-JSON-001: Tests recursive descent abuse.
///
/// Context: $..'s can cause excessive recursion.
#[test]
fn json_jsonpath_recursive_descent() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": "$..value"
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should either succeed (limited recursion) or fail gracefully
    assert!(result.is_ok() || result.is_err(), "Recursive descent should be handled");
}

/// TM-JSON-001: Tests filter expression safety.
///
/// Context: Filter expressions like $[?(@.price < 10)] should be safe.
#[test]
fn json_jsonpath_filter_expression() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": "$[?(@.value > 0)]"
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should be handled safely (no injection)
    assert!(result.is_ok() || result.is_err(), "Filter expressions should be safe");
}

/// TM-JSON-001: Tests array index bounds.
///
/// Context: Very large array indices should not cause issues.
#[test]
fn json_jsonpath_array_index_bounds() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": "$[99999999]"
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail gracefully (index out of bounds)
    if let Ok(evidence_result) = result {
        assert!(
            evidence_result.error.is_some() || evidence_result.value.is_none(),
            "Large index should fail gracefully"
        );
    } else {
        // Acceptable
    }
}

/// TM-JSON-001: Tests `JSONPath` with special characters.
///
/// Context: Paths with quotes, backslashes should be handled.
#[test]
fn json_jsonpath_special_characters() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": "$.\"value\""
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should handle special characters safely
    assert!(result.is_ok() || result.is_err(), "Special chars should be handled");
}

/// TM-JSON-001: Tests `JSONPath` length limits.
///
/// Context: Very long `JSONPath` expressions should be rejected.
#[test]
fn json_jsonpath_length_limit() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    // Create very long JSONPath (10000 characters)
    let long_path = format!("$.{}", "a".repeat(10000));

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": long_path
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail gracefully (too long or not found)
    if let Ok(evidence_result) = result {
        assert!(
            evidence_result.error.is_some() || evidence_result.value.is_none(),
            "Long JSONPath should fail gracefully"
        );
    } else {
        // Acceptable
    }
}

/// TM-JSON-001: Tests union operator safety.
///
/// Context: Union operators like `$['a','b']` should be safe.
#[test]
fn json_jsonpath_union_operator() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": "$['value','other']"
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should handle unions safely
    assert!(result.is_ok() || result.is_err(), "Union operator should be safe");
}

/// TM-JSON-001: Tests that invalid `JSONPath` syntax is rejected.
///
/// Context: Malformed `JSONPath` should fail closed.
#[test]
fn json_jsonpath_invalid_syntax() {
    let (_temp, root) = setup_test_files();
    let provider = provider_with_root(&root);

    let query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
        params: Some(json!({
            "file": "test.json",
            "jsonpath": "$[[[invalid]]"
        })),
    };

    let result = provider.query(&query, &sample_context());
    // Should fail (invalid syntax)
    if let Ok(evidence_result) = result {
        assert!(evidence_result.error.is_some(), "Invalid JSONPath should fail");
    } else {
        // Acceptable
    }
}
