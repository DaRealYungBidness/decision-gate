// decision-gate-mcp/src/auth.rs
// ============================================================================
// Module: MCP Authn/Authz
// Description: Authentication and authorization enforcement for MCP tool calls.
// Purpose: Provide strict, fail-closed auth policies for MCP tool requests.
// Dependencies: decision-gate-contract, decision-gate-core, serde
// ============================================================================

//! ## Overview
//! This module defines the authn/authz interfaces for MCP tool calls and
//! provides default policies for local-only, bearer token, and mTLS-subject
//! enforcement. All decisions are fail-closed and emit audit events.
//! Security posture: auth decisions are a trust boundary and must fail closed
//! on any invalid input; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeSet;
use std::io::Write;
use std::net::IpAddr;

use async_trait::async_trait;
use decision_gate_contract::ToolName;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use serde::Serialize;
use thiserror::Error;

use crate::config::ServerAuthConfig;
use crate::config::ServerAuthMode;
use crate::config::ServerTransport;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum allowed Authorization header size in bytes.
const MAX_AUTH_HEADER_BYTES: usize = 8 * 1024;
/// Default auth realm used for RFC 6750 challenges.
const DEFAULT_AUTH_REALM: &str = "decision-gate";

// ============================================================================
// SECTION: Auth Challenges
// ============================================================================

/// HTTP auth challenge header value (WWW-Authenticate).
///
/// # Invariants
/// - `header_value` is a complete serialized header value.
#[derive(Debug, Clone)]
pub struct AuthChallenge {
    /// Serialized WWW-Authenticate header value.
    header_value: String,
}

impl AuthChallenge {
    /// Builds a bearer challenge with a realm.
    #[must_use]
    pub fn bearer(realm: &str) -> Self {
        Self {
            header_value: format!("Bearer realm=\"{realm}\""),
        }
    }

    /// Returns the serialized header value.
    #[must_use]
    pub fn header_value(&self) -> &str {
        &self.header_value
    }
}

/// Returns the default auth challenge for a configured auth mode.
#[must_use]
pub fn auth_challenge_for_mode(mode: ServerAuthMode) -> Option<AuthChallenge> {
    match mode {
        ServerAuthMode::BearerToken => Some(AuthChallenge::bearer(DEFAULT_AUTH_REALM)),
        ServerAuthMode::LocalOnly | ServerAuthMode::Mtls => None,
    }
}

// ============================================================================
// SECTION: Request Context
// ============================================================================

/// Per-request context used for auth decisions.
///
/// # Invariants
/// - Fields reflect untrusted request metadata as received by the server.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Transport used by the caller.
    pub transport: ServerTransport,
    /// Peer IP address when available.
    pub peer_ip: Option<IpAddr>,
    /// Authorization header value (HTTP/SSE).
    pub auth_header: Option<String>,
    /// Client subject asserted by a trusted mTLS proxy.
    pub client_subject: Option<String>,
    /// Unsafe client correlation identifier (sanitized).
    pub unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier.
    pub server_correlation_id: Option<String>,
    /// Optional request identifier for auditing.
    pub request_id: Option<String>,
}

impl RequestContext {
    /// Builds a stdio request context.
    #[must_use]
    pub const fn stdio() -> Self {
        Self {
            transport: ServerTransport::Stdio,
            peer_ip: None,
            auth_header: None,
            client_subject: None,
            unsafe_client_correlation_id: None,
            server_correlation_id: None,
            request_id: None,
        }
    }

    /// Builds an HTTP/SSE request context without correlation identifiers.
    #[must_use]
    pub const fn http(
        transport: ServerTransport,
        peer_ip: Option<IpAddr>,
        auth_header: Option<String>,
        client_subject: Option<String>,
    ) -> Self {
        Self {
            transport,
            peer_ip,
            auth_header,
            client_subject,
            unsafe_client_correlation_id: None,
            server_correlation_id: None,
            request_id: None,
        }
    }

    /// Builds an HTTP/SSE request context with correlation identifiers.
    #[must_use]
    pub const fn http_with_correlation(
        transport: ServerTransport,
        peer_ip: Option<IpAddr>,
        auth_header: Option<String>,
        client_subject: Option<String>,
        unsafe_client_correlation_id: Option<String>,
        server_correlation_id: Option<String>,
    ) -> Self {
        Self {
            transport,
            peer_ip,
            auth_header,
            client_subject,
            unsafe_client_correlation_id,
            server_correlation_id,
            request_id: None,
        }
    }

    /// Returns a copy with the request identifier set.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Returns a copy with the server correlation identifier set.
    #[must_use]
    pub fn with_server_correlation_id(mut self, server_correlation_id: impl Into<String>) -> Self {
        self.server_correlation_id = Some(server_correlation_id.into());
        self
    }

    /// Returns true when the peer IP is loopback.
    #[must_use]
    pub fn peer_is_loopback(&self) -> bool {
        self.peer_ip.is_some_and(|ip| ip.is_loopback())
    }
}

// ============================================================================
// SECTION: Auth Context
// ============================================================================

/// Authenticated caller context.
///
/// # Invariants
/// - `method` is the authoritative authn method for the request.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authentication method.
    pub method: AuthMethod,
    /// Optional subject identifier.
    pub subject: Option<String>,
    /// Token fingerprint for bearer auth (hashed).
    pub token_fingerprint: Option<String>,
}

impl AuthContext {
    /// Returns a stable label for the authentication method.
    const fn method_label(&self) -> &'static str {
        match self.method {
            AuthMethod::Local => "local",
            AuthMethod::BearerToken => "bearer_token",
            AuthMethod::MtlsSubject => "mtls_subject",
        }
    }

    /// Returns a stable principal identifier string for ACL mapping.
    #[must_use]
    pub fn principal_id(&self) -> String {
        if let Some(subject) = &self.subject {
            return subject.clone();
        }
        if let Some(fingerprint) = &self.token_fingerprint {
            return format!("token:{fingerprint}");
        }
        match self.method {
            AuthMethod::Local => "local".to_string(),
            AuthMethod::BearerToken => "token:unknown".to_string(),
            AuthMethod::MtlsSubject => "mtls:unknown".to_string(),
        }
    }
}

/// Authentication method used for the request.
///
/// # Invariants
/// - Variants are stable for audit labeling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// Local-only loopback or stdio access.
    Local,
    /// Bearer token authentication.
    BearerToken,
    /// mTLS subject authentication via trusted proxy header.
    MtlsSubject,
}

/// Authz action for MCP requests.
///
/// # Invariants
/// - Variants are stable for audit labeling.
#[derive(Debug, Clone, Copy)]
pub enum AuthAction<'a> {
    /// List tools action.
    ListTools,
    /// Tool call action.
    CallTool(&'a ToolName),
}

impl AuthAction<'_> {
    /// Returns a stable label for the requested action.
    fn label(self) -> String {
        match self {
            AuthAction::ListTools => "tools/list".to_string(),
            AuthAction::CallTool(tool) => tool.as_str().to_string(),
        }
    }
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Authentication or authorization errors.
///
/// # Invariants
/// - Variants are stable for error classification.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Missing or invalid authentication.
    #[error("unauthenticated: {0}")]
    Unauthenticated(String),
    /// Caller is authenticated but not authorized.
    #[error("unauthorized: {0}")]
    Unauthorized(String),
}

// ============================================================================
// SECTION: Traits
// ============================================================================

/// Authn/authz interface for MCP tool calls.
#[async_trait]
pub trait ToolAuthz: Send + Sync {
    /// Authorize a tool request. Returns an authenticated context on success.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] if the caller is unauthenticated or unauthorized.
    async fn authorize(
        &self,
        ctx: &RequestContext,
        action: AuthAction<'_>,
    ) -> Result<AuthContext, AuthError>;
}

/// Audit sink for auth decisions.
pub trait AuthAuditSink: Send + Sync {
    /// Record an auth audit event.
    fn record(&self, event: &AuthAuditEvent);
}

// ============================================================================
// SECTION: Default Policies
// ============================================================================

/// Default authz implementation derived from server config.
///
/// # Invariants
/// - Behavior is fully determined by the stored configuration.
pub struct DefaultToolAuthz {
    /// Configured auth mode.
    mode: ServerAuthMode,
    /// Allowed bearer tokens.
    bearer_tokens: BTreeSet<String>,
    /// Allowed mTLS subject names.
    mtls_subjects: BTreeSet<String>,
    /// Optional tool allowlist.
    allowed_tools: Option<BTreeSet<ToolName>>,
}

impl DefaultToolAuthz {
    /// Builds a default authz policy from server auth configuration.
    #[must_use]
    pub fn from_config(config: Option<&ServerAuthConfig>) -> Self {
        let mode = config.map_or(ServerAuthMode::LocalOnly, |cfg| cfg.mode);
        let bearer_tokens =
            config.map(|cfg| cfg.bearer_tokens.iter().cloned().collect()).unwrap_or_default();
        let mtls_subjects =
            config.map(|cfg| cfg.mtls_subjects.iter().cloned().collect()).unwrap_or_default();
        let allowed_tools = config.and_then(|cfg| {
            if cfg.allowed_tools.is_empty() {
                return None;
            }
            let mut parsed = BTreeSet::new();
            for name in &cfg.allowed_tools {
                if let Some(tool) = ToolName::parse(name) {
                    parsed.insert(tool);
                } else {
                    // Fail closed if a tool name cannot be parsed.
                    return Some(BTreeSet::new());
                }
            }
            Some(parsed)
        });
        Self {
            mode,
            bearer_tokens,
            mtls_subjects,
            allowed_tools,
        }
    }

    /// Returns the configured auth mode.
    #[must_use]
    pub const fn mode(&self) -> ServerAuthMode {
        self.mode
    }
}

#[async_trait]
impl ToolAuthz for DefaultToolAuthz {
    async fn authorize(
        &self,
        ctx: &RequestContext,
        action: AuthAction<'_>,
    ) -> Result<AuthContext, AuthError> {
        let mut auth = match self.mode {
            ServerAuthMode::LocalOnly => authorize_local_only(ctx)?,
            ServerAuthMode::BearerToken => authorize_bearer(ctx, &self.bearer_tokens)?,
            ServerAuthMode::Mtls => authorize_mtls(ctx, &self.mtls_subjects)?,
        };

        if let AuthAction::CallTool(tool) = action
            && let Some(allowed) = &self.allowed_tools
            && !allowed.contains(tool)
        {
            return Err(AuthError::Unauthorized("tool not authorized".to_string()));
        }

        if auth.subject.is_none() && matches!(auth.method, AuthMethod::Local) {
            auth.subject = Some(match ctx.transport {
                ServerTransport::Stdio => "stdio".to_string(),
                _ => "loopback".to_string(),
            });
        }

        Ok(auth)
    }
}

// ============================================================================
// SECTION: Audit Events
// ============================================================================

/// Auth audit event payload.
///
/// # Invariants
/// - Payload fields are derived from request metadata and are redacted.
#[derive(Debug, Serialize)]
pub struct AuthAuditEvent {
    /// Event identifier.
    event: &'static str,
    /// Decision outcome.
    decision: &'static str,
    /// MCP action name.
    action: String,
    /// Transport label.
    transport: &'static str,
    /// Caller IP address (if available).
    peer_ip: Option<String>,
    /// Auth method label.
    auth_method: Option<&'static str>,
    /// Caller subject or identity label.
    subject: Option<String>,
    /// Bearer token fingerprint (sha256).
    token_fingerprint: Option<String>,
    /// Failure reason (for deny events).
    reason: Option<String>,
    /// Request identifier (if provided).
    request_id: Option<String>,
    /// Unsafe client correlation identifier (if provided).
    unsafe_client_correlation_id: Option<String>,
    /// Server-generated correlation identifier (if provided).
    server_correlation_id: Option<String>,
}

impl AuthAuditEvent {
    /// Builds an allow event.
    #[must_use]
    pub fn allowed(ctx: &RequestContext, action: AuthAction<'_>, auth: &AuthContext) -> Self {
        Self {
            event: "mcp_tool_authz",
            decision: "allow",
            action: action.label(),
            transport: transport_label(ctx.transport),
            peer_ip: ctx.peer_ip.map(|ip| ip.to_string()),
            auth_method: Some(auth.method_label()),
            subject: auth.subject.clone(),
            token_fingerprint: auth.token_fingerprint.clone(),
            reason: None,
            request_id: ctx.request_id.clone(),
            unsafe_client_correlation_id: ctx.unsafe_client_correlation_id.clone(),
            server_correlation_id: ctx.server_correlation_id.clone(),
        }
    }

    /// Builds a deny event.
    #[must_use]
    pub fn denied(ctx: &RequestContext, action: AuthAction<'_>, error: &AuthError) -> Self {
        Self {
            event: "mcp_tool_authz",
            decision: "deny",
            action: action.label(),
            transport: transport_label(ctx.transport),
            peer_ip: ctx.peer_ip.map(|ip| ip.to_string()),
            auth_method: None,
            subject: None,
            token_fingerprint: None,
            reason: Some(error.to_string()),
            request_id: ctx.request_id.clone(),
            unsafe_client_correlation_id: ctx.unsafe_client_correlation_id.clone(),
            server_correlation_id: ctx.server_correlation_id.clone(),
        }
    }
}

/// Audit sink that logs JSON lines to stderr.
///
/// # Invariants
/// - Audit events are written to stderr in JSON form.
pub struct StderrAuditSink;

impl AuthAuditSink for StderrAuditSink {
    fn record(&self, event: &AuthAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            let _ = writeln!(std::io::stderr(), "{payload}");
        }
    }
}

/// No-op audit sink for tests.
///
/// # Invariants
/// - Audit events are intentionally discarded.
pub struct NoopAuditSink;

impl AuthAuditSink for NoopAuditSink {
    fn record(&self, _event: &AuthAuditEvent) {}
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Returns a stable label for the server transport.
const fn transport_label(transport: ServerTransport) -> &'static str {
    match transport {
        ServerTransport::Stdio => "stdio",
        ServerTransport::Http => "http",
        ServerTransport::Sse => "sse",
    }
}

/// Authorizes a local-only request based on the transport and peer IP.
fn authorize_local_only(ctx: &RequestContext) -> Result<AuthContext, AuthError> {
    match ctx.transport {
        ServerTransport::Stdio => Ok(AuthContext {
            method: AuthMethod::Local,
            subject: Some("stdio".to_string()),
            token_fingerprint: None,
        }),
        ServerTransport::Http | ServerTransport::Sse => {
            if ctx.peer_is_loopback() {
                Ok(AuthContext {
                    method: AuthMethod::Local,
                    subject: Some("loopback".to_string()),
                    token_fingerprint: None,
                })
            } else {
                Err(AuthError::Unauthenticated(
                    "local-only mode requires loopback access".to_string(),
                ))
            }
        }
    }
}

/// Authorizes a bearer token request against the configured token set.
fn authorize_bearer(
    ctx: &RequestContext,
    tokens: &BTreeSet<String>,
) -> Result<AuthContext, AuthError> {
    let token = parse_bearer_token(ctx.auth_header.as_deref())?;
    if !tokens.contains(&token) {
        return Err(AuthError::Unauthenticated("invalid bearer token".to_string()));
    }
    let digest = hash_bytes(HashAlgorithm::Sha256, token.as_bytes());
    Ok(AuthContext {
        method: AuthMethod::BearerToken,
        subject: None,
        token_fingerprint: Some(digest.value),
    })
}

/// Authorizes an mTLS subject request against the configured subject set.
fn authorize_mtls(
    ctx: &RequestContext,
    subjects: &BTreeSet<String>,
) -> Result<AuthContext, AuthError> {
    let subject = ctx
        .client_subject
        .as_deref()
        .ok_or_else(|| AuthError::Unauthenticated("missing mTLS client subject".to_string()))?;
    if !subjects.is_empty() && !subjects.contains(subject) {
        return Err(AuthError::Unauthorized("client subject not authorized".to_string()));
    }
    Ok(AuthContext {
        method: AuthMethod::MtlsSubject,
        subject: Some(subject.to_string()),
        token_fingerprint: None,
    })
}

/// Parses and validates a bearer token from an Authorization header.
pub(crate) fn parse_bearer_token(auth_header: Option<&str>) -> Result<String, AuthError> {
    let header = auth_header
        .ok_or_else(|| AuthError::Unauthenticated("missing authorization".to_string()))?;
    if header.len() > MAX_AUTH_HEADER_BYTES {
        return Err(AuthError::Unauthenticated("authorization header too large".to_string()));
    }
    let mut parts = header.trim().splitn(2, ' ');
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default().trim();
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(AuthError::Unauthenticated("invalid authorization header".to_string()));
    }
    Ok(token.to_string())
}
