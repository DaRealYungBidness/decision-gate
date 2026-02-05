//! JSON provider property-based tests.
//!
//! ## Purpose
//! These tests fuzz file paths and `JSONPath` expressions to ensure the provider
//! fails closed and never panics on adversarial inputs.
//!
//! ## Threat model
//! - TM-FILE-001 (path traversal): hostile paths must be rejected or sanitized.
//! - TM-JSON-001 (`JSONPath` injection): invalid selectors must fail closed.
//!
//! ## What is covered
//! - Random file paths are handled without panic.
//! - Random `JSONPath` expressions are handled without panic.
//!
//! ## What is intentionally out of scope
//! - Specific path traversal vectors (covered by unit tests).
//! - YAML parsing edge cases (covered by `json_provider.rs` tests).
// crates/decision-gate-providers/tests/proptest_json.rs
// ============================================================================
// Module: JSON Provider Property-Based Tests
// Description: Fuzz-like checks for path and JSONPath handling.
// Purpose: Ensure provider fails closed without panics on adversarial inputs.
// ============================================================================

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
    reason = "Test-only assertions and helpers are permitted."
)]

use std::fs;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_providers::JsonProvider;
use decision_gate_providers::JsonProviderConfig;
use proptest::prelude::*;
use serde_json::json;
use tempfile::tempdir;

mod common;
use crate::common::sample_context;

fn provider_with_root() -> (tempfile::TempDir, JsonProvider) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.json");
    fs::write(&path, r#"{"value": 42, "nested": {"ok": true}}"#).unwrap();
    let provider = JsonProvider::new(JsonProviderConfig {
        root: dir.path().to_path_buf(),
        root_id: "proptest-root".to_string(),
        max_bytes: 1024 * 1024,
        allow_yaml: true,
    })
    .expect("json provider config should be valid");
    (dir, provider)
}

proptest! {
    #[test]
    fn json_provider_handles_random_paths(path in ".{1,64}") {
        let (_dir, provider) = provider_with_root();
        let query = EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({"file": path})),
        };
        let result = provider.query(&query, &sample_context());
        prop_assert!(result.is_ok());
    }

    #[test]
    fn json_provider_handles_random_jsonpaths(path in ".{1,64}") {
        let (_dir, provider) = provider_with_root();
        let query = EvidenceQuery {
            provider_id: ProviderId::new("json"),
            check_id: "path".to_string(),
            params: Some(json!({"file": "data.json", "jsonpath": path})),
        };
        let result = provider.query(&query, &sample_context());
        prop_assert!(result.is_ok());
    }
}
