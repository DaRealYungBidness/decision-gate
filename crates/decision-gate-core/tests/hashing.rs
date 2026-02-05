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
use decision_gate_core::hashing::HashDigest;
use decision_gate_core::hashing::HashError;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
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

// ============================================================================
// SECTION: Size-Limit Edge Cases
// ============================================================================

#[test]
fn size_limit_exact_boundary_passes() {
    let payload = BTreeMap::from([("d", "x".to_string())]);
    let bytes = canonical_json_bytes(&payload).expect("canonical bytes");
    let exact_limit = bytes.len();

    let result = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, exact_limit);
    assert!(result.is_ok(), "Exact boundary should succeed");
}

#[test]
fn size_limit_one_byte_under_fails() {
    let payload = BTreeMap::from([("d", "x".to_string())]);
    let bytes = canonical_json_bytes(&payload).expect("canonical bytes");
    let limit = bytes.len() - 1;

    let result = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, limit);
    assert!(
        matches!(result, Err(HashError::SizeLimitExceeded { .. })),
        "One byte under limit should fail"
    );
}

#[test]
fn size_limit_one_byte_over_passes() {
    let payload = BTreeMap::from([("d", "x".to_string())]);
    let bytes = canonical_json_bytes(&payload).expect("canonical bytes");
    let limit = bytes.len() + 1;

    let result = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, limit);
    assert!(result.is_ok(), "One byte over limit should succeed");
}

#[test]
fn size_limit_zero_rejects_all() {
    let payload = BTreeMap::from([("a", 1i32)]);
    let result = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, 0);
    assert!(
        matches!(
            result,
            Err(HashError::SizeLimitExceeded {
                limit: 0,
                ..
            })
        ),
        "Zero limit should reject everything"
    );
}

#[test]
fn size_limit_reports_actual_size_correctly() {
    let payload = BTreeMap::from([("data", "x".repeat(100))]);
    let result = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, 10);

    if let Err(HashError::SizeLimitExceeded {
        limit,
        actual,
    }) = result
    {
        assert_eq!(limit, 10, "Limit should be 10");
        assert!(actual > 10, "Actual should exceed limit");
    } else {
        panic!("Expected SizeLimitExceeded error");
    }
}

#[test]
fn size_limit_usize_max_accepts_large_payloads() {
    let payload = BTreeMap::from([("data", "x".repeat(10000))]);
    let result = hash_canonical_json_with_limit(HashAlgorithm::Sha256, &payload, usize::MAX);
    assert!(result.is_ok(), "usize::MAX limit should accept large payloads");
}

// ============================================================================
// SECTION: f32 Non-Finite Float Rejection
// ============================================================================

#[derive(Serialize)]
struct F32Wrapper {
    value: f32,
}

#[test]
fn canonical_hash_rejects_f32_nan() {
    let value = F32Wrapper {
        value: f32::NAN,
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)), "f32 NaN should be rejected");
}

#[test]
fn canonical_hash_rejects_f32_infinity() {
    let value = F32Wrapper {
        value: f32::INFINITY,
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)), "f32 INFINITY should be rejected");
}

#[test]
fn canonical_hash_rejects_f32_neg_infinity() {
    let value = F32Wrapper {
        value: f32::NEG_INFINITY,
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)), "f32 NEG_INFINITY should be rejected");
}

#[test]
fn canonical_hash_accepts_f32_max() {
    let value = F32Wrapper {
        value: f32::MAX,
    };
    let result = hash_canonical_json(HashAlgorithm::Sha256, &value);
    assert!(result.is_ok(), "f32::MAX is finite and should be accepted");
}

#[test]
fn canonical_hash_accepts_f32_min() {
    let value = F32Wrapper {
        value: f32::MIN,
    };
    let result = hash_canonical_json(HashAlgorithm::Sha256, &value);
    assert!(result.is_ok(), "f32::MIN is finite and should be accepted");
}

#[test]
fn canonical_hash_accepts_f32_min_positive() {
    let value = F32Wrapper {
        value: f32::MIN_POSITIVE,
    };
    let result = hash_canonical_json(HashAlgorithm::Sha256, &value);
    assert!(result.is_ok(), "f32::MIN_POSITIVE (smallest positive) should be accepted");
}

#[derive(Serialize)]
struct NestedF32 {
    inner: F32Wrapper,
}

#[test]
fn canonical_hash_rejects_nested_f32_nan() {
    let value = NestedF32 {
        inner: F32Wrapper {
            value: f32::NAN,
        },
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)), "Nested f32 NaN should be rejected");
}

#[derive(Serialize)]
struct VecF32 {
    values: Vec<f32>,
}

#[test]
fn canonical_hash_rejects_f32_nan_in_vec() {
    let value = VecF32 {
        values: vec![1.0, f32::NAN, 3.0],
    };
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(matches!(err, HashError::Canonicalization(_)), "f32 NaN in Vec should be rejected");
}

#[test]
fn canonical_hash_rejects_f32_infinity_in_option() {
    let value: Option<F32Wrapper> = Some(F32Wrapper {
        value: f32::INFINITY,
    });
    let err = hash_canonical_json(HashAlgorithm::Sha256, &value).unwrap_err();
    assert!(
        matches!(err, HashError::Canonicalization(_)),
        "f32 INFINITY in Option should be rejected"
    );
}

// ============================================================================
// SECTION: Golden SHA-256 Tests (Known-Value Verification)
// ============================================================================

#[test]
fn golden_hash_empty_object() {
    // SHA-256 of "{}" = 44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a
    let value = json!({});
    let digest = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash");
    assert_eq!(
        digest.value, "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        "Empty object hash mismatch"
    );
    assert_eq!(digest.algorithm, HashAlgorithm::Sha256);
}

#[test]
fn golden_hash_empty_array() {
    // SHA-256 of "[]" = 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945
    let value = json!([]);
    let digest = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash");
    assert_eq!(
        digest.value, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        "Empty array hash mismatch"
    );
}

#[test]
fn golden_hash_integer_one() {
    // SHA-256 of "1" = 6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b
    let value = json!(1);
    let digest = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash");
    assert_eq!(
        digest.value, "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
        "Integer 1 hash mismatch"
    );
}

#[test]
fn golden_hash_boolean_true() {
    // SHA-256 of "true" = b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b
    let value = json!(true);
    let digest = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash");
    assert_eq!(
        digest.value, "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
        "Boolean true hash mismatch"
    );
}

#[test]
fn golden_hash_boolean_false() {
    // SHA-256 of "false" = fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa
    let value = json!(false);
    let digest = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash");
    assert_eq!(
        digest.value, "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
        "Boolean false hash mismatch"
    );
}

#[test]
fn golden_hash_null() {
    // SHA-256 of "null" via RFC 8785 canonical JSON
    let value = json!(null);
    let digest = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash");
    assert_eq!(
        digest.value, "74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b",
        "Null hash mismatch"
    );
}

#[test]
fn golden_hash_bytes_direct() {
    // SHA-256 of "test" = 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
    let digest = hash_bytes(HashAlgorithm::Sha256, b"test");
    assert_eq!(
        digest.value, "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
        "Direct bytes hash mismatch"
    );
}

#[test]
fn golden_hash_empty_bytes() {
    // SHA-256 of empty input = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    let digest = hash_bytes(HashAlgorithm::Sha256, b"");
    assert_eq!(
        digest.value, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "Empty bytes hash mismatch"
    );
}

// ============================================================================
// SECTION: Robustness Tests
// ============================================================================

#[test]
fn hash_deeply_nested_structure() {
    let mut value = json!({});
    for i in 0 .. 100 {
        value = json!({ format!("level{}", i): value });
    }
    let result = hash_canonical_json(HashAlgorithm::Sha256, &value);
    assert!(result.is_ok(), "Deeply nested structure should hash successfully");
}

#[test]
fn hash_unicode_strings() {
    let value = json!({"emoji": "Hello, 世界! 🎉"});
    let result = hash_canonical_json(HashAlgorithm::Sha256, &value);
    assert!(result.is_ok(), "Unicode strings should hash successfully");
}

#[test]
fn hash_consistency_across_calls() {
    let value = json!({"a": 1, "b": [1, 2, 3], "c": {"nested": true}});
    let hash1 = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash1");
    let hash2 = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash2");
    let hash3 = hash_canonical_json(HashAlgorithm::Sha256, &value).expect("hash3");
    assert_eq!(hash1, hash2, "Hash must be deterministic");
    assert_eq!(hash2, hash3, "Hash must be deterministic");
}

#[test]
fn hash_digest_produces_lowercase_hex() {
    let bytes = [0xAB, 0xCD, 0xEF, 0x12];
    let digest = HashDigest::new(HashAlgorithm::Sha256, &bytes);
    assert_eq!(digest.value, "abcdef12", "Hex must be lowercase");
    assert!(!digest.value.chars().any(|c| c.is_uppercase()), "No uppercase chars allowed");
}
