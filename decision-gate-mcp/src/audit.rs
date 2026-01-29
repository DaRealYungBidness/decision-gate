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
use decision_gate_core::HashDigest;
use serde::Serialize;
use serde_json::Value;

use crate::config::RegistryAclAction;
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
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
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

/// Precheck audit event payload (hash-only by default).
#[derive(Debug, Clone, Serialize)]
pub struct PrecheckAuditEvent {
    /// Event identifier.
    pub event: &'static str,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tenant identifier.
    pub tenant_id: String,
    /// Namespace identifier.
    pub namespace_id: String,
    /// Scenario identifier (from request or spec).
    pub scenario_id: Option<String>,
    /// Stage identifier override (if provided).
    pub stage_id: Option<String>,
    /// Data shape schema identifier.
    pub schema_id: String,
    /// Data shape schema version.
    pub schema_version: String,
    /// Canonical hash of the precheck request.
    pub request_hash: HashDigest,
    /// Canonical hash of the precheck response.
    pub response_hash: HashDigest,
    /// Optional raw request payload (explicit opt-in only).
    pub request: Option<Value>,
    /// Optional raw response payload (explicit opt-in only).
    pub response: Option<Value>,
    /// Redaction classification for payload logging.
    pub redaction: &'static str,
}

/// Schema registry audit event payload.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryAuditEvent {
    /// Event identifier.
    pub event: &'static str,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tenant identifier.
    pub tenant_id: String,
    /// Namespace identifier.
    pub namespace_id: String,
    /// Registry action.
    pub action: RegistryAclAction,
    /// Whether access was allowed.
    pub allowed: bool,
    /// Decision reason label.
    pub reason: String,
    /// Principal identifier.
    pub principal_id: String,
    /// Principal roles.
    pub roles: Vec<String>,
    /// Policy class label when available.
    pub policy_class: Option<String>,
    /// Optional schema id.
    pub schema_id: Option<String>,
    /// Optional schema version.
    pub schema_version: Option<String>,
}

/// Tenant authorization audit event payload.
#[derive(Debug, Clone, Serialize)]
pub struct TenantAuthzEvent {
    /// Event identifier.
    pub event: &'static str,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tool name when available.
    pub tool: Option<ToolName>,
    /// Whether access was allowed.
    pub allowed: bool,
    /// Decision reason label.
    pub reason: String,
    /// Principal identifier.
    pub principal_id: String,
    /// Tenant identifier when provided.
    pub tenant_id: Option<String>,
    /// Namespace identifier when provided.
    pub namespace_id: Option<String>,
}

/// Usage audit event payload.
#[derive(Debug, Clone, Serialize)]
pub struct UsageAuditEvent {
    /// Event identifier.
    pub event: &'static str,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tool name when available.
    pub tool: Option<ToolName>,
    /// Tenant identifier when provided.
    pub tenant_id: Option<String>,
    /// Namespace identifier when provided.
    pub namespace_id: Option<String>,
    /// Principal identifier.
    pub principal_id: String,
    /// Usage metric label.
    pub metric: String,
    /// Units consumed.
    pub units: u64,
    /// Whether the request was allowed.
    pub allowed: bool,
    /// Decision reason label.
    pub reason: String,
}
/// Security posture audit event payload.
#[derive(Debug, Clone, Serialize)]
pub struct SecurityAuditEvent {
    /// Event identifier.
    pub event: &'static str,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Unsafe client correlation identifier when available.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier when available.
    pub server_correlation_id: Option<String>,
    /// Security event kind.
    pub kind: String,
    /// Optional message.
    pub message: Option<String>,
    /// Dev-permissive enabled.
    pub dev_permissive: bool,
    /// Namespace authority mode label.
    pub namespace_authority: String,
}

/// Inputs required to construct an audit event.
pub struct McpAuditEventParams {
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
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

/// Inputs required to construct a precheck audit event.
pub struct PrecheckAuditEventParams {
    /// Tenant identifier.
    pub tenant_id: String,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Namespace identifier.
    pub namespace_id: String,
    /// Scenario identifier (from request or spec).
    pub scenario_id: Option<String>,
    /// Stage identifier override (if provided).
    pub stage_id: Option<String>,
    /// Data shape schema identifier.
    pub schema_id: String,
    /// Data shape schema version.
    pub schema_version: String,
    /// Canonical hash of the precheck request.
    pub request_hash: HashDigest,
    /// Canonical hash of the precheck response.
    pub response_hash: HashDigest,
    /// Optional raw request payload (explicit opt-in only).
    pub request: Option<Value>,
    /// Optional raw response payload (explicit opt-in only).
    pub response: Option<Value>,
    /// Redaction classification for payload logging.
    pub redaction: &'static str,
}

/// Inputs required to construct a registry audit event.
pub struct RegistryAuditEventParams {
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tenant identifier.
    pub tenant_id: String,
    /// Namespace identifier.
    pub namespace_id: String,
    /// Registry action.
    pub action: RegistryAclAction,
    /// Whether access was allowed.
    pub allowed: bool,
    /// Decision reason label.
    pub reason: String,
    /// Principal identifier.
    pub principal_id: String,
    /// Principal roles.
    pub roles: Vec<String>,
    /// Policy class label when available.
    pub policy_class: Option<String>,
    /// Optional schema id.
    pub schema_id: Option<String>,
    /// Optional schema version.
    pub schema_version: Option<String>,
}

/// Inputs required to construct a tenant authorization audit event.
pub struct TenantAuthzEventParams {
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tool name when available.
    pub tool: Option<ToolName>,
    /// Whether access was allowed.
    pub allowed: bool,
    /// Decision reason label.
    pub reason: String,
    /// Principal identifier.
    pub principal_id: String,
    /// Tenant identifier when provided.
    pub tenant_id: Option<String>,
    /// Namespace identifier when provided.
    pub namespace_id: Option<String>,
}

/// Inputs required to construct a usage audit event.
pub struct UsageAuditEventParams {
    /// Request identifier when provided.
    pub request_id: Option<String>,
    /// Unsafe client correlation identifier when provided.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: String,
    /// Tool name when available.
    pub tool: Option<ToolName>,
    /// Tenant identifier when provided.
    pub tenant_id: Option<String>,
    /// Namespace identifier when provided.
    pub namespace_id: Option<String>,
    /// Principal identifier.
    pub principal_id: String,
    /// Usage metric label.
    pub metric: String,
    /// Units consumed.
    pub units: u64,
    /// Whether the request was allowed.
    pub allowed: bool,
    /// Decision reason label.
    pub reason: String,
}
/// Inputs required to construct a security audit event.
pub struct SecurityAuditEventParams {
    /// Security event kind.
    pub kind: String,
    /// Unsafe client correlation identifier when available.
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier when available.
    pub server_correlation_id: Option<String>,
    /// Optional message.
    pub message: Option<String>,
    /// Dev-permissive enabled.
    pub dev_permissive: bool,
    /// Namespace authority mode label.
    pub namespace_authority: String,
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
            unsafe_client_correlation_id: params.unsafe_client_correlation_id,
            server_correlation_id: params.server_correlation_id,
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

impl PrecheckAuditEvent {
    /// Creates a new precheck audit event with a consistent timestamp.
    #[must_use]
    pub fn new(params: PrecheckAuditEventParams) -> Self {
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            event: "precheck_audit",
            timestamp_ms,
            unsafe_client_correlation_id: params.unsafe_client_correlation_id,
            server_correlation_id: params.server_correlation_id,
            tenant_id: params.tenant_id,
            namespace_id: params.namespace_id,
            scenario_id: params.scenario_id,
            stage_id: params.stage_id,
            schema_id: params.schema_id,
            schema_version: params.schema_version,
            request_hash: params.request_hash,
            response_hash: params.response_hash,
            request: params.request,
            response: params.response,
            redaction: params.redaction,
        }
    }
}

impl RegistryAuditEvent {
    /// Creates a new registry audit event with a consistent timestamp.
    #[must_use]
    pub fn new(params: RegistryAuditEventParams) -> Self {
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            event: "registry_audit",
            timestamp_ms,
            request_id: params.request_id,
            unsafe_client_correlation_id: params.unsafe_client_correlation_id,
            server_correlation_id: params.server_correlation_id,
            tenant_id: params.tenant_id,
            namespace_id: params.namespace_id,
            action: params.action,
            allowed: params.allowed,
            reason: params.reason,
            principal_id: params.principal_id,
            roles: params.roles,
            policy_class: params.policy_class,
            schema_id: params.schema_id,
            schema_version: params.schema_version,
        }
    }
}

impl TenantAuthzEvent {
    /// Creates a new tenant authorization audit event with a consistent timestamp.
    #[must_use]
    pub fn new(params: TenantAuthzEventParams) -> Self {
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            event: "tenant_authz",
            timestamp_ms,
            request_id: params.request_id,
            unsafe_client_correlation_id: params.unsafe_client_correlation_id,
            server_correlation_id: params.server_correlation_id,
            tool: params.tool,
            allowed: params.allowed,
            reason: params.reason,
            principal_id: params.principal_id,
            tenant_id: params.tenant_id,
            namespace_id: params.namespace_id,
        }
    }
}

impl UsageAuditEvent {
    /// Creates a new usage audit event with a consistent timestamp.
    #[must_use]
    pub fn new(params: UsageAuditEventParams) -> Self {
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            event: "usage_audit",
            timestamp_ms,
            request_id: params.request_id,
            unsafe_client_correlation_id: params.unsafe_client_correlation_id,
            server_correlation_id: params.server_correlation_id,
            tool: params.tool,
            tenant_id: params.tenant_id,
            namespace_id: params.namespace_id,
            principal_id: params.principal_id,
            metric: params.metric,
            units: params.units,
            allowed: params.allowed,
            reason: params.reason,
        }
    }
}
impl SecurityAuditEvent {
    /// Creates a new security audit event with a consistent timestamp.
    #[must_use]
    pub fn new(params: SecurityAuditEventParams) -> Self {
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            event: "security_audit",
            timestamp_ms,
            kind: params.kind,
            unsafe_client_correlation_id: params.unsafe_client_correlation_id,
            server_correlation_id: params.server_correlation_id,
            message: params.message,
            dev_permissive: params.dev_permissive,
            namespace_authority: params.namespace_authority,
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

    /// Record a precheck audit event.
    fn record_precheck(&self, _event: &PrecheckAuditEvent) {}

    /// Record a registry audit event.
    fn record_registry(&self, _event: &RegistryAuditEvent) {}

    /// Record a tenant authorization audit event.
    fn record_tenant_authz(&self, _event: &TenantAuthzEvent) {}

    /// Record a usage audit event.
    fn record_usage(&self, _event: &UsageAuditEvent) {}

    /// Record a security posture audit event.
    fn record_security(&self, _event: &SecurityAuditEvent) {}
}

/// Audit sink that logs JSON lines to stderr.
pub struct McpStderrAuditSink;

impl McpAuditSink for McpStderrAuditSink {
    fn record(&self, event: &McpAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }

    fn record_precheck(&self, event: &PrecheckAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }

    fn record_registry(&self, event: &RegistryAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }

    fn record_tenant_authz(&self, event: &TenantAuthzEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }

    fn record_usage(&self, event: &UsageAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }

    fn record_security(&self, event: &SecurityAuditEvent) {
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

    fn record_precheck(&self, event: &PrecheckAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event)
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(file, "{payload}");
            let _ = file.flush();
        }
    }

    fn record_registry(&self, event: &RegistryAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event)
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(file, "{payload}");
            let _ = file.flush();
        }
    }

    fn record_tenant_authz(&self, event: &TenantAuthzEvent) {
        if let Ok(payload) = serde_json::to_string(event)
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(file, "{payload}");
            let _ = file.flush();
        }
    }

    fn record_usage(&self, event: &UsageAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event)
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(file, "{payload}");
            let _ = file.flush();
        }
    }

    fn record_security(&self, event: &SecurityAuditEvent) {
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

    fn record_precheck(&self, _event: &PrecheckAuditEvent) {}

    fn record_registry(&self, _event: &RegistryAuditEvent) {}

    fn record_tenant_authz(&self, _event: &TenantAuthzEvent) {}

    fn record_usage(&self, _event: &UsageAuditEvent) {}

    fn record_security(&self, _event: &SecurityAuditEvent) {}
}
