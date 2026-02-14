// crates/decision-gate-mcp/src/evidence/tests.rs
// ============================================================================
// Module: Evidence Framing Unit Tests
// Description: Tests for evidence stdio framing helpers.
// Purpose: Validate payload framing limits for evidence transport.
// Dependencies: decision-gate-mcp
// ============================================================================

//! ## Overview
//! Exercises evidence framing boundaries for MCP stdio payloads.
//!
//! Security posture: Tests exercise untrusted request handling; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

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
    reason = "Test-only framing assertions."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::io::BufReader;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;

use axum::Router;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::routing::post;
use decision_gate_core::CorrelationId;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceSignature;
use decision_gate_core::EvidenceValue;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use ed25519_dalek::Signer;
use serde_json::json;
use tokio::sync::oneshot;

use super::McpProviderClient;
use super::McpTransport;
use super::ProviderTrust;
use super::apply_signature_policy;
use super::ensure_evidence_hash;
use super::read_framed;
use super::request_id_for_context;
use super::sanitize_context_correlation_id;
use crate::config::ProviderConfig;
use crate::config::ProviderTimeoutConfig;
use crate::config::ProviderType;
use crate::config::TrustPolicy;
use crate::correlation::sanitize_client_correlation_id;

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn read_framed_rejects_large_payloads() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#;
    let framed =
        format!("Content-Length: {}\r\n\r\n{}", payload.len(), String::from_utf8_lossy(payload));
    let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
    let result = read_framed(&mut reader, payload.len() - 1);
    assert!(result.is_err());
}

#[test]
fn read_framed_accepts_payload_at_limit() {
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#;
    let framed =
        format!("Content-Length: {}\r\n\r\n{}", payload.len(), String::from_utf8_lossy(payload));
    let mut reader = BufReader::new(Cursor::new(framed.into_bytes()));
    let result = read_framed(&mut reader, payload.len());
    assert!(result.is_ok());
    let bytes = result.expect("payload read");
    assert_eq!(bytes, payload);
}

#[test]
fn request_id_uses_correlation_id_when_present() {
    let context = EvidenceContext {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(100).expect("nonzero namespaceid"),
        run_id: RunId::new("run"),
        scenario_id: ScenarioId::new("scenario"),
        stage_id: StageId::new("stage"),
        trigger_id: TriggerId::new("trigger"),
        trigger_time: Timestamp::Logical(1),
        correlation_id: Some(CorrelationId::new("corr-1")),
    };
    let sanitized = sanitize_context_correlation_id(&context).expect("valid correlation");
    let first = request_id_for_context(sanitized.as_deref());
    let second = request_id_for_context(sanitized.as_deref());
    assert_eq!(first, second);
    assert_eq!(first, serde_json::Value::String("corr-1".to_string()));
}

#[test]
fn request_id_increments_without_correlation_id() {
    let context = EvidenceContext {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(100).expect("nonzero namespaceid"),
        run_id: RunId::new("run"),
        scenario_id: ScenarioId::new("scenario"),
        stage_id: StageId::new("stage"),
        trigger_id: TriggerId::new("trigger"),
        trigger_time: Timestamp::Logical(1),
        correlation_id: None,
    };
    let sanitized = sanitize_context_correlation_id(&context).expect("valid context");
    let first = request_id_for_context(sanitized.as_deref());
    let second = request_id_for_context(sanitized.as_deref());
    assert_ne!(first, second);
    assert!(matches!(first, serde_json::Value::Number(_)));
}

#[test]
fn sanitize_client_correlation_id_rejects_invalid_chars() {
    assert!(sanitize_client_correlation_id(Some("valid-123")).unwrap().is_some());
    assert!(sanitize_client_correlation_id(Some("bad\nvalue")).is_err());
}

#[test]
fn signature_rejects_mismatched_evidence_hash() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let tampered_hash = hash_bytes(HashAlgorithm::Sha256, b"tampered");
    let message = canonical_json_bytes(&tampered_hash).expect("hash serialization");
    let signature_bytes = signing_key.sign(&message).to_bytes().to_vec();
    let signature = EvidenceSignature {
        scheme: "ed25519".to_string(),
        key_id: "test-key".to_string(),
        signature: signature_bytes,
    };

    let mut result = EvidenceResult {
        value: Some(EvidenceValue::Json(json!(true))),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: Some(tampered_hash),
        evidence_ref: None,
        evidence_anchor: None,
        signature: Some(signature),
        content_type: Some("application/json".to_string()),
    };

    let mut keys = BTreeMap::new();
    keys.insert("test-key".to_string(), verifying_key);
    let policy = ProviderTrust::RequireSignature {
        keys,
    };

    let err =
        apply_signature_policy(&policy, &mut result).expect_err("expected hash mismatch to fail");
    assert!(err.to_string().contains("evidence hash mismatch"));
}

#[test]
fn mcp_provider_client_from_config_uses_stdio_transport() {
    let mut config = base_provider_config();
    config.command = stdio_command();
    let client = McpProviderClient::from_config(&config).expect("client");
    match client.transport {
        McpTransport::Stdio {
            ..
        } => {}
        McpTransport::Http {
            ..
        } => panic!("expected stdio transport"),
    }
}

#[test]
fn mcp_provider_client_rejects_insecure_http() {
    let mut config = base_provider_config();
    config.url = Some("http://example.com".to_string());
    let err = McpProviderClient::from_config(&config).expect_err("expected insecure http reject");
    assert!(err.to_string().contains("insecure http disabled for provider"));
}

#[test]
fn mcp_provider_client_requires_url_when_no_command() {
    let config = base_provider_config();
    let err = McpProviderClient::from_config(&config).expect_err("missing url");
    assert!(err.to_string().contains("mcp url missing"));
}

#[test]
fn mcp_provider_client_http_query_decodes_json_result() {
    let (base_url, shutdown_tx, capture, join_handle) = spawn_http_server(StatusCode::OK);
    let mut config = base_provider_config();
    config.url = Some(base_url);
    config.allow_insecure_http = true;
    let client = McpProviderClient::from_config(&config).expect("client");

    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context(Some("corr-1"));
    let result = client.query(&query, &context).expect("query");

    assert_eq!(result.lane, TrustLane::Verified);
    assert_eq!(result.content_type.as_deref(), Some("application/json"));
    let captured = capture.lock().expect("capture lock").clone();
    assert_eq!(captured.as_deref(), Some("corr-1"));
    let _ = shutdown_tx.send(());
    let _ = join_handle.join();
}

#[test]
fn signature_policy_accepts_valid_signature() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let mut result = EvidenceResult {
        value: Some(EvidenceValue::Json(json!({"ok": true}))),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: Some("application/json".to_string()),
    };
    let hash = ensure_evidence_hash(&mut result).expect("hash");
    let message = canonical_json_bytes(&hash).expect("hash json");
    let signature_bytes = signing_key.sign(&message).to_bytes().to_vec();
    result.signature = Some(EvidenceSignature {
        scheme: "ed25519".to_string(),
        key_id: "key-1".to_string(),
        signature: signature_bytes,
    });

    let mut keys = BTreeMap::new();
    keys.insert("key-1".to_string(), verifying_key);
    let policy = ProviderTrust::RequireSignature {
        keys,
    };
    apply_signature_policy(&policy, &mut result).expect("signature accepted");
}

#[test]
fn signature_policy_rejects_missing_signature() {
    let mut keys = BTreeMap::new();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    keys.insert("key-1".to_string(), signing_key.verifying_key());
    let policy = ProviderTrust::RequireSignature {
        keys,
    };
    let mut result = EvidenceResult {
        value: Some(EvidenceValue::Json(json!({"ok": true}))),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };
    let err = apply_signature_policy(&policy, &mut result).expect_err("missing signature");
    assert!(err.to_string().contains("missing evidence signature"));
}

// ============================================================================
// SECTION: Fixtures
// ============================================================================

fn base_provider_config() -> ProviderConfig {
    ProviderConfig {
        name: "mcp-test".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: Some(TrustPolicy::Audit),
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }
}

fn sample_context(correlation_id: Option<&str>) -> EvidenceContext {
    EvidenceContext {
        tenant_id: TenantId::from_raw(100).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run"),
        scenario_id: ScenarioId::new("scenario"),
        stage_id: StageId::new("stage"),
        trigger_id: TriggerId::new("trigger"),
        trigger_time: Timestamp::Logical(1),
        correlation_id: correlation_id.map(decision_gate_core::CorrelationId::new),
    }
}

#[cfg(windows)]
fn stdio_command() -> Vec<String> {
    vec!["cmd".to_string(), "/C".to_string(), "more".to_string()]
}

#[cfg(not(windows))]
fn stdio_command() -> Vec<String> {
    vec!["/bin/sh".to_string(), "-c".to_string(), "cat".to_string()]
}

type SpawnedHttpServer =
    (String, oneshot::Sender<()>, Arc<Mutex<Option<String>>>, std::thread::JoinHandle<()>);

fn spawn_http_server(status: StatusCode) -> SpawnedHttpServer {
    let (addr_tx, addr_rx) = std::sync::mpsc::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let received = Arc::new(Mutex::new(None::<String>));
    let received_clone = Arc::clone(&received);
    let join_handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async move {
            let app = Router::new().route(
                "/",
                post(move |headers: HeaderMap| {
                    let received = Arc::clone(&received_clone);
                    async move {
                        let correlation = headers
                            .get("x-correlation-id")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_string);
                        *received.lock().expect("capture lock") = correlation;
                        let result = EvidenceResult {
                            value: Some(EvidenceValue::Json(json!({"ok": true}))),
                            lane: TrustLane::Verified,
                            error: None,
                            evidence_hash: None,
                            evidence_ref: None,
                            evidence_anchor: None,
                            signature: None,
                            content_type: Some("application/json".to_string()),
                        };
                        let body = json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "result": {
                                "content": [
                                    {
                                        "type": "json",
                                        "json": result
                                    }
                                ]
                            }
                        });
                        (status, axum::Json(body))
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("addr");
            addr_tx.send(addr).expect("addr send");
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
    });
    let addr = addr_rx.recv().expect("addr recv");
    (format!("http://{addr}"), shutdown_tx, received, join_handle)
}
