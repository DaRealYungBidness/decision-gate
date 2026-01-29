// decision-gate-mcp/tests/auth.rs
// ============================================================================
// Module: MCP Auth Tests
// Description: Unit tests for inbound MCP authn/authz policies.
// Purpose: Validate fail-closed behavior for local-only and bearer auth modes.
// Dependencies: decision-gate-mcp, decision-gate-contract
// ============================================================================

//! Auth policy tests for MCP tool calls.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions use unwrap for clarity."
)]

use std::net::IpAddr;

use decision_gate_contract::ToolName;
use decision_gate_mcp::DefaultToolAuthz;
use decision_gate_mcp::RequestContext;
use decision_gate_mcp::ToolAuthz;
use decision_gate_mcp::auth::AuthAction;
use decision_gate_mcp::auth::AuthAuditEvent;
use decision_gate_mcp::auth::AuthContext;
use decision_gate_mcp::auth::AuthError;
use decision_gate_mcp::auth::AuthMethod;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerTransport;
use serde_json::Value;

fn authorize_sync(
    authz: &DefaultToolAuthz,
    context: &RequestContext,
    action: AuthAction<'_>,
) -> Result<AuthContext, AuthError> {
    tokio::runtime::Runtime::new().expect("runtime").block_on(authz.authorize(context, action))
}

#[test]
fn local_only_allows_stdio() {
    let authz = DefaultToolAuthz::from_config(None);
    let context = RequestContext::stdio();
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_ok());
}

#[test]
fn local_only_rejects_remote_http() {
    let authz = DefaultToolAuthz::from_config(None);
    let context =
        RequestContext::http(ServerTransport::Http, Some(IpAddr::from([10, 0, 0, 1])), None, None);
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_err());
}

#[test]
fn bearer_auth_requires_token() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token-1".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let context =
        RequestContext::http(ServerTransport::Http, Some(IpAddr::from([127, 0, 0, 1])), None, None);
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_err());
}

#[test]
fn bearer_auth_accepts_valid_token() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token-1".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let context = RequestContext::http(
        ServerTransport::Http,
        Some(IpAddr::from([127, 0, 0, 1])),
        Some("Bearer token-1".to_string()),
        None,
    );
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_ok());
}

#[test]
fn tool_allowlist_denies_disallowed_tool() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token-1".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: vec!["scenario_define".to_string()],
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let context = RequestContext::http(
        ServerTransport::Http,
        Some(IpAddr::from([127, 0, 0, 1])),
        Some("Bearer token-1".to_string()),
        None,
    );
    let allowed = authorize_sync(&authz, &context, AuthAction::CallTool(&ToolName::ScenarioDefine));
    assert!(allowed.is_ok());
    let denied = authorize_sync(&authz, &context, AuthAction::CallTool(&ToolName::ScenarioStatus));
    assert!(denied.is_err());
}

#[test]
fn bearer_auth_rejects_invalid_scheme() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token-1".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let context = RequestContext::http(
        ServerTransport::Http,
        Some(IpAddr::from([127, 0, 0, 1])),
        Some("Basic token-1".to_string()),
        None,
    );
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_err());
}

#[test]
fn bearer_auth_rejects_oversized_header() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token-1".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let oversized = format!("Bearer {}", "a".repeat(9000));
    let context = RequestContext::http(
        ServerTransport::Http,
        Some(IpAddr::from([127, 0, 0, 1])),
        Some(oversized),
        None,
    );
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_err());
}

#[test]
fn mtls_requires_subject_header() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec!["CN=client".to_string()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let context =
        RequestContext::http(ServerTransport::Http, Some(IpAddr::from([127, 0, 0, 1])), None, None);
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_err());
}

#[test]
fn mtls_rejects_unlisted_subject() {
    let config = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec!["CN=client".to_string()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let authz = DefaultToolAuthz::from_config(Some(&config));
    let context = RequestContext::http(
        ServerTransport::Http,
        Some(IpAddr::from([127, 0, 0, 1])),
        None,
        Some("CN=other".to_string()),
    );
    let result = authorize_sync(&authz, &context, AuthAction::ListTools);
    assert!(result.is_err());
}

#[test]
fn audit_event_serializes_with_decision() {
    let context = RequestContext::stdio();
    let auth = AuthContext {
        method: AuthMethod::Local,
        subject: Some("stdio".to_string()),
        token_fingerprint: None,
    };
    let event = AuthAuditEvent::allowed(&context, AuthAction::ListTools, &auth);
    let payload = serde_json::to_value(&event).expect("serialize audit event");
    assert_eq!(payload.get("decision").and_then(Value::as_str), Some("allow"));

    let denied = AuthAuditEvent::denied(
        &context,
        AuthAction::ListTools,
        &decision_gate_mcp::auth::AuthError::Unauthenticated("missing".to_string()),
    );
    let payload = serde_json::to_value(&denied).expect("serialize audit event");
    assert_eq!(payload.get("decision").and_then(Value::as_str), Some("deny"));
    assert!(payload.get("reason").and_then(Value::as_str).is_some());
}
