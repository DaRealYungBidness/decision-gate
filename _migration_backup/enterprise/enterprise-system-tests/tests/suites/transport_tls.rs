//! Enterprise transport TLS system tests.
// enterprise/enterprise-system-tests/tests/suites/transport_tls.rs
// ============================================================================
// Module: Enterprise TLS/mTLS Tests
// Description: Validate TLS and mTLS enforcement for enterprise servers.
// Purpose: Ensure transport security is fail-closed.
// Dependencies: enterprise system-test helpers
// ============================================================================


use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use decision_gate_enterprise::config::EnterpriseConfig;
use decision_gate_enterprise::config::EnterpriseRunpackConfig;
use decision_gate_enterprise::config::EnterpriseStorageConfig;
use decision_gate_enterprise::config::EnterpriseUsageConfig;
use decision_gate_enterprise::config::UsageLedgerConfig;
use decision_gate_enterprise::config::UsageLedgerType;
use decision_gate_enterprise::tenant_authz::MappedTenantAuthorizer;
use decision_gate_enterprise::tenant_authz::TenantAuthzPolicy;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config_with_mtls_tls;
use helpers::harness::base_http_config_with_tls;
use helpers::harness::spawn_enterprise_server_from_configs;
use helpers::mcp_client::McpHttpClient;
use helpers::readiness::wait_for_server_ready;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn enterprise_tls_rejects_plaintext() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_tls_rejects_plaintext")?;

    let bind = allocate_bind_addr()?.to_string();
    let fixtures = tls_fixtures();
    let config = base_http_config_with_tls(&bind, &fixtures.server_cert, &fixtures.server_key);
    let enterprise_config = base_enterprise_config();
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(TenantAuthzPolicy::default()));

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let plaintext_url = server.base_url().replace("https://", "http://");
    let plaintext_client = McpHttpClient::new(plaintext_url, Duration::from_secs(5))?;
    let Err(err) = plaintext_client.list_tools().await else {
        return Err("expected plaintext rejection".into());
    };
    if err.is_empty() {
        return Err("expected plaintext rejection error".into());
    }

    let ca_pem = fs::read(&fixtures.ca_pem)?;
    let tls_client = McpHttpClient::new_with_tls(
        server.base_url().to_string(),
        Duration::from_secs(5),
        &ca_pem,
        None,
    )?;
    wait_for_server_ready(&tls_client, Duration::from_secs(5)).await?;

    reporter.artifacts().write_json("tool_transcript.json", &tls_client.transcript())?;
    reporter.finish(
        "pass",
        vec!["TLS-only endpoint rejects plaintext".to_string()],
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
async fn enterprise_mtls_subject_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("enterprise_mtls_subject_enforced")?;

    let bind = allocate_bind_addr()?.to_string();
    let fixtures = tls_fixtures();
    let mut config = base_http_config_with_mtls_tls(
        &bind,
        &fixtures.server_cert,
        &fixtures.server_key,
        &fixtures.ca_pem,
        true,
    );
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec!["CN=allowed".to_string()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });

    let enterprise_config = base_enterprise_config();
    let tenant_authorizer = Arc::new(MappedTenantAuthorizer::new(TenantAuthzPolicy::default()));

    let server = spawn_enterprise_server_from_configs(
        config,
        enterprise_config,
        tenant_authorizer,
        Arc::new(McpNoopAuditSink),
    )
    .await?;

    let ca_pem = fs::read(&fixtures.ca_pem)?;
    let identity = fs::read(&fixtures.client_identity)?;
    let denied_client = McpHttpClient::new_with_tls(
        server.base_url().to_string(),
        Duration::from_secs(5),
        &ca_pem,
        Some(&identity),
    )?
    .with_client_subject("CN=denied".to_string());
    let Err(err) = denied_client.list_tools().await else {
        return Err("expected mTLS subject denial".into());
    };
    if err.is_empty() {
        return Err("expected mTLS subject denial error".into());
    }

    let allowed_client = McpHttpClient::new_with_tls(
        server.base_url().to_string(),
        Duration::from_secs(5),
        &ca_pem,
        Some(&identity),
    )?
    .with_client_subject("CN=allowed".to_string());
    wait_for_server_ready(&allowed_client, Duration::from_secs(5)).await?;
    let tools = allowed_client.list_tools().await?;
    if tools.is_empty() {
        return Err("expected non-empty tool list".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &allowed_client.transcript())?;
    reporter.finish(
        "pass",
        vec!["mTLS subject enforcement verified".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    Ok(())
}

struct TlsFixtures {
    ca_pem: PathBuf,
    server_cert: PathBuf,
    server_key: PathBuf,
    client_identity: PathBuf,
}

fn tls_fixtures() -> TlsFixtures {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    TlsFixtures {
        ca_pem: root.join("ca.pem"),
        server_cert: root.join("server.pem"),
        server_key: root.join("server.key"),
        client_identity: root.join("client.identity.pem"),
    }
}

fn base_enterprise_config() -> EnterpriseConfig {
    EnterpriseConfig {
        storage: EnterpriseStorageConfig::default(),
        runpacks: EnterpriseRunpackConfig::default(),
        usage: EnterpriseUsageConfig {
            ledger: UsageLedgerConfig {
                ledger_type: UsageLedgerType::Memory,
                sqlite_path: None,
            },
            ..EnterpriseUsageConfig::default()
        },
        source_modified_at: None,
    }
}
