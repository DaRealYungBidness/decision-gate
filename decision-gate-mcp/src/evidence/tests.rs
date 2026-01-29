// decision-gate-mcp/src/evidence/tests.rs
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

use std::io::BufReader;
use std::io::Cursor;

use decision_gate_core::CorrelationId;
use decision_gate_core::EvidenceContext;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;

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
