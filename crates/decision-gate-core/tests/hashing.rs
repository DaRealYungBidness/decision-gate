// crates/decision-gate-core/tests/hashing.rs
// ============================================================================
// Module: Canonical Hashing Tests
// Description: Verifies RFC 8785 canonical JSON hashing behavior.
// ============================================================================
//! ## Overview
//! Ensures canonical JSON hashing is deterministic across key ordering, numeric
//! normalization, and size limits, and rejects non-finite floats.

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

use std::collections::BTreeMap;

use decision_gate_core::HashAlgorithm;
use decision_gate_core::hashing::HashError;
use decision_gate_core::hashing::hash_canonical_json;
use decision_gate_core::hashing::hash_canonical_json_with_limit;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

#[test]
fn canonical_hash_is_order_independent_for_maps() {
    let mut map_a = Map::new();
    map_a.insert("b".to_string(), json!(2));
    map_a.insert("a".to_string(), json!(1));

    let mut map_b = Map::new();
    map_b.insert("a".to_string(), json!(1));
    map_b.insert("b".to_string(), json!(2));

    let value_a = Value::Object(map_a);
    let value_b = Value::Object(map_b);

    let hash_a = hash_canonical_json(HashAlgorithm::Sha256, &value_a).expect("hash a");
    let hash_b = hash_canonical_json(HashAlgorithm::Sha256, &value_b).expect("hash b");

    assert_eq!(hash_a, hash_b);
}

#[test]
fn canonical_hash_normalizes_numeric_representation() {
    let value_a = json!(1.0);
    let value_b = json!(1);

    let hash_a = hash_canonical_json(HashAlgorithm::Sha256, &value_a).expect("hash a");
    let hash_b = hash_canonical_json(HashAlgorithm::Sha256, &value_b).expect("hash b");

    assert_eq!(hash_a, hash_b);
}

#[derive(Serialize)]
struct FloatWrapper {
    value: f64,
}

#[test]
fn canonical_hash_rejects_nan() {
    let value = FloatWrapper {
        value: f64::NAN,
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)));
}

#[test]
fn canonical_hash_rejects_infinity() {
    let value = FloatWrapper {
        value: f64::INFINITY,
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)));
}

#[test]
fn canonical_hash_rejects_negative_infinity() {
    let value = FloatWrapper {
        value: f64::NEG_INFINITY,
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)));
}

#[test]
fn canonical_hash_respects_size_limit() {
    let payload = BTreeMap::from([("data", "x".repeat(64))]);
    let err = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, 16).unwrap_err();
    assert!(matches!(err, HashError::SizeLimitExceeded { .. }));
}
