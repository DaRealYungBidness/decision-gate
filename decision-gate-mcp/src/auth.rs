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

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeSet;
use std::net::IpAddr;

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

const MAX_AUTH_HEADER_BYTES: usize = 8 * 1024;

// ============================================================================
// SECTION: Request Context
// ============================================================================

/// Per-request context used for auth decisions.
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
    /// Optional request identifier for auditing.
    pub request_id: Option<String>,
}

impl RequestContext {
    /// Builds a stdio request context.
    #[must_use]
    pub fn stdio() -> Self {
        Self {
            transport: ServerTransport::Stdio,
            peer_ip: None,
            auth_header: None,
            client_subject: None,
            request_id: None,
        }
    }

    /// Builds an HTTP/SSE request context.
    #[must_use]
    pub fn http(
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
            request_id: None,
        }
    }

    /// Returns a copy with the request identifier set.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Returns true when the peer IP is loopback.
    #[must_use]
    pub fn peer_is_loopback(&self) -> bool {
        self.peer_ip.map_or(false, |ip| ip.is_loopback())
    }
}

// ============================================================================
// SECTION: Auth Context
// ============================================================================

/// Authenticated caller context.
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
    fn method_label(&self) -> &'static str {
        match self.method {
            AuthMethod::Local => "local",
            AuthMethod::BearerToken => "bearer_token",
            AuthMethod::MtlsSubject => "mtls_subject",
        }
    }
}

/// Authentication method used for the request.
#[derive(Debug, Clone, Copy)]
pub enum AuthMethod {
    /// Local-only loopback or stdio access.
    Local,
    /// Bearer token authentication.
    BearerToken,
    /// mTLS subject authentication via trusted proxy header.
    MtlsSubject,
}

/// Authz action for MCP requests.
#[derive(Debug, Clone, Copy)]
pub enum AuthAction<'a> {
    /// List tools action.
    ListTools,
    /// Tool call action.
    CallTool(&'a ToolName),
}

impl AuthAction<'_> {
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
pub trait ToolAuthz: Send + Sync {
    /// Authorize a tool request. Returns an authenticated context on success.
    fn authorize(
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
pub struct DefaultToolAuthz {
    mode: ServerAuthMode,
    bearer_tokens: BTreeSet<String>,
    mtls_subjects: BTreeSet<String>,
    allowed_tools: Option<BTreeSet<ToolName>>,
}

impl DefaultToolAuthz {
    /// Builds a default authz policy from server auth configuration.
    #[must_use]
    pub fn from_config(config: Option<&ServerAuthConfig>) -> Self {
        let mode = config.map(|cfg| cfg.mode).unwrap_or(ServerAuthMode::LocalOnly);
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

impl ToolAuthz for DefaultToolAuthz {
    fn authorize(
        &self,
        ctx: &RequestContext,
        action: AuthAction<'_>,
    ) -> Result<AuthContext, AuthError> {
        let mut auth = match self.mode {
            ServerAuthMode::LocalOnly => authorize_local_only(ctx)?,
            ServerAuthMode::BearerToken => authorize_bearer(ctx, &self.bearer_tokens)?,
            ServerAuthMode::Mtls => authorize_mtls(ctx, &self.mtls_subjects)?,
        };

        if let AuthAction::CallTool(tool) = action {
            if let Some(allowed) = &self.allowed_tools {
                if !allowed.contains(tool) {
                    return Err(AuthError::Unauthorized("tool not authorized".to_string()));
                }
            }
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
        }
    }
}

/// Audit sink that logs JSON lines to stderr.
pub struct StderrAuditSink;

impl AuthAuditSink for StderrAuditSink {
    fn record(&self, event: &AuthAuditEvent) {
        if let Ok(payload) = serde_json::to_string(event) {
            eprintln!("{payload}");
        }
    }
}

/// No-op audit sink for tests.
pub struct NoopAuditSink;

impl AuthAuditSink for NoopAuditSink {
    fn record(&self, _event: &AuthAuditEvent) {}
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn transport_label(transport: ServerTransport) -> &'static str {
    match transport {
        ServerTransport::Stdio => "stdio",
        ServerTransport::Http => "http",
        ServerTransport::Sse => "sse",
    }
}

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

fn parse_bearer_token(auth_header: Option<&str>) -> Result<String, AuthError> {
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
