// decision-gate-broker/tests/sinks/callback_tests.rs
// ============================================================================
// Module: CallbackSink Unit Tests
// Description: Comprehensive tests for the callback-based payload sink.
// Purpose: Validate callback invocation and receipt handling.
// Dependencies: decision-gate-broker, decision-gate-core
// ============================================================================

//! ## Overview
//! Exercises [`decision_gate_broker::CallbackSink`] handler execution paths.

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

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use decision_gate_broker::CallbackSink;
use decision_gate_broker::Payload;
use decision_gate_broker::PayloadBody;
use decision_gate_broker::Sink;
use decision_gate_broker::SinkError;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;

use super::common::sample_bytes_envelope;
use super::common::sample_target;

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

/// Tests callback sink new creates sink.
#[test]
fn callback_sink_new_creates_sink() {
    let _sink = CallbackSink::new(|_target, _payload| {
        Ok(DispatchReceipt {
            dispatch_id: "test".to_string(),
            target: DispatchTarget::Agent {
                agent_id: "test".to_string(),
            },
            receipt_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"test"),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });
    // Sink created successfully
}

// ============================================================================
// SECTION: Success Path Tests
// ============================================================================

/// Tests callback sink invokes handler with correct arguments.
#[test]
fn callback_sink_invokes_handler_with_correct_arguments() {
    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"payload data"),
        body: PayloadBody::Bytes(b"payload data".to_vec()),
    };

    let expected_target = target.clone();
    let expected_hash = payload.envelope.content_hash.clone();

    let sink = CallbackSink::new(move |received_target, received_payload| {
        assert_eq!(received_target, &expected_target);
        assert_eq!(received_payload.envelope.content_hash, expected_hash);
        Ok(DispatchReceipt {
            dispatch_id: "callback-receipt".to_string(),
            target: received_target.clone(),
            receipt_hash: received_payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "callback".to_string(),
        })
    });

    let receipt = sink.deliver(&target, &payload).expect("callback deliver");
    assert_eq!(receipt.dispatch_id, "callback-receipt");
    assert_eq!(receipt.dispatcher, "callback");
}

/// Tests callback sink returns receipt from handler.
#[test]
fn callback_sink_returns_receipt_from_handler() {
    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let custom_receipt = DispatchReceipt {
        dispatch_id: "custom-id-12345".to_string(),
        target: target.clone(),
        receipt_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"custom"),
        dispatched_at: Timestamp::Logical(42),
        dispatcher: "custom-dispatcher".to_string(),
    };
    let expected_receipt = custom_receipt.clone();

    let sink = CallbackSink::new(move |_, _| Ok(custom_receipt.clone()));

    let receipt = sink.deliver(&target, &payload).expect("callback deliver");
    assert_eq!(receipt.dispatch_id, expected_receipt.dispatch_id);
    assert_eq!(receipt.dispatched_at, expected_receipt.dispatched_at);
    assert_eq!(receipt.dispatcher, expected_receipt.dispatcher);
}

/// Tests callback sink handler called multiple times.
#[test]
fn callback_sink_handler_called_multiple_times() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&call_count);

    let sink = CallbackSink::new(move |target, payload| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(DispatchReceipt {
            dispatch_id: format!("call-{}", counter.load(Ordering::SeqCst)),
            target: target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "counter".to_string(),
        })
    });

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    for i in 1 ..= 5 {
        let receipt = sink.deliver(&target, &payload).expect("callback deliver");
        assert_eq!(call_count.load(Ordering::SeqCst), i);
        assert_eq!(receipt.dispatch_id, format!("call-{i}"));
    }
}

/// Tests callback sink is clone.
#[test]
fn callback_sink_is_clone() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&call_count);

    let sink = CallbackSink::new(move |target, payload| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(DispatchReceipt {
            dispatch_id: "cloned".to_string(),
            target: target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "clone-test".to_string(),
        })
    });

    let cloned_sink = sink.clone();

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    sink.deliver(&target, &payload).expect("original deliver");
    cloned_sink.deliver(&target, &payload).expect("cloned deliver");

    // Both sinks share the same handler and counter
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

// ============================================================================
// SECTION: Error Path Tests
// ============================================================================

/// Tests callback sink propagates handler error.
#[test]
fn callback_sink_propagates_handler_error() {
    let sink =
        CallbackSink::new(|_, _| Err(SinkError::DeliveryFailed("handler failed".to_string())));

    let target = sample_target();
    let payload = Payload {
        envelope: sample_bytes_envelope(b"data"),
        body: PayloadBody::Bytes(b"data".to_vec()),
    };

    let err = sink.deliver(&target, &payload).unwrap_err();

    assert!(matches!(err, SinkError::DeliveryFailed(_)));
    assert!(err.to_string().contains("handler failed"));
}

/// Tests callback sink handler can return different errors.
#[test]
fn callback_sink_handler_can_return_different_errors() {
    let error_messages = vec!["connection refused", "timeout exceeded", "invalid target"];

    for msg in error_messages {
        let error_msg = msg.to_string();
        let sink = CallbackSink::new(move |_, _| Err(SinkError::DeliveryFailed(error_msg.clone())));

        let target = sample_target();
        let payload = Payload {
            envelope: sample_bytes_envelope(b"data"),
            body: PayloadBody::Bytes(b"data".to_vec()),
        };

        let err = sink.deliver(&target, &payload).unwrap_err();
        assert!(err.to_string().contains(msg));
    }
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests callback sink handles different agent targets.
#[test]
fn callback_sink_handles_different_agent_targets() {
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

    for expected_target in targets {
        let captured_target = expected_target.clone();
        let sink = CallbackSink::new(move |target, payload| {
            assert_eq!(target, &captured_target);
            Ok(DispatchReceipt {
                dispatch_id: "target-test".to_string(),
                target: target.clone(),
                receipt_hash: payload.envelope.content_hash.clone(),
                dispatched_at: Timestamp::Logical(1),
                dispatcher: "test".to_string(),
            })
        });

        let payload = Payload {
            envelope: sample_bytes_envelope(b"data"),
            body: PayloadBody::Bytes(b"data".to_vec()),
        };

        sink.deliver(&expected_target, &payload).expect("deliver");
    }
}

/// Tests callback sink handles json payload.
#[test]
fn callback_sink_handles_json_payload() {
    let json_value = serde_json::json!({"key": "value", "count": 42});
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
        issued_at: Timestamp::Logical(1),
    };

    let expected_json = json_value.clone();
    let payload = Payload {
        envelope,
        body: PayloadBody::Json(json_value),
    };

    let sink = CallbackSink::new(move |target, received_payload| {
        if let PayloadBody::Json(value) = &received_payload.body {
            assert_eq!(value, &expected_json);
        } else {
            panic!("expected JSON payload");
        }
        Ok(DispatchReceipt {
            dispatch_id: "json-test".to_string(),
            target: target.clone(),
            receipt_hash: received_payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "test".to_string(),
        })
    });

    let target = sample_target();
    sink.deliver(&target, &payload).expect("deliver");
}
