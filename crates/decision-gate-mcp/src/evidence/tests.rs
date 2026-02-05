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

use decision_gate_core::CorrelationId;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceSignature;
use decision_gate_core::EvidenceValue;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::NamespaceId;
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

use super::ProviderTrust;
use super::apply_signature_policy;
use super::read_framed;
use super::request_id_for_context;
use super::sanitize_context_correlation_id;
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
