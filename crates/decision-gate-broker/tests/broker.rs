// crates/decision-gate-broker/tests/broker.rs
// ============================================================================
// Module: Decision Gate Broker Tests
// Description: Tests for sources, sinks, and composite broker behavior.
// Purpose: Exercise broker sources, sinks, and composite dispatch wiring.
// Dependencies: decision-gate-broker, decision-gate-core, base64, tempfile, tiny_http, url
// ============================================================================
//! ## Overview
//! Validates broker sources, sinks, and dispatcher wiring.

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
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use decision_gate_broker::CallbackSink;
use decision_gate_broker::ChannelSink;
use decision_gate_broker::CompositeBroker;
use decision_gate_broker::DispatchMessage;
use decision_gate_broker::FileSource;
use decision_gate_broker::HttpSource;
use decision_gate_broker::HttpSourcePolicy;
use decision_gate_broker::InlineSource;
use decision_gate_broker::LogSink;
use decision_gate_broker::Payload;
use decision_gate_broker::PayloadBody;
use decision_gate_broker::Sink;
use decision_gate_broker::SinkError;
use decision_gate_broker::Source;
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
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server;
use url::Url;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn sample_envelope(
    content_type: &str,
    content_hash: decision_gate_core::hashing::HashDigest,
) -> PacketEnvelope {
    PacketEnvelope {
        scenario_id: ScenarioId::new("scenario"),
        run_id: RunId::new("run"),
        stage_id: StageId::new("stage"),
        packet_id: PacketId::new("packet"),
        schema_id: SchemaId::new("schema"),
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
        agent_id: "agent-1".to_string(),
    }
}

// ============================================================================
// SECTION: Source Tests
// ============================================================================

/// Tests file source reads bytes.
#[test]
fn file_source_reads_bytes() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("payload.bin");
    std::fs::write(&path, b"hello").expect("write file");
    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"hello");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let payload = source.fetch(&content_ref).expect("file fetch");
    assert_eq!(payload.bytes, b"hello");
}

/// Tests inline source decodes bytes.
#[test]
fn inline_source_decodes_bytes() {
    let encoded = STANDARD.encode(b"inline-bytes");
    let content_ref = ContentRef {
        uri: format!("inline+bytes:{encoded}"),
        content_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"inline-bytes"),
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");
    assert_eq!(payload.bytes, b"inline-bytes");
}

/// Tests http source fetches bytes.
#[test]
fn http_source_fetches_bytes() {
    let server = Server::http("127.0.0.1:0").expect("http server");
    let addr = server.server_addr();
    let body = b"remote".to_vec();
    let handle = std::thread::spawn(move || {
        if let Ok(request) = server.recv() {
            let response = Response::from_data(body).with_header(
                Header::from_bytes("Content-Type", "application/octet-stream").unwrap(),
            );
            request.respond(response).expect("respond");
        }
    });

    let uri = format!("http://{addr}");
    let content_ref = ContentRef {
        uri,
        content_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"remote"),
        encryption: None,
    };
    let source = HttpSource::with_policy(HttpSourcePolicy::new().allow_private_networks())
        .expect("http source");
    let payload = source.fetch(&content_ref).expect("http fetch");
    assert_eq!(payload.bytes, b"remote");
    assert_eq!(payload.content_type.as_deref(), Some("application/octet-stream"));
    handle.join().expect("server thread");
}

// ============================================================================
// SECTION: Sink Tests
// ============================================================================

/// Tests callback sink invokes handler.
#[test]
fn callback_sink_invokes_handler() {
    let target = sample_target();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let envelope = sample_envelope("application/octet-stream", content_hash.clone());
    let payload = Payload {
        envelope,
        body: PayloadBody::Bytes(b"payload".to_vec()),
    };

    let expected_target = target.clone();
    let expected_hash = content_hash;
    let sink = CallbackSink::new(move |target, payload| {
        assert_eq!(target, &expected_target);
        assert_eq!(payload.envelope.content_hash, expected_hash);
        Ok(DispatchReceipt {
            dispatch_id: "callback-1".to_string(),
            target: target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "callback".to_string(),
        })
    });

    let receipt = sink.deliver(&target, &payload).expect("callback deliver");
    assert_eq!(receipt.dispatch_id, "callback-1");
}

/// Tests channel sink sends message.
#[test]
fn channel_sink_sends_message() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(1);
    let sink = ChannelSink::new(tx);

    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let payload = Payload {
        envelope: sample_envelope("application/octet-stream", content_hash),
        body: PayloadBody::Bytes(b"payload".to_vec()),
    };
    let target = sample_target();

    let receipt = sink.deliver(&target, &payload).expect("channel deliver");
    let message = rx.try_recv().expect("channel recv");
    assert_eq!(message.payload, payload);
    assert_eq!(message.receipt, receipt);
}

/// Tests log sink writes record.
#[test]
fn log_sink_writes_record() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let writer = SharedBuffer {
        inner: Arc::clone(&buffer),
    };
    let sink = LogSink::new(writer);

    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"log");
    let payload = Payload {
        envelope: sample_envelope("application/octet-stream", content_hash),
        body: PayloadBody::Bytes(b"log".to_vec()),
    };
    let target = sample_target();

    let receipt = sink.deliver(&target, &payload).expect("log deliver");
    let output = String::from_utf8(buffer.lock().unwrap().clone()).expect("utf8");
    assert!(output.contains(&receipt.dispatch_id));
}

// ============================================================================
// SECTION: Composite Broker Tests
// ============================================================================

/// Tests composite broker dispatches inline payload.
#[test]
fn composite_broker_dispatches_inline_payload() {
    let payload = PacketPayload::Json {
        value: json!({"hello": "world"}),
    };
    let content_hash =
        hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"hello": "world"})).expect("hash");
    let envelope = sample_envelope("application/json", content_hash);
    let target = sample_target();

    let expected_target = target.clone();
    let sink = CallbackSink::new(move |_, payload| {
        match &payload.body {
            PayloadBody::Json(value) => assert_eq!(value["hello"], "world"),
            PayloadBody::Bytes(_) => panic!("expected json payload"),
        }
        Ok(DispatchReceipt {
            dispatch_id: "sink-1".to_string(),
            target: expected_target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(1),
            dispatcher: "callback".to_string(),
        })
    });
    let broker = CompositeBroker::builder().sink(sink).build().expect("broker build");

    let receipt = broker.dispatch(&target, &envelope, &payload).expect("broker dispatch");
    assert_eq!(receipt.dispatch_id, "sink-1");
}

/// Tests composite broker rejects hash mismatch.
#[test]
fn composite_broker_rejects_hash_mismatch() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("payload.bin");
    std::fs::write(&path, b"actual").expect("write file");
    let uri = Url::from_file_path(&path).expect("file url").to_string();

    let content_ref = ContentRef {
        uri,
        content_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"expected"),
        encryption: None,
    };
    let envelope = sample_envelope("application/octet-stream", content_ref.content_hash.clone());
    let payload = PacketPayload::External {
        content_ref,
    };

    let sink = CallbackSink::new(|_, _| Err(SinkError::DeliveryFailed("unused".to_string())));
    let broker = CompositeBroker::builder()
        .source("file", FileSource::new(dir.path()))
        .sink(sink)
        .build()
        .expect("broker build");

    let err = broker.dispatch(&sample_target(), &envelope, &payload).unwrap_err();
    assert!(err.to_string().contains("payload hash mismatch"));
}

// ============================================================================
// SECTION: Shared Buffer
// ============================================================================

struct SharedBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl std::io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.lock().expect("buffer lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
