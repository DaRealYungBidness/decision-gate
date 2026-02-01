// system-tests/tests/suites/broker_integration.rs
// ============================================================================
// Module: Broker Integration Tests
// Description: End-to-end CompositeBroker wiring coverage.
// Purpose: Validate file/http/inline sources and sink dispatch behavior.
// Dependencies: decision-gate-broker, system-tests helpers
// ============================================================================

//! Composite broker integration coverage for system-tests.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::header::CONTENT_TYPE;
use axum::routing::get;
use base64::Engine;
use decision_gate_broker::ChannelSink;
use decision_gate_broker::CompositeBroker;
use decision_gate_broker::DispatchMessage;
use decision_gate_broker::FileSource;
use decision_gate_broker::HttpSource;
use decision_gate_broker::HttpSourcePolicy;
use decision_gate_broker::InlineSource;
use decision_gate_broker::PayloadBody;
use decision_gate_core::ContentRef;
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
use decision_gate_core::hashing::HashDigest;
use decision_gate_core::hashing::hash_canonical_json;
use tempfile::TempDir;
use url::Url;

use crate::helpers::artifacts::TestReporter;

fn sample_envelope(
    packet_id: &str,
    content_type: &str,
    content_hash: HashDigest,
) -> PacketEnvelope {
    PacketEnvelope {
        scenario_id: ScenarioId::new("broker-scenario"),
        run_id: RunId::new("run-1"),
        stage_id: StageId::new("stage-1"),
        packet_id: PacketId::new(packet_id),
        schema_id: SchemaId::new("schema-1"),
        content_type: content_type.to_string(),
        content_hash,
        visibility: VisibilityPolicy::new(vec![], vec![]),
        expiry: None,
        correlation_id: None,
        issued_at: Timestamp::Logical(1),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "CompositeBroker flow kept in one sequence for auditability."
)]
async fn broker_composite_sources_and_sinks() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("broker_composite_sources_and_sinks")?;
    let temp_dir = TempDir::new()?;

    let file_value = serde_json::json!({"source": "file"});
    let file_bytes = serde_json::to_vec(&file_value)?;
    let file_path = temp_dir.path().join("payload.json");
    std::fs::write(&file_path, &file_bytes)?;

    let http_value = serde_json::json!({"source": "http"});
    let http_bytes = Bytes::from(serde_json::to_vec(&http_value)?);
    let app = Router::new().route(
        "/payload",
        get({
            let http_bytes = http_bytes.clone();
            move || {
                let http_bytes = http_bytes.clone();
                async move { ([(CONTENT_TYPE, "application/json")], http_bytes) }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let inline_value = serde_json::json!({"source": "inline"});
    let inline_bytes = serde_json::to_vec(&inline_value)?;
    let inline_encoded = base64::engine::general_purpose::STANDARD.encode(inline_bytes);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<DispatchMessage>(8);
    let http_policy =
        HttpSourcePolicy::new().allow_hosts([addr.ip().to_string()]).allow_private_networks();
    let file_root = temp_dir.path().to_path_buf();

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };

    let mut expected = BTreeMap::new();

    let file_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &file_value)?;
    let file_envelope = sample_envelope("packet-file", "application/json", file_hash.clone());
    let file_ref = ContentRef {
        uri: Url::from_file_path(&file_path).expect("file url").to_string(),
        content_hash: file_hash,
        encryption: None,
    };
    expected.insert("packet-file", file_value);

    let http_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &http_value)?;
    let http_envelope = sample_envelope("packet-http", "application/json", http_hash.clone());
    let http_ref = ContentRef {
        uri: format!("http://{addr}/payload"),
        content_hash: http_hash,
        encryption: None,
    };
    expected.insert("packet-http", http_value);

    let inline_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &inline_value)?;
    let inline_envelope = sample_envelope("packet-inline", "application/json", inline_hash.clone());
    let inline_ref = ContentRef {
        uri: format!("inline+json:{inline_encoded}"),
        content_hash: inline_hash,
        encryption: None,
    };
    expected.insert("packet-inline", inline_value);

    let dispatch_handle = std::thread::spawn(move || -> Result<(), String> {
        let sink = ChannelSink::new(tx);
        let http_source = HttpSource::with_policy(http_policy).map_err(|err| err.to_string())?;
        let broker = CompositeBroker::builder()
            .source("file", FileSource::new(file_root))
            .source("http", http_source)
            .source("inline", InlineSource::new())
            .sink(sink)
            .build()
            .map_err(|err| err.to_string())?;
        broker
            .dispatch(
                &target,
                &file_envelope,
                &PacketPayload::External {
                    content_ref: file_ref,
                },
            )
            .map_err(|err| err.to_string())?;
        broker
            .dispatch(
                &target,
                &http_envelope,
                &PacketPayload::External {
                    content_ref: http_ref,
                },
            )
            .map_err(|err| err.to_string())?;
        broker
            .dispatch(
                &target,
                &inline_envelope,
                &PacketPayload::External {
                    content_ref: inline_ref,
                },
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    });
    tokio::task::block_in_place(|| {
        dispatch_handle.join().map_err(|_| "broker dispatch thread panicked".to_string())
    })??;

    let mut received = Vec::new();
    for _ in 0 .. 3 {
        let message = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .map_err(|_| "timed out waiting for broker dispatch")?
            .ok_or("broker dispatch channel closed")?;
        received.push(message);
    }

    for message in received {
        let packet_id = message.payload.envelope.packet_id.as_str();
        let expected_value = expected
            .remove(packet_id)
            .ok_or_else(|| format!("unexpected packet id {packet_id}"))?;
        match &message.payload.body {
            PayloadBody::Json(value) => {
                if value != &expected_value {
                    return Err(format!("payload mismatch for {packet_id}").into());
                }
            }
            PayloadBody::Bytes(_) => {
                return Err(format!("expected json payload for {packet_id}").into());
            }
        }
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["CompositeBroker file/http/inline wiring validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    let _ = shutdown_tx.send(());
    Ok(())
}
