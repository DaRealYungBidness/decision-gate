// system-tests/tests/suites/config_validation.rs
// ============================================================================
// Module: Config Validation Tests
// Description: Validate invalid config combinations fail closed.
// Purpose: Ensure unsafe configurations are rejected before startup.
// Dependencies: system-tests helpers
// ============================================================================

//! Configuration validation system tests.


use decision_gate_mcp::config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_mcp::config::NamespaceAuthorityMode;
use decision_gate_mcp::config::NamespaceMappingMode;
use decision_gate_mcp::config::ServerAuditConfig;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::namespace_authority_stub::spawn_namespace_authority_stub;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn dev_permissive_assetcore_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("dev_permissive_assetcore_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);

    let authority = spawn_namespace_authority_stub(vec![42]).await?;
    config.dev.permissive = true;
    config.namespace.authority.mode = NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore = Some(AssetCoreNamespaceAuthorityConfig {
        base_url: authority.base_url().to_string(),
        auth_token: None,
        connect_timeout_ms: 500,
        request_timeout_ms: 1_000,
        mapping: [("default".to_string(), 42)].into_iter().collect(),
        mapping_mode: NamespaceMappingMode::ExplicitMap,
    });

    let audit_path = reporter.artifacts().root().join("audit.log");
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some(audit_path.display().to_string()),
        log_precheck_payloads: false,
    };

    let err = spawn_mcp_server(config).await.err().ok_or_else(|| {
        "expected dev.permissive + assetcore config to fail validation".to_string()
    })?;
    if !err.contains("dev.permissive not allowed") {
        return Err(format!("unexpected error: {err}").into());
    }

    if audit_path.exists() {
        let contents = std::fs::read_to_string(&audit_path)?;
        if contents.contains("security_audit") {
            return Err("unexpected security_audit for invalid config".into());
        }
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["dev-permissive rejected with assetcore authority".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}
