// decision-gate-broker/tests/sinks/log_tests.rs
// ============================================================================
// Module: LogSink Unit Tests
// Description: Comprehensive tests for the log-based payload sink.
// ============================================================================

use decision_gate_broker::LogSink;
use decision_gate_broker::Payload;
use decision_gate_broker::PayloadBody;
use decision_gate_broker::Sink;
use decision_gate_broker::SinkError;
use serde_json::Value;

use super::common::sample_bytes_envelope;
use super::common::sample_json_envelope;
use super::common::sample_target;
use super::common::FailingWriter;
use super::common::SharedBuffer;

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

#[test]
fn log_sink_new_creates_sink_with_default_dispatcher() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("deliver");
    assert!(receipt.dispatch_id.starts_with("log-"));
    assert_eq!(receipt.dispatcher, "log");
}

#[test]
fn log_sink_with_dispatcher_uses_custom_name() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::with_dispatcher(buffer.clone(), "audit-log");

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("deliver");
    assert!(receipt.dispatch_id.starts_with("audit-log-"));
    assert_eq!(receipt.dispatcher, "audit-log");
}

// ============================================================================
// SECTION: Success Path Tests
// ============================================================================

#[test]
fn log_sink_writes_json_record() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"log data"),
        body: PayloadBody::Bytes(b"log data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    assert!(!output.is_empty());

    // Parse the JSON record
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert_eq!(record["dispatch_id"], receipt.dispatch_id);
    assert_eq!(record["dispatcher"], "log");
    assert_eq!(record["content_type"], "application/octet-stream");
    assert_eq!(record["payload_len"], 8); // "log data" is 8 bytes
}

#[test]
fn log_sink_record_contains_dispatch_id() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("deliver");
    let output = buffer.to_string_lossy();

    assert!(output.contains(&receipt.dispatch_id));
}

#[test]
fn log_sink_record_contains_content_hash() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"hashed content"),
        body: PayloadBody::Bytes(b"hashed content".to_vec()),
    };

    sink.deliver(&target, &payload).expect("deliver");
    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    // Verify content_hash is present and has expected structure
    assert!(record["content_hash"].is_object());
    assert!(record["content_hash"]["algorithm"].is_string());
    assert!(record["content_hash"]["value"].is_string());
}

#[test]
fn log_sink_record_contains_target() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    sink.deliver(&target, &payload).expect("deliver");
    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert!(record["target"].is_object());
}

#[test]
fn log_sink_increments_sequence_number() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();

    for i in 1..=3 {
        let payload = Payload {
            envelope: sample_bytes_envelope(b"data"),
            body: PayloadBody::Bytes(b"data".to_vec()),
        };
        let receipt = sink.deliver(&target, &payload).expect("deliver");
        assert_eq!(receipt.dispatch_id, format!("log-{i}"));
    }
}

#[test]
fn log_sink_writes_newline_after_each_record() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    sink.deliver(&target, &payload).expect("deliver");
    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let lines: Vec<_> = output.lines().collect();

    // Should have 2 lines (2 records)
    assert_eq!(lines.len(), 2);

    // Each line should be valid JSON
    for line in lines {
        let _: Value = serde_json::from_str(line).expect("parse json line");
    }
}

#[test]
fn log_sink_handles_json_payload() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let json_value = serde_json::json!({"key": "value", "nested": {"a": 1}});
    let envelope = sample_json_envelope(&json_value);
    let payload = Payload {
        envelope,
        body: PayloadBody::Json(json_value.clone()),
    };

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert_eq!(record["content_type"], "application/json");
    // payload_len should be the serialized JSON length
    assert!(record["payload_len"].as_u64().unwrap() > 0);
}

#[test]
fn log_sink_reports_correct_payload_length_for_bytes() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let data = b"exactly 20 bytes!!!";
    assert_eq!(data.len(), 19); // Verify our test data

    let payload = Payload {
        envelope: sample_bytes_envelope(data),
        body: PayloadBody::Bytes(data.to_vec()),
    };

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert_eq!(record["payload_len"], 19);
}

#[test]
fn log_sink_reports_correct_payload_length_for_empty() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let payload = Payload {
        envelope: sample_bytes_envelope(b""),
        body: PayloadBody::Bytes(vec![]),
    };

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert_eq!(record["payload_len"], 0);
}

// ============================================================================
// SECTION: Error Path Tests
// ============================================================================

#[test]
fn log_sink_fails_on_write_error() {
    let sink = LogSink::new(FailingWriter);

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let err = sink.deliver(&target, &payload).unwrap_err();

    assert!(matches!(err, SinkError::LogWriteFailed(_)));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

#[test]
fn log_sink_handles_large_payload() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let large_data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
    let payload = Payload {
        envelope: sample_bytes_envelope(&large_data),
        body: PayloadBody::Bytes(large_data.clone()),
    };

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert_eq!(record["payload_len"], 10000);
}

#[test]
fn log_sink_handles_special_characters_in_content_type() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let mut envelope = sample_bytes_envelope(b"data");
    envelope.content_type = "application/json; charset=utf-8; boundary=\"===boundary===\"".to_string();

    let payload = Payload {
        envelope,
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    assert_eq!(
        record["content_type"],
        "application/json; charset=utf-8; boundary=\"===boundary===\""
    );
}

#[test]
fn log_sink_handles_agent_target_serialization() {
    let buffer = SharedBuffer::new();
    let sink = LogSink::new(buffer.clone());

    let target = decision_gate_core::DispatchTarget::Agent {
        agent_id: "test-agent-123".to_string(),
    };
    let payload = Payload {
        envelope: sample_bytes_envelope(b"agent data"),
        body: PayloadBody::Bytes(b"agent data".to_vec()),
    };

    sink.deliver(&target, &payload).expect("deliver");

    let output = buffer.to_string_lossy();
    let record: Value = serde_json::from_str(&output).expect("parse json");

    // DispatchTarget uses serde internally tagged: {"kind": "agent", "agent_id": "..."}
    assert_eq!(record["target"]["kind"], "agent");
    assert_eq!(record["target"]["agent_id"], "test-agent-123");
}
