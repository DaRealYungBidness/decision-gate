//! Enterprise stdio server binary for system tests.
// enterprise-system-tests/src/bin/decision_gate_enterprise_stdio_server.rs
// ============================================================================
// Module: Decision Gate Enterprise Stdio Server
// Description: MCP stdio server runner for enterprise system-tests.
// Purpose: Provide a dedicated enterprise stdio server binary for tests.
// Dependencies: decision-gate-enterprise, decision-gate-mcp, tokio
// ============================================================================

use std::io::Write;
use std::sync::Arc;

use decision_gate_enterprise::config::EnterpriseConfig;
use decision_gate_enterprise::server::build_enterprise_server_from_configs;
use decision_gate_enterprise::tenant_authz::MappedTenantAuthorizer;
use decision_gate_enterprise::tenant_authz::TenantAuthzPolicy;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::NoopMetrics;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = match DecisionGateConfig::load(None) {
        Ok(config) => config,
        Err(err) => {
            write_stderr_line(&format!(
                "decision-gate-enterprise-stdio: config load failed: {err}"
            ));
            std::process::exit(1);
        }
    };

    let enterprise_config = match EnterpriseConfig::load(None) {
        Ok(config) => config,
        Err(err) => {
            write_stderr_line(&format!(
                "decision-gate-enterprise-stdio: enterprise config load failed: {err}"
            ));
            std::process::exit(1);
        }
    };

    let policy = std::env::var("DECISION_GATE_ENTERPRISE_TENANT_POLICY").map_or_else(
        |_| TenantAuthzPolicy::default(),
        |payload| match serde_json::from_str::<TenantAuthzPolicy>(&payload) {
            Ok(policy) => policy,
            Err(err) => {
                write_stderr_line(&format!(
                    "decision-gate-enterprise-stdio: tenant policy parse failed: {err}"
                ));
                std::process::exit(1);
            }
        },
    );

    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(policy));
    let audit_sink = Arc::new(McpNoopAuditSink);
    let metrics = Arc::new(NoopMetrics);

    let server = match tokio::task::spawn_blocking(move || {
        build_enterprise_server_from_configs(
            config,
            &enterprise_config,
            tenant_authorizer,
            audit_sink,
            metrics,
        )
    })
    .await
    {
        Ok(result) => match result {
            Ok(server) => server,
            Err(err) => {
                write_stderr_line(&format!("decision-gate-enterprise-stdio: init failed: {err}"));
                std::process::exit(1);
            }
        },
        Err(err) => {
            write_stderr_line(&format!("decision-gate-enterprise-stdio: init join failed: {err}"));
            std::process::exit(1);
        }
    };

    if let Err(err) = server.serve().await {
        write_stderr_line(&format!("decision-gate-enterprise-stdio: server failed: {err}"));
        std::process::exit(1);
    }
}

/// Writes a single line to stderr for startup failures.
fn write_stderr_line(message: &str) {
    let mut stderr = std::io::stderr();
    let _ = writeln!(&mut stderr, "{message}");
}
