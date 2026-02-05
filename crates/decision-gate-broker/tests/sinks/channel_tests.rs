// crates/decision-gate-broker/tests/sinks/channel_tests.rs
// ============================================================================
// Module: ChannelSink Unit Tests
// Description: Comprehensive tests for the channel-based async payload sink.
// Purpose: Validate channel dispatch behavior and receipt generation.
// Dependencies: decision-gate-broker, decision-gate-core, tokio
// ============================================================================

//! ## Overview
//! Exercises [`decision_gate_broker::ChannelSink`] message delivery behavior.

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

use decision_gate_broker::ChannelSink;
use decision_gate_broker::DispatchMessage;
use decision_gate_broker::Payload;
use decision_gate_broker::PayloadBody;
use decision_gate_broker::Sink;
use decision_gate_broker::SinkError;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;

use super::common::sample_bytes_envelope;
use super::common::sample_target;

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

/// Tests channel sink new creates sink with default dispatcher.
#[test]
fn channel_sink_new_creates_sink_with_default_dispatcher() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let _sink = ChannelSink::new(tx);
    // Sink created successfully
}

/// Tests channel sink with dispatcher uses custom name.
#[test]
fn channel_sink_with_dispatcher_uses_custom_name() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::with_dispatcher(tx, "custom-dispatcher");

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("deliver");
    assert!(receipt.dispatch_id.starts_with("custom-dispatcher-"));
    assert_eq!(receipt.dispatcher, "custom-dispatcher");
}

// ============================================================================
// SECTION: Success Path Tests
// ============================================================================

/// Tests channel sink sends message to channel.
#[test]
fn channel_sink_sends_message_to_channel() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"channel data"),
        body: PayloadBody::Bytes(b"channel data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("channel deliver");
    let message = rx.try_recv().expect("channel recv");

    assert_eq!(message.payload, payload);
    assert_eq!(message.target, target);
    assert_eq!(message.receipt, receipt);
}

/// Tests channel sink returns correct receipt.
#[test]
fn channel_sink_returns_correct_receipt() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<DispatchMessage>(10);
    let sink = ChannelSink::new(tx);

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let receipt = sink.deliver(&target, &payload).expect("deliver");

    assert_eq!(receipt.dispatch_id, "channel-1");
    assert_eq!(receipt.dispatcher, "channel");
    assert_eq!(receipt.target, target);
    assert_eq!(receipt.receipt_hash, payload.envelope.content_hash);
}

/// Tests channel sink increments sequence number.
#[test]
fn channel_sink_increments_sequence_number() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(10);
    let sink = ChannelSink::new(tx);

    let target = sample_target();
    let payloads: Vec<_> = (0 .. 5)
        .map(|i| {
            let data = format!("data-{i}");
            Payload {
                envelope: sample_bytes_envelope(data.as_bytes()),
                body: PayloadBody::Bytes(data.into_bytes()),
            }
        })
        .collect();

    for (i, payload) in payloads.iter().enumerate() {
        let receipt = sink.deliver(&target, payload).expect("deliver");
        assert_eq!(receipt.dispatch_id, format!("channel-{}", i + 1));

        let message = rx.try_recv().expect("recv");
        assert_eq!(message.receipt.dispatch_id, format!("channel-{}", i + 1));
    }
}

/// Tests channel sink message contains cloned payload.
#[test]
fn channel_sink_message_contains_cloned_payload() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    let target = sample_target();
    let original_data = b"original payload content";
    let payload = Payload {
        envelope: sample_bytes_envelope(original_data),
        body: PayloadBody::Bytes(original_data.to_vec()),
    };

    sink.deliver(&target, &payload).expect("deliver");
    let message = rx.try_recv().expect("recv");

    // Verify the payload was properly cloned
    assert_eq!(message.payload.envelope, payload.envelope);
    if let PayloadBody::Bytes(bytes) = &message.payload.body {
        assert_eq!(bytes, original_data);
    } else {
        panic!("expected Bytes payload");
    }
}

/// Tests channel sink handles json payload.
#[test]
fn channel_sink_handles_json_payload() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    let json_value = serde_json::json!({"key": "value"});
    let content_hash =
        decision_gate_core::hashing::hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json_value)
            .expect("json hash");

    let envelope = decision_gate_core::PacketEnvelope {
        scenario_id: decision_gate_core::ScenarioId::new("test"),
        run_id: decision_gate_core::RunId::new("test"),
        stage_id: decision_gate_core::StageId::new("test"),
        packet_id: decision_gate_core::PacketId::new("test"),
        schema_id: decision_gate_core::SchemaId::new("test"),
        content_type: "application/json".to_string(),
        content_hash,
        visibility: decision_gate_core::VisibilityPolicy::new(vec![], vec![]),
        expiry: None,
        correlation_id: None,
        issued_at: decision_gate_core::Timestamp::Logical(1),
    };

    let payload = Payload {
        envelope,
        body: PayloadBody::Json(json_value.clone()),
    };

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");

    let message = rx.try_recv().expect("recv");
    if let PayloadBody::Json(value) = &message.payload.body {
        assert_eq!(value, &json_value);
    } else {
        panic!("expected JSON payload");
    }
}

// ============================================================================
// SECTION: Error Path Tests
// ============================================================================

/// Tests channel sink fails when channel full.
#[test]
fn channel_sink_fails_when_channel_full() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    // First send should succeed
    sink.deliver(&target, &payload).expect("first deliver");

    // Second send should fail (channel full, buffer size 1)
    let err = sink.deliver(&target, &payload).unwrap_err();

    assert!(matches!(err, SinkError::DeliveryFailed(_)));
    assert!(err.to_string().contains("full") || err.to_string().contains("no available capacity"));
}

/// Tests channel sink fails when receiver dropped.
#[test]
fn channel_sink_fails_when_receiver_dropped() {
    let (tx, rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    // Drop the receiver
    drop(rx);

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let err = sink.deliver(&target, &payload).unwrap_err();

    assert!(matches!(err, SinkError::DeliveryFailed(_)));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests channel sink handles large channel buffer.
#[test]
fn channel_sink_handles_large_channel_buffer() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1000);
    let sink = ChannelSink::new(tx);

    let target = sample_target();

    // Send many messages
    for i in 0 .. 100 {
        let data = format!("data-{i}");
        let payload = Payload {
            envelope: sample_bytes_envelope(data.as_bytes()),
            body: PayloadBody::Bytes(data.into_bytes()),
        };
        sink.deliver(&target, &payload).expect("deliver");
    }

    // Verify all messages received in order
    for i in 0 .. 100 {
        let message = rx.try_recv().expect("recv");
        assert_eq!(message.receipt.dispatch_id, format!("channel-{}", i + 1));
    }
}

/// Tests channel sink receipt timestamp increments.
#[test]
fn channel_sink_receipt_timestamp_increments() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<DispatchMessage>(10);
    let sink = ChannelSink::new(tx);

    let target = sample_target();

    for i in 1 ..= 3 {
        let payload = Payload {
            envelope: sample_bytes_envelope(b"data"),
            body: PayloadBody::Bytes(b"data".to_vec()),
        };
        let receipt = sink.deliver(&target, &payload).expect("deliver");

        if let decision_gate_core::Timestamp::Logical(ts) = receipt.dispatched_at {
            assert_eq!(ts, i);
        } else {
            panic!("expected Logical timestamp");
        }
    }
}

/// Tests channel sink handles empty payload.
#[test]
fn channel_sink_handles_empty_payload() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b""),
        body: PayloadBody::Bytes(vec![]),
    };

    sink.deliver(&target, &payload).expect("deliver");
    let message = rx.try_recv().expect("recv");

    if let PayloadBody::Bytes(bytes) = &message.payload.body {
        assert!(bytes.is_empty());
    } else {
        panic!("expected Bytes payload");
    }
}
