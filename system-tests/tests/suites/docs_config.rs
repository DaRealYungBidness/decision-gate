// system-tests/tests/suites/docs_config.rs
// ============================================================================
// Module: Docs Config Tests
// Description: System tests for docs config toggles and ingestion limits.
// Purpose: Validate fail-closed docs config behavior end-to-end.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! ## Overview
//! System tests for docs config toggles and ingestion limits.
//! Purpose: Validate fail-closed docs config behavior end-to-end.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::time::Duration;

use decision_gate_contract::ToolName;
use helpers::artifacts::TestReporter;
use helpers::docs::SearchResult;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use serde_json::json;
use tempfile::TempDir;

use crate::helpers;

#[allow(
    clippy::too_many_lines,
    reason = "End-to-end toggles are validated in a single flow for clarity."
)]
#[tokio::test(flavor = "multi_thread")]
async fn docs_config_toggles() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("docs_config_toggles")?;
    let mut transcript = Vec::new();

    let bind_disabled = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind_disabled);
    config.docs.enabled = false;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("docs search should be hidden when docs.enabled=false".into());
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
        return Err("expected docs search to be unavailable".into());
    };
    if !err.contains("unknown tool") {
        return Err(format!("unexpected docs search error: {err}").into());
    }
    let Err(err) = client.list_resources().await else {
        return Err("expected resources/list to be unavailable".into());
    };
    if !err.contains("method not found") {
        return Err(format!("unexpected resources/list error: {err}").into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    let bind_search_disabled = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind_search_disabled);
    config.docs.enable_search = false;
    config.docs.enable_resources = true;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("docs search should be hidden when enable_search=false".into());
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
        return Err("expected docs search to be unavailable".into());
    };
    if !err.contains("unknown tool") {
        return Err(format!("unexpected docs search error: {err}").into());
    }
    let resources = client.list_resources().await?;
    if resources.is_empty() {
        return Err("resources/list should return entries when enabled".into());
    }
    let evidence = resources
        .iter()
        .find(|entry| entry.uri == "decision-gate://docs/evidence-flow")
        .ok_or_else(|| "missing evidence-flow resource".to_string())?;
    let content = client.read_resource(&evidence.uri).await?;
    if !content.text.contains("# Evidence Flow + Execution Model") {
        return Err("resources/read missing expected heading".into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    let bind_resources_disabled = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind_resources_disabled);
    config.docs.enable_resources = false;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if !tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("docs search should be visible when enabled".into());
    }
    let search_value = client
        .call_tool(
            "decision_gate_docs_search",
            json!({
                "query": "trust lanes"
            }),
        )
        .await?;
    let search: SearchResult = serde_json::from_value(search_value)?;
    if search.sections.is_empty() {
        return Err("docs search returned empty sections".into());
    }
    let Err(err) = client.list_resources().await else {
        return Err("expected resources/list to be unavailable".into());
    };
    if !err.contains("method not found") {
        return Err(format!("unexpected resources/list error: {err}").into());
    }
    let Err(err) = client.read_resource("decision-gate://docs/evidence-flow").await else {
        return Err("expected resources/read to be unavailable".into());
    };
    if !err.contains("method not found") {
        return Err(format!("unexpected resources/read error: {err}").into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["docs config toggles enforce visibility and availability".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "Ingestion limits are validated in a single end-to-end flow."
)]
#[tokio::test(flavor = "multi_thread")]
async fn docs_extra_paths_ingestion_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("docs_extra_paths_ingestion_limits")?;
    let mut transcript = Vec::new();

    let temp_dir = TempDir::new()?;
    let alpha_path = temp_dir.path().join("alpha.md");
    let beta_path = temp_dir.path().join("beta.md");
    let oversized_path = temp_dir.path().join("oversized.md");
    let ignored_path = temp_dir.path().join("ignore.txt");

    let alpha_doc = "# Alpha Doc\n\nalpha-signal\n";
    let beta_doc = "# Beta Doc\n\nbeta-signal\n";
    let oversized_doc = format!("# Oversized\n\n{}", "X".repeat(512));

    fs::write(&alpha_path, alpha_doc)?;
    fs::write(&beta_path, beta_doc)?;
    fs::write(&oversized_path, oversized_doc)?;
    fs::write(&ignored_path, "ignore me")?;

    let missing_bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&missing_bind);
    config.docs.include_default_docs = false;
    config.docs.extra_paths = vec![temp_dir.path().join("missing.md").display().to_string()];
    let Err(err) = spawn_mcp_server(config).await else {
        return Err("expected missing docs.extra_paths to fail server start".into());
    };
    if !err.contains("docs.extra_paths missing") {
        return Err(format!("unexpected missing path error: {err}").into());
    }

    let bind_max_doc = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind_max_doc);
    config.docs.include_default_docs = false;
    config.docs.max_doc_bytes = 128;
    config.docs.max_total_bytes = 4096;
    config.docs.max_docs = 10;
    config.docs.extra_paths = vec![
        alpha_path.display().to_string(),
        oversized_path.display().to_string(),
        ignored_path.display().to_string(),
    ];
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let resources = client.list_resources().await?;
    let custom: Vec<_> = resources
        .iter()
        .filter(|entry| entry.uri.contains("decision-gate://docs/custom/"))
        .collect();
    if custom.len() != 1 || custom[0].uri != "decision-gate://docs/custom/alpha" {
        return Err("custom doc ingestion did not honor max_doc_bytes".into());
    }
    let search_value = client
        .call_tool(
            "decision_gate_docs_search",
            json!({
                "query": "alpha-signal"
            }),
        )
        .await?;
    let search: SearchResult = serde_json::from_value(search_value)?;
    if !search.docs_covered.iter().any(|doc| doc.doc_id == "alpha") {
        return Err("docs search missing ingested alpha doc".into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    let bind_max_docs = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind_max_docs);
    config.docs.include_default_docs = false;
    config.docs.max_doc_bytes = 1024;
    config.docs.max_total_bytes = 4096;
    config.docs.max_docs = 1;
    config.docs.extra_paths =
        vec![alpha_path.display().to_string(), beta_path.display().to_string()];
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let resources = client.list_resources().await?;
    let custom: Vec<_> = resources
        .iter()
        .filter(|entry| entry.uri.contains("decision-gate://docs/custom/"))
        .collect();
    if custom.len() != 1 || custom[0].uri != "decision-gate://docs/custom/alpha" {
        return Err("custom doc ingestion did not honor max_docs".into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    let bind_max_total = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind_max_total);
    config.docs.include_default_docs = false;
    config.docs.max_doc_bytes = 1024;
    config.docs.max_total_bytes = alpha_doc.trim().len();
    config.docs.max_docs = 10;
    config.docs.extra_paths =
        vec![alpha_path.display().to_string(), beta_path.display().to_string()];
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let resources = client.list_resources().await?;
    let custom: Vec<_> = resources
        .iter()
        .filter(|entry| entry.uri.contains("decision-gate://docs/custom/"))
        .collect();
    if custom.len() != 1 || custom[0].uri != "decision-gate://docs/custom/alpha" {
        return Err("custom doc ingestion did not honor max_total_bytes".into());
    }

    transcript.extend(client.transcript());
    server.shutdown().await;

    reporter.artifacts().write_json("tool_transcript.json", &transcript)?;
    reporter.finish(
        "pass",
        vec!["docs.extra_paths ingestion enforces limits".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
