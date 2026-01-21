// decision-gate-broker/tests/payload_tests.rs
// ============================================================================
// Module: Payload Unit Tests
// Description: Comprehensive tests for Payload and PayloadBody types.
// ============================================================================

//! Payload unit tests.

#![allow(clippy::unwrap_used, reason = "Tests use unwrap on deterministic fixtures.")]
#![allow(clippy::expect_used, reason = "Tests use expect for explicit failure messages.")]

use decision_gate_broker::Payload;
use decision_gate_broker::PayloadBody;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::hashing::hash_canonical_json;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::SchemaId;
use decision_gate_core::StageId;
use decision_gate_core::Timestamp;
use decision_gate_core::VisibilityPolicy;
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

#[test]
fn payload_body_bytes_empty() {
    let body = PayloadBody::Bytes(vec![]);

    if let PayloadBody::Bytes(bytes) = body {
        assert!(bytes.is_empty());
    } else {
        panic!("expected Bytes variant");
    }
}

#[test]
fn payload_body_json_null() {
    let body = PayloadBody::Json(json!(null));

    if let PayloadBody::Json(v) = body {
        assert!(v.is_null());
    } else {
        panic!("expected Json variant");
    }
}

#[test]
fn payload_body_json_array() {
    let value = json!([1, 2, 3, "four"]);
    let body = PayloadBody::Json(value.clone());

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

#[test]
fn payload_body_bytes_equality() {
    let body1 = PayloadBody::Bytes(b"same".to_vec());
    let body2 = PayloadBody::Bytes(b"same".to_vec());
    let body3 = PayloadBody::Bytes(b"different".to_vec());

    assert_eq!(body1, body2);
    assert_ne!(body1, body3);
}

#[test]
fn payload_body_json_equality() {
    let body1 = PayloadBody::Json(json!({"a": 1}));
    let body2 = PayloadBody::Json(json!({"a": 1}));
    let body3 = PayloadBody::Json(json!({"a": 2}));

    assert_eq!(body1, body2);
    assert_ne!(body1, body3);
}

#[test]
fn payload_body_different_variants_not_equal() {
    let bytes_body = PayloadBody::Bytes(b"{}".to_vec());
    let json_body = PayloadBody::Json(json!({}));

    assert_ne!(bytes_body, json_body);
}

// ============================================================================
// SECTION: Payload Construction Tests
// ============================================================================

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

#[test]
fn payload_construction_with_json() {
    let value = json!({"test": true});
    let envelope = sample_json_envelope(&value);
    let body = PayloadBody::Json(value.clone());

    let payload = Payload {
        envelope: envelope.clone(),
        body: body.clone(),
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

#[test]
fn payload_len_returns_zero_for_empty_bytes() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b""),
        body: PayloadBody::Bytes(vec![]),
    };

    assert_eq!(payload.len(), 0);
}

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

#[test]
fn payload_len_handles_large_bytes() {
    let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();

    let payload = Payload {
        envelope: sample_bytes_envelope(&data),
        body: PayloadBody::Bytes(data.clone()),
    };

    assert_eq!(payload.len(), 10000);
}

// ============================================================================
// SECTION: Payload::is_empty() Tests
// ============================================================================

#[test]
fn payload_is_empty_true_for_empty_bytes() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b""),
        body: PayloadBody::Bytes(vec![]),
    };

    assert!(payload.is_empty());
}

#[test]
fn payload_is_empty_false_for_non_empty_bytes() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b"x"),
        body: PayloadBody::Bytes(b"x".to_vec()),
    };

    assert!(!payload.is_empty());
}

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

        let payload = Payload { envelope, body };

        assert_eq!(
            payload.is_empty(),
            expected_empty,
            "is_empty mismatch for payload"
        );
        assert_eq!(
            payload.len() == 0,
            expected_empty,
            "len==0 should match is_empty"
        );
    }
}

// ============================================================================
// SECTION: Payload Clone Tests
// ============================================================================

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

#[test]
fn payload_debug_format_works() {
    let payload = Payload {
        envelope: sample_bytes_envelope(b"debug"),
        body: PayloadBody::Bytes(b"debug".to_vec()),
    };

    let debug_str = format!("{:?}", payload);
    assert!(debug_str.contains("Payload"));
    assert!(debug_str.contains("envelope"));
    assert!(debug_str.contains("body"));
}

#[test]
fn payload_body_debug_format_works() {
    let bytes_body = PayloadBody::Bytes(b"test".to_vec());
    let json_body = PayloadBody::Json(json!({"test": true}));

    let bytes_debug = format!("{:?}", bytes_body);
    let json_debug = format!("{:?}", json_body);

    assert!(bytes_debug.contains("Bytes"));
    assert!(json_debug.contains("Json"));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

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

#[test]
fn payload_with_binary_data() {
    // Test with all possible byte values
    let data: Vec<u8> = (0..=255).collect();

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

    assert!(payload.len() > 0);
}
