// system-tests/tests/suites/mcp_hardening.rs
// ============================================================================
// Module: MCP Hardening Tests
// Description: System tests for transport hardening and audit logging.
// Purpose: Validate rate limiting, TLS, and audit sinks end-to-end.
// Dependencies: system-tests helpers
// ============================================================================

//! MCP hardening system tests.


use std::fs;
use std::time::Duration;

use decision_gate_mcp::config::RateLimitConfig;
use decision_gate_mcp::config::ServerAuditConfig;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::base_http_config_with_mtls_tls;
use helpers::harness::base_http_config_with_tls;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;
use serde_json::Value;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn http_rate_limit_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_rate_limit_enforced")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.server.limits.rate_limit = Some(RateLimitConfig {
        max_requests: 2,
        window_ms: 60_000,
        max_entries: 64,
    });
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let _ = client.list_tools().await?;
    let Err(err) = client.list_tools().await else {
        return Err("expected rate limit".into());
    };
    if !err.contains("rate limit") {
        return Err(format!("expected rate limit error, got: {err}").into());
    }

    let transcript = client.transcript();
    let last = transcript.last().ok_or_else(|| "missing transcript entry".to_string())?;
    let code =
        last.response.get("error").and_then(|error| error.get("code")).and_then(Value::as_i64);
    if code != Some(-32071) {
        return Err(format!("expected rate limit code -32071, got {code:?}").into());
    }
    let kind = last
        .response
        .get("error")
        .and_then(|error| error.get("data"))
        .and_then(|data| data.get("kind"))
        .and_then(Value::as_str);
    if kind != Some("rate_limited") {
        return Err(format!("expected rate_limited kind, got {kind:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["rate limiting enforced for MCP HTTP".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_tls_handshake_success() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_tls_handshake_success")?;
    let bind = allocate_bind_addr()?.to_string();
    let fixtures = helpers::tls::generate_tls_fixtures()?;
    let config = base_http_config_with_tls(&bind, &fixtures.server_cert, &fixtures.server_key);
    let server = spawn_mcp_server(config).await?;

    let ca_pem = fs::read(&fixtures.ca_pem)?;
    let client = McpHttpClient::new_with_tls(
        server.base_url().to_string(),
        Duration::from_secs(5),
        &ca_pem,
        None,
    )?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;
    let tools = client.list_tools().await?;
    if tools.is_empty() {
        return Err("expected non-empty tools list".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["TLS handshake succeeds with test CA".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_mtls_client_cert_required() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_mtls_client_cert_required")?;
    let bind = allocate_bind_addr()?.to_string();
    let fixtures = helpers::tls::generate_tls_fixtures()?;
    let config = base_http_config_with_mtls_tls(
        &bind,
        &fixtures.server_cert,
        &fixtures.server_key,
        &fixtures.ca_pem,
        true,
    );
    let server = spawn_mcp_server(config).await?;

    let ca_pem = fs::read(&fixtures.ca_pem)?;
    let unauth = McpHttpClient::new_with_tls(
        server.base_url().to_string(),
        Duration::from_secs(5),
        &ca_pem,
        None,
    )?;
    let Err(err) = unauth.list_tools().await else {
        return Err("expected mtls rejection".into());
    };
    if err.is_empty() {
        return Err("expected mtls rejection error".into());
    }

    let identity = fs::read(&fixtures.client_identity)?;
    let auth_client = McpHttpClient::new_with_tls(
        server.base_url().to_string(),
        Duration::from_secs(5),
        &ca_pem,
        Some(&identity),
    )?;
    wait_for_server_ready(&auth_client, Duration::from_secs(5)).await?;
    let tools = auth_client.list_tools().await?;
    if tools.is_empty() {
        return Err("expected non-empty tools list".into());
    }

    let mut transcript = unauth.transcript();
    transcript.extend(auth_client.transcript());
    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["mTLS client certificates required for MCP HTTP".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn http_audit_log_written() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("http_audit_log_written")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    let audit_path = reporter.artifacts().root().join("audit.log");
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some(audit_path.display().to_string()),
        log_precheck_payloads: false,
    };
    let server = spawn_mcp_server(config).await?;

    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;
    let _ = client.list_tools().await?;

    let contents = fs::read_to_string(&audit_path)?;
    let line = contents.lines().next().unwrap_or_default();
    let payload: Value = serde_json::from_str(line)?;
    let event = payload.get("event").and_then(Value::as_str);
    if event != Some("mcp_request") {
        return Err(format!("expected audit event mcp_request, got {event:?}").into());
    }
    let redaction = payload.get("redaction").and_then(Value::as_str);
    if redaction != Some("full") {
        return Err(format!("expected audit redaction full, got {redaction:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["audit log records MCP requests".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "audit.log".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}
