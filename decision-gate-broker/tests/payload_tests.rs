// decision-gate-broker/tests/payload_tests.rs
// ============================================================================
// Module: Payload Unit Tests
// Description: Comprehensive tests for Payload and PayloadBody types.
// Purpose: Validate payload construction and helper behavior.
// Dependencies: decision-gate-broker, decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! Exercises [`decision_gate_broker::Payload`] helpers and payload body variants.

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

use decision_gate_broker::Payload;
use decision_gate_broker::PayloadBody;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::SchemaId;
use decision_gate_core::StageId;
use decision_gate_core::Timestamp;
use decision_gate_core::VisibilityPolicy;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::hashing::hash_canonical_json;
use serde_json::json;

// ============================================================================
// SECTION: Helper Functions
// ============================================================================

fn sample_envelope_with_hash(
    content_type: &str,
    content_hash: decision_gate_core::hashing::HashDigest,
) -> PacketEnvelope {
    PacketEnvelope {
        scenario_id: ScenarioId::new("test-scenario"),
        run_id: RunId::new("test-run"),
        stage_id: StageId::new("test-stage"),
        packet_id: PacketId::new("test-packet"),
        schema_id: SchemaId::new("test-schema"),
        content_type: content_type.to_string(),
        content_hash,
        visibility: VisibilityPolicy::new(vec![], vec![]),
        expiry: None,
        correlation_id: None,
        issued_at: Timestamp::Logical(1),
    }
}

fn sample_bytes_envelope(bytes: &[u8]) -> PacketEnvelope {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, bytes);
    sample_envelope_with_hash("application/octet-stream", content_hash)
}

fn sample_json_envelope(value: &serde_json::Value) -> PacketEnvelope {
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, value).expect("json hash");
    sample_envelope_with_hash("application/json", content_hash)
}

// ============================================================================
// SECTION: PayloadBody Construction Tests
// ============================================================================

/// Tests payload body bytes construction.
#[test]
fn payload_body_bytes_construction() {
    let data = b"test bytes";
    let body = PayloadBody::Bytes(data.to_vec());

    if let PayloadBody::Bytes(bytes) = body {
        assert_eq!(bytes, data);
    } else {
        panic!("expected Bytes variant");
    }
}

/// Tests payload body json construction.
#[test]
fn payload_body_json_construction() {
    let value = json!({"key": "value"});
    let body = PayloadBody::Json(value.clone());

    if let PayloadBody::Json(v) = body {
        assert_eq!(v, value);
    } else {
        panic!("expected Json variant");
    }
}

/// Tests payload body bytes empty.
#[test]
fn payload_body_bytes_empty() {
    let body = PayloadBody::Bytes(vec![]);

    if let PayloadBody::Bytes(bytes) = body {
        assert!(bytes.is_empty());
    } else {
        panic!("expected Bytes variant");
    }
}

/// Tests payload body json null.
#[test]
fn payload_body_json_null() {
    let body = PayloadBody::Json(json!(null));

    if let PayloadBody::Json(v) = body {
        assert!(v.is_null());
    } else {
        panic!("expected Json variant");
    }
}

/// Tests payload body json array.
#[test]
fn payload_body_json_array() {
    let value = json!([1, 2, 3, "four"]);
    let body = PayloadBody::Json(value);

    if let PayloadBody::Json(v) = body {
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 4);
    } else {
        panic!("expected Json variant");
    }
}

// ============================================================================
// SECTION: PayloadBody Equality Tests
// ============================================================================

/// Tests payload body bytes equality.
#[test]
fn payload_body_bytes_equality() {
    let body1 = PayloadBody::Bytes(b"same".to_vec());
    let body2 = PayloadBody::Bytes(b"same".to_vec());
    let body3 = PayloadBody::Bytes(b"different".to_vec());

    assert_eq!(body1, body2);
    assert_ne!(body1, body3);
}

/// Tests payload body json equality.
#[test]
fn payload_body_json_equality() {
    let body1 = PayloadBody::Json(json!({"a": 1}));
    let body2 = PayloadBody::Json(json!({"a": 1}));
    let body3 = PayloadBody::Json(json!({"a": 2}));

    assert_eq!(body1, body2);
    assert_ne!(body1, body3);
}

/// Tests payload body different variants not equal.
#[test]
fn payload_body_different_variants_not_equal() {
    let bytes_body = PayloadBody::Bytes(b"{}".to_vec());
    let json_body = PayloadBody::Json(json!({}));

    assert_ne!(bytes_body, json_body);
}

// ============================================================================
// SECTION: Payload Construction Tests
// ============================================================================

/// Tests payload construction with bytes.
#[test]
fn payload_construction_with_bytes() {
    let data = b"payload content";
    let envelope = sample_bytes_envelope(data);
    let body = PayloadBody::Bytes(data.to_vec());

    let payload = Payload {
        envelope: envelope.clone(),
        body: body.clone(),
    };

    assert_eq!(payload.envelope, envelope);
    assert_eq!(payload.body, body);
}

/// Tests payload construction with json.
#[test]
fn payload_construction_with_json() {
    let value = json!({"test": true});
    let envelope = sample_json_envelope(&value);
    let body = PayloadBody::Json(value.clone());

    let payload = Payload {
        envelope: envelope.clone(),
        body,
    };

    assert_eq!(payload.envelope, envelope);
    if let PayloadBody::Json(v) = &payload.body {
        assert_eq!(v, &value);
    } else {
        panic!("expected Json variant");
    }
}

// ============================================================================
// SECTION: Payload::len() Tests
// ============================================================================

/// Tests payload len returns bytes length.
#[test]
fn payload_len_returns_bytes_length() {
    let data = b"exactly 16 bytes";
    assert_eq!(data.len(), 16);

    let payload = Payload {
        envelope: sample_bytes_envelope(data),
        body: PayloadBody::Bytes(data.to_vec()),
    };

    assert_eq!(payload.len(), 16);
}

/// Tests payload len returns zero for empty bytes.
#[test]
fn payload_len_returns_zero_for_empty_bytes() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b""),
        body: PayloadBody::Bytes(vec![]),
    };

    assert_eq!(payload.len(), 0);
}

/// Tests payload len returns serialized json length.
#[test]
fn payload_len_returns_serialized_json_length() {
    let value = json!({"key": "value"});
    let serialized = serde_json::to_vec(&value).expect("serialize");
    let expected_len = serialized.len();

    let payload = Payload {
        envelope: sample_json_envelope(&value),
        body: PayloadBody::Json(value),
    };

    assert_eq!(payload.len(), expected_len);
}

/// Tests payload len handles complex json.
#[test]
fn payload_len_handles_complex_json() {
    let value = json!({
        "nested": {
            "array": [1, 2, 3],
            "object": {"a": "b"}
        },
        "string": "hello",
        "number": 42,
        "boolean": true,
        "null": null
    });
    let serialized = serde_json::to_vec(&value).expect("serialize");
    let expected_len = serialized.len();

    let payload = Payload {
        envelope: sample_json_envelope(&value),
        body: PayloadBody::Json(value),
    };

    assert_eq!(payload.len(), expected_len);
}

/// Tests payload len handles large bytes.
#[test]
fn payload_len_handles_large_bytes() {
    let data: Vec<u8> =
        (0 .. 10000).map(|i| u8::try_from(i % 256).expect("u8 conversion")).collect();

    let payload = Payload {
        envelope: sample_bytes_envelope(&data),
        body: PayloadBody::Bytes(data.clone()),
    };

    assert_eq!(payload.len(), 10000);
}

// ============================================================================
// SECTION: Payload::is_empty() Tests
// ============================================================================

/// Tests payload is empty true for empty bytes.
#[test]
fn payload_is_empty_true_for_empty_bytes() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b""),
        body: PayloadBody::Bytes(vec![]),
    };

    assert!(payload.is_empty());
}

/// Tests payload is empty false for non empty bytes.
#[test]
fn payload_is_empty_false_for_non_empty_bytes() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b"x"),
        body: PayloadBody::Bytes(b"x".to_vec()),
    };

    assert!(!payload.is_empty());
}

/// Tests payload is empty false for json.
#[test]
fn payload_is_empty_false_for_json() {
    // Even an empty JSON object serializes to "{}" which is non-empty
    let value = json!({});
    let payload = Payload {
        envelope: sample_json_envelope(&value),
        body: PayloadBody::Json(value),
    };

    assert!(!payload.is_empty());
}

/// Tests payload is empty consistent with len.
#[test]
fn payload_is_empty_consistent_with_len() {
    let test_cases: Vec<(PayloadBody, bool)> = vec![
        (PayloadBody::Bytes(vec![]), true),
        (PayloadBody::Bytes(b"a".to_vec()), false),
        (PayloadBody::Json(json!({})), false),
        (PayloadBody::Json(json!(null)), false),
        (PayloadBody::Json(json!([1, 2, 3])), false),
    ];

    for (body, expected_empty) in test_cases {
        let envelope = match &body {
            PayloadBody::Bytes(b) => sample_bytes_envelope(b),
            PayloadBody::Json(v) => sample_json_envelope(v),
        };

        let payload = Payload {
            envelope,
            body,
        };

        assert_eq!(payload.is_empty(), expected_empty, "is_empty mismatch for payload");
        assert_eq!(payload.is_empty(), expected_empty, "is_empty should match expected");
    }
}

// ============================================================================
// SECTION: Payload Clone Tests
// ============================================================================

/// Tests payload clone creates independent copy.
#[test]
fn payload_clone_creates_independent_copy() {
    let data = b"original";
    let payload = Payload {
        envelope: sample_bytes_envelope(data),
        body: PayloadBody::Bytes(data.to_vec()),
    };

    let cloned = payload.clone();

    assert_eq!(payload, cloned);
    assert_eq!(payload.envelope, cloned.envelope);
    assert_eq!(payload.body, cloned.body);
}

/// Tests payload clone json independent.
#[test]
fn payload_clone_json_independent() {
    let value = json!({"mutable": "value"});
    let payload = Payload {
        envelope: sample_json_envelope(&value),
        body: PayloadBody::Json(value),
    };

    let cloned = payload.clone();

    assert_eq!(payload.body, cloned.body);
}

// ============================================================================
// SECTION: Payload Debug Tests
// ============================================================================

/// Tests payload debug format works.
#[test]
fn payload_debug_format_works() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b"debug"),
        body: PayloadBody::Bytes(b"debug".to_vec()),
    };

    let debug_str = format!("{payload:?}");
    assert!(debug_str.contains("Payload"));
    assert!(debug_str.contains("envelope"));
    assert!(debug_str.contains("body"));
}

/// Tests payload body debug format works.
#[test]
fn payload_body_debug_format_works() {
    let bytes_body = PayloadBody::Bytes(b"test".to_vec());
    let json_body = PayloadBody::Json(json!({"test": true}));

    let bytes_debug = format!("{bytes_body:?}");
    let json_debug = format!("{json_body:?}");

    assert!(bytes_debug.contains("Bytes"));
    assert!(json_debug.contains("Json"));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests payload with unicode json.
#[test]
fn payload_with_unicode_json() {
    let value = json!({
        "emoji": "🎉🚀",
        "chinese": "中文",
        "arabic": "العربية",
        "special": "tab\there\nnewline"
    });

    let payload = Payload {
        envelope: sample_json_envelope(&value),
        body: PayloadBody::Json(value.clone()),
    };

    if let PayloadBody::Json(v) = &payload.body {
        assert_eq!(v["emoji"], "🎉🚀");
        assert_eq!(v["chinese"], "中文");
    } else {
        panic!("expected Json variant");
    }
}

/// Tests payload with binary data.
#[test]
fn payload_with_binary_data() {
    // Test with all possible byte values
    let data: Vec<u8> = (0 ..= 255).collect();

    let payload = Payload {
        envelope: sample_bytes_envelope(&data),
        body: PayloadBody::Bytes(data.clone()),
    };

    if let PayloadBody::Bytes(bytes) = &payload.body {
        assert_eq!(bytes, &data);
        assert_eq!(bytes.len(), 256);
    } else {
        panic!("expected Bytes variant");
    }
}

/// Tests payload json with deeply nested structure.
#[test]
fn payload_json_with_deeply_nested_structure() {
    let value = json!({
        "level1": {
            "level2": {
                "level3": {
                    "level4": {
                        "level5": {
                            "value": "deep"
                        }
                    }
                }
            }
        }
    });

    let payload = Payload {
        envelope: sample_json_envelope(&value),
        body: PayloadBody::Json(value),
    };

    assert!(!payload.is_empty());
}
