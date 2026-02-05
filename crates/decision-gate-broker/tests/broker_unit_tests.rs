// crates/decision-gate-broker/tests/broker_unit_tests.rs
// ============================================================================
// Module: CompositeBroker Unit Tests
// Description: Comprehensive unit tests for the broker dispatcher.
// Purpose: Validate CompositeBroker payload resolution and dispatch behavior.
// Dependencies: decision-gate-broker, decision-gate-core, base64, tempfile, url
// ============================================================================

//! ## Overview
//! Exercises [`decision_gate_broker::CompositeBroker`] behavior for payload
//! resolution, hashing, and dispatch.

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

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use decision_gate_broker::BrokerError;
use decision_gate_broker::CallbackSink;
use decision_gate_broker::CompositeBroker;
use decision_gate_broker::FileSource;
use decision_gate_broker::InlineSource;
use decision_gate_broker::PayloadBody;
use decision_gate_broker::SinkError;
use decision_gate_core::ContentRef;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
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
use tempfile::tempdir;
use url::Url;

// ============================================================================
// SECTION: Helper Functions
// ============================================================================

fn sample_envelope(
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

fn sample_target() -> DispatchTarget {
    DispatchTarget::Agent {
        agent_id: "test-agent".to_string(),
    }
}

fn success_sink() -> CallbackSink {
    CallbackSink::new(|target, payload| {
        Ok(DispatchReceipt {
            dispatch_id: "test-receipt".to_string(),
            target: target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    })
}

// ============================================================================
// SECTION: Builder Tests
// ============================================================================

/// Tests broker builder builds with sink.
#[test]
fn broker_builder_builds_with_sink() {
    let broker = CompositeBroker::builder().sink(success_sink()).build();

    assert!(broker.is_ok());
}

/// Tests broker builder fails without sink.
#[test]
fn broker_builder_fails_without_sink() {
    let result = CompositeBroker::builder().build();

    assert!(result.is_err());
    match result {
        Err(BrokerError::MissingSink) => {} // Expected
        Err(other) => panic!("expected MissingSink, got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// Tests broker builder registers source.
#[test]
fn broker_builder_registers_source() {
    let dir = tempdir().expect("temp dir");
    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(success_sink())
        .build();

    assert!(broker.is_ok());
}

/// Tests broker builder registers multiple sources.
#[test]
fn broker_builder_registers_multiple_sources() {
    let dir = tempdir().expect("temp dir");
    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .source("inline", InlineSource::new())
        .sink(success_sink())
        .build();

    assert!(broker.is_ok());
}

/// Tests broker builder allows source override.
#[test]
fn broker_builder_allows_source_override() {
    let dir1 = tempdir().expect("temp dir 1");
    let dir2 = tempdir().expect("temp dir 2");

    // Register same scheme twice - second should override
    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir1.path()))
        .source("file", FileSource::new(dir2.path()))
        .sink(success_sink())
        .build();

    assert!(broker.is_ok());
}

// ============================================================================
// SECTION: Dispatch - Inline JSON Payload Tests
// ============================================================================

/// Tests broker dispatches inline json payload.
#[test]
fn broker_dispatches_inline_json_payload() {
    let json_value = json!({"key": "value"});
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value).expect("hash");
    let envelope = sample_envelope("application/json", content_hash);
    let expected_json = json_value.clone();
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let sink = CallbackSink::new(move |_, payload| {
        if let PayloadBody::Json(value) = &payload.body {
            assert_eq!(value, &expected_json);
        } else {
            panic!("expected JSON payload");
        }
        Ok(DispatchReceipt {
            dispatch_id: "json-dispatch".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder().sink(sink).build().expect("build broker");

    let receipt = broker.dispatch(&sample_target(), &envelope, &payload).expect("dispatch");

    assert_eq!(receipt.dispatch_id, "json-dispatch");
}

/// Tests broker dispatches inline json array.
#[test]
fn broker_dispatches_inline_json_array() {
    let json_value = json!([1, 2, 3, "four"]);
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value).expect("hash");
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Dispatch - Inline Bytes Payload Tests
// ============================================================================

/// Tests broker dispatches inline bytes payload.
#[test]
fn broker_dispatches_inline_bytes_payload() {
    let bytes = b"binary data";
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, bytes);
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::Bytes {
        bytes: bytes.to_vec(),
    };

    let expected_bytes = bytes.to_vec();
    let sink = CallbackSink::new(move |_, payload| {
        if let PayloadBody::Bytes(b) = &payload.body {
            assert_eq!(b, &expected_bytes);
        } else {
            panic!("expected Bytes payload");
        }
        Ok(DispatchReceipt {
            dispatch_id: "bytes-dispatch".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder().sink(sink).build().expect("build broker");

    let receipt = broker.dispatch(&sample_target(), &envelope, &payload).expect("dispatch");

    assert_eq!(receipt.dispatch_id, "bytes-dispatch");
}

/// Tests broker dispatches empty bytes payload.
#[test]
fn broker_dispatches_empty_bytes_payload() {
    let bytes = b"";
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, bytes);
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::Bytes {
        bytes: vec![],
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Dispatch - External Payload Tests
// ============================================================================

/// Tests broker dispatches external file payload.
#[test]
fn broker_dispatches_external_file_payload() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("payload.bin");
    let content = b"file content";
    std::fs::write(&path, content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, content);
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let expected_content = content.to_vec();
    let sink = CallbackSink::new(move |_, payload| {
        if let PayloadBody::Bytes(b) = &payload.body {
            assert_eq!(b, &expected_content);
        } else {
            panic!("expected Bytes payload");
        }
        Ok(DispatchReceipt {
            dispatch_id: "file-dispatch".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("build broker");

    let receipt = broker.dispatch(&sample_target(), &envelope, &payload).expect("dispatch");

    assert_eq!(receipt.dispatch_id, "file-dispatch");
}

/// Tests broker dispatches external inline payload.
#[test]
fn broker_dispatches_external_inline_payload() {
    let data = b"inline external";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);
    let content_ref = ContentRef {
        uri: format!("inline+bytes:{encoded}"),
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder()
        .source("inline", InlineSource::new())
        .sink(success_sink())
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Hash Validation Tests
// ============================================================================

/// Tests broker rejects hash mismatch inline json.
#[test]
fn broker_rejects_hash_mismatch_inline_json() {
    let json_value = json!({"key": "value"});
    // Use wrong hash
    let wrong_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"wrong");
    let envelope = sample_envelope("application/json", wrong_hash);
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hash mismatch"));
}

/// Tests broker rejects hash mismatch inline bytes.
#[test]
fn broker_rejects_hash_mismatch_inline_bytes() {
    let bytes = b"actual content";
    let wrong_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"expected content");
    let envelope = sample_envelope("application/octet-stream", wrong_hash);
    let payload = PacketPayload::Bytes {
        bytes: bytes.to_vec(),
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hash mismatch"));
}

/// Tests broker rejects hash mismatch external payload.
#[test]
fn broker_rejects_hash_mismatch_external_payload() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("payload.bin");
    std::fs::write(&path, b"actual").expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let wrong_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"expected");
    let content_ref = ContentRef {
        uri,
        content_hash: wrong_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", wrong_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(success_sink())
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hash mismatch"));
}

/// Tests broker rejects envelope content ref hash mismatch.
#[test]
fn broker_rejects_envelope_content_ref_hash_mismatch() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("payload.bin");
    let content = b"content";
    std::fs::write(&path, content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let correct_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, content);
    let different_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"different");

    // Content ref has correct hash but envelope has different hash
    let content_ref = ContentRef {
        uri,
        content_hash: correct_hash,
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", different_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(success_sink())
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hash mismatch"));
}

// ============================================================================
// SECTION: Source Resolution Tests
// ============================================================================

/// Tests broker rejects missing source.
#[test]
fn broker_rejects_missing_source() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "unknown://example.com/file".to_string(),
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing source"));
}

/// Tests broker resolves compound scheme.
#[test]
fn broker_resolves_compound_scheme() {
    // Test that compound schemes (e.g., "inline+custom") fall back to base scheme ("inline")
    // The broker splits on '+' and looks up the base scheme in the source registry

    let data = b"compound inline";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    // inline+bytes should be handled by inline source directly
    // To test compound scheme fallback, we'd need something like "inline+custom"
    // which would fall back to "inline"
    let content_ref = ContentRef {
        uri: format!("inline+custom:{encoded}"),
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    // Register only "inline" source - the "inline+custom" should fall back to it
    let broker = CompositeBroker::builder()
        .source("inline", InlineSource::new())
        .sink(success_sink())
        .build()
        .expect("build broker");

    // This should work because inline+custom falls back to inline
    // But InlineSource doesn't handle "inline+custom:" prefix...
    // The broker resolves the SOURCE by scheme, but the source still needs to handle the URI
    // So this actually tests source resolution, not source handling
    let result = broker.dispatch(&sample_target(), &envelope, &payload);

    // The source is resolved to "inline", but InlineSource doesn't handle "inline+custom:"
    // So this should fail at the source level, not source resolution
    assert!(result.is_err());
}

/// Tests broker rejects invalid uri.
#[test]
fn broker_rejects_invalid_uri() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "not a valid uri at all".to_string(),
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid uri"));
}

// ============================================================================
// SECTION: Content Type Detection Tests
// ============================================================================

/// Tests broker parses json content type.
#[test]
fn broker_parses_json_content_type() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("data.json");
    let json_bytes = br#"{"parsed": true}"#;
    std::fs::write(&path, json_bytes).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash =
        hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"parsed": true})).expect("hash");
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let sink = CallbackSink::new(|_, payload| {
        assert!(matches!(payload.body, PayloadBody::Json(_)));
        Ok(DispatchReceipt {
            dispatch_id: "json-parsed".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

/// Tests broker parses json with charset.
#[test]
fn broker_parses_json_with_charset() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("data.json");
    let json_bytes = br#"{"charset": "utf8"}"#;
    std::fs::write(&path, json_bytes).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash =
        hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"charset": "utf8"})).expect("hash");
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/json; charset=utf-8", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let sink = CallbackSink::new(|_, payload| {
        assert!(matches!(payload.body, PayloadBody::Json(_)));
        Ok(DispatchReceipt {
            dispatch_id: "json-charset".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

/// Tests broker parses json content type case-insensitively.
#[test]
fn broker_parses_json_content_type_case_insensitive() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("data.json");
    let json_bytes = br#"{"case": "insensitive"}"#;
    std::fs::write(&path, json_bytes).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash =
        hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"case": "insensitive"})).expect("hash");
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("Application/JSON; Charset=UTF-8", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let sink = CallbackSink::new(|_, payload| {
        assert!(matches!(payload.body, PayloadBody::Json(_)));
        Ok(DispatchReceipt {
            dispatch_id: "json-case".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

/// Tests broker parses vendor json type.
#[test]
fn broker_parses_vendor_json_type() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("data.json");
    let json_bytes = br#"{"vendor": "api"}"#;
    std::fs::write(&path, json_bytes).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash =
        hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"vendor": "api"})).expect("hash");
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/vnd.api+json", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let sink = CallbackSink::new(|_, payload| {
        assert!(matches!(payload.body, PayloadBody::Json(_)));
        Ok(DispatchReceipt {
            dispatch_id: "vendor-json".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Content Type Validation Tests
// ============================================================================

/// Tests broker rejects json payload with non-json content type.
#[test]
fn broker_rejects_json_payload_with_non_json_content_type() {
    let json_value = json!({"key": "value"});
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value).expect("hash");
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let err = broker.dispatch(&sample_target(), &envelope, &payload).unwrap_err();
    assert!(err.to_string().contains("payload kind json"));
}

/// Tests broker rejects bytes payload with json content type.
#[test]
fn broker_rejects_bytes_payload_with_json_content_type() {
    let bytes = b"binary data";
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, bytes);
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::Bytes {
        bytes: bytes.to_vec(),
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    let err = broker.dispatch(&sample_target(), &envelope, &payload).unwrap_err();
    assert!(err.to_string().contains("payload kind bytes"));
}

/// Tests broker rejects source content type mismatch.
#[test]
fn broker_rejects_source_content_type_mismatch() {
    let data = b"inline external";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);
    let content_ref = ContentRef {
        uri: format!("inline+bytes:{encoded}"),
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder()
        .source("inline", InlineSource::new())
        .sink(success_sink())
        .build()
        .expect("build broker");

    let err = broker.dispatch(&sample_target(), &envelope, &payload).unwrap_err();
    assert!(err.to_string().contains("source content type mismatch"));
}

/// Tests broker keeps binary for non json type.
#[test]
fn broker_keeps_binary_for_non_json_type() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("data.bin");
    let bytes = b"binary content";
    std::fs::write(&path, bytes).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, bytes);
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let sink = CallbackSink::new(|_, payload| {
        assert!(matches!(payload.body, PayloadBody::Bytes(_)));
        Ok(DispatchReceipt {
            dispatch_id: "binary-kept".to_string(),
            target: sample_target(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_ok());
}

/// Tests broker rejects invalid json in json content type.
#[test]
fn broker_rejects_invalid_json_in_json_content_type() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("invalid.json");
    let invalid_json = b"not valid json {{{";
    std::fs::write(&path, invalid_json).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    // Use bytes hash since it's not valid JSON
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, invalid_json);
    let content_ref = ContentRef {
        uri,
        content_hash: content_hash.clone(),
        encryption: None,
    };
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::External {
        content_ref,
    };

    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(success_sink())
        .build()
        .expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("json parse"));
}

// ============================================================================
// SECTION: Sink Error Propagation Tests
// ============================================================================

/// Tests broker propagates sink error.
#[test]
fn broker_propagates_sink_error() {
    let json_value = json!({"key": "value"});
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value).expect("hash");
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let failing_sink =
        CallbackSink::new(|_, _| Err(SinkError::DeliveryFailed("sink failed".to_string())));

    let broker = CompositeBroker::builder().sink(failing_sink).build().expect("build broker");

    let result = broker.dispatch(&sample_target(), &envelope, &payload);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("sink failed"));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests broker handles different agent targets.
#[test]
fn broker_handles_different_agent_targets() {
    let json_value = json!({"test": true});
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value).expect("hash");
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    // Test different agent targets
    let targets = vec![
        DispatchTarget::Agent {
            agent_id: "agent-1".to_string(),
        },
        DispatchTarget::Agent {
            agent_id: "agent-2".to_string(),
        },
        DispatchTarget::Agent {
            agent_id: "special-agent".to_string(),
        },
    ];

    for target in targets {
        assert!(broker.dispatch(&target, &envelope, &payload).is_ok());
    }
}

/// Tests broker multiple dispatches same payload.
#[test]
fn broker_multiple_dispatches_same_payload() {
    let json_value = json!({"reusable": true});
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value).expect("hash");
    let envelope = sample_envelope("application/json", content_hash);
    let payload = PacketPayload::Json {
        value: json_value,
    };

    let broker = CompositeBroker::builder().sink(success_sink()).build().expect("build broker");

    // Same payload should work multiple times
    for _ in 0 .. 5 {
        let result = broker.dispatch(&sample_target(), &envelope, &payload);
        assert!(result.is_ok());
    }
}
