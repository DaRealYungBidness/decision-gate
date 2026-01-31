// system-tests/tests/suites/tool_visibility.rs
// ============================================================================
// Module: Tool Visibility Tests
// Description: System tests for tool visibility allowlist/denylist behavior.
// Purpose: Ensure tools/list and tools/call obey visibility rules and auth separation.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Tool visibility system tests.

use std::time::Duration;

use decision_gate_contract::ToolName;
use decision_gate_mcp::config::ToolVisibilityMode;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use serde_json::json;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn server_tools_visibility_filtering() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("server_tools_visibility_filtering")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.server.tools.mode = ToolVisibilityMode::Filter;
    config.server.tools.allowlist =
        vec!["scenario_define".to_string(), "decision_gate_docs_search".to_string()];
    config.server.tools.denylist = vec!["scenario_define".to_string()];
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if tools.iter().any(|tool| tool.name == ToolName::ScenarioDefine) {
        return Err("scenario_define should be hidden by denylist".into());
    }
    if !tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("docs search should remain visible when allowlisted".into());
    }

    let Err(err) = client.call_tool("scenario_define", json!({})).await else {
        return Err("expected hidden tool call to fail".into());
    };
    if !err.contains("unknown tool") {
        return Err(format!("unexpected hidden tool error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["tool visibility filtering enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    server.shutdown().await;
    drop(reporter);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn server_tools_visibility_defaults_and_auth_separation()
-> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("server_tools_visibility_defaults_and_auth_separation")?;
    let mut transcript = Vec::new();

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.server.tools.denylist = vec!["scenario_define".to_string()];
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if tools.iter().any(|tool| tool.name == ToolName::ScenarioDefine) {
        return Err("scenario_define should be hidden by denylist".into());
    }
    if !tools.iter().any(|tool| tool.name == ToolName::ScenarioStart) {
        return Err("allowlist empty should still expose other tools".into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    if let Some(auth) = config.server.auth.as_mut() {
        auth.allowed_tools = vec!["scenario_define".to_string()];
    }
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if !tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("docs search should remain visible with auth allowlist".into());
    }
    let Err(err) = client
        .call_tool(
            "decision_gate_docs_search",
            json!({
                "query": "trust lanes"
            }),
        )
        .await
    else {
        return Err("expected auth allowlist to block docs search call".into());
    };
    if !err.contains("unauthorized") {
        return Err(format!("unexpected auth allowlist error: {err}").into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["tool visibility defaults and auth separation enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
