// decision-gate-mcp/src/audit.rs
// ============================================================================
// Module: MCP Audit Logging
// Description: Structured audit events for MCP request handling.
// Purpose: Emit redacted audit logs without hard dependencies.
// Dependencies: decision-gate-contract, serde
// ============================================================================

//! ## Overview
//! This module defines audit event payloads and sinks for MCP request logging.
//! It is intentionally lightweight so deployments can route events to their
//! preferred logging pipeline without redesign.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_contract::ToolName;
use serde::Serialize;

use crate::config::ServerTransport;
use crate::telemetry::McpMethod;
use crate::telemetry::McpOutcome;

// ============================================================================
// SECTION: Types
// ============================================================================

/// MCP audit event payload.
#[derive(Debug, Clone, Serialize)]
pub struct McpAuditEvent {
    /// Event identifier.
    pub event: &'static str,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Transport used for the request.
    pub transport: ServerTransport,
    /// Peer IP address when available.
    pub peer_ip: Option<String>,
    /// JSON-RPC method classification.
    pub method: McpMethod,
    /// Tool name when available (tools/call).
    pub tool: Option<ToolName>,
    /// Request outcome.
    pub outcome: McpOutcome,
    /// JSON-RPC error code when present.
    pub error_code: Option<i64>,
    /// Normalized error kind label.
    pub error_kind: Option<&'static str>,
    /// Request body size in bytes.
    pub request_bytes: usize,
    /// Response body size in bytes.
    pub response_bytes: usize,
    /// Client subject when provided.
    pub client_subject: Option<String>,
    /// Redaction classification for payload logging.
    pub redaction: &'static str,
}

/// Inputs required to construct an audit event.
pub struct McpAuditEventParams {
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Transport type used for the request.
    pub transport: ServerTransport,
    /// Peer IP address if known.
    pub peer_ip: Option<String>,
    /// JSON-RPC method classification.
    pub method: McpMethod,
    /// Tool name when available (tools/call).
    pub tool: Option<ToolName>,
    /// Request outcome.
    pub outcome: McpOutcome,
    /// JSON-RPC error code when present.
    pub error_code: Option<i64>,
    /// Normalized error kind label.
    pub error_kind: Option<&'static str>,
    /// Request body size in bytes.
    pub request_bytes: usize,
    /// Response body size in bytes.
    pub response_bytes: usize,
    /// Client subject when provided.
    pub client_subject: Option<String>,
    /// Redaction classification for payload logging.
    pub redaction: &'static str,
}

impl McpAuditEvent {
    /// Creates a new audit event with a consistent timestamp.
    #[must_use]
    pub fn new(params: McpAuditEventParams) -> Self {
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            event: "mcp_request",
            timestamp_ms,
            request_id: params.request_id,
            transport: params.transport,
            peer_ip: params.peer_ip,
            method: params.method,
            tool: params.tool,
            outcome: params.outcome,
            error_code: params.error_code,
            error_kind: params.error_kind,
            request_bytes: params.request_bytes,
            response_bytes: params.response_bytes,
            client_subject: params.client_subject,
            redaction: params.redaction,
        }
    }
}

// ============================================================================
// SECTION: Trait
// ============================================================================

/// Audit sink for MCP request events.
pub trait McpAuditSink: Send + Sync {
    /// Record an audit event.
    fn record(&self, event: &McpAuditEvent);
}

/// Audit sink that logs JSON lines to stderr.
pub struct McpStderrAuditSink;

impl McpAuditSink for McpStderrAuditSink {
    fn record(&self, event: &McpAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }
}

/// Audit sink that logs JSON lines to a file.
pub struct McpFileAuditSink {
    /// File handle used for append-only logging.
    file: Mutex<std::fs::File>,
}

impl McpFileAuditSink {
    /// Opens the audit log file in append mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened.
    pub fn new(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl McpAuditSink for McpFileAuditSink {
    fn record(&self, event: &McpAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event)
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(file, "{payload}");
            let _ = file.flush();
        }
    }
}

/// No-op audit sink.
pub struct McpNoopAuditSink;

impl McpAuditSink for McpNoopAuditSink {
    fn record(&self, _event: &McpAuditEvent) {}
}
