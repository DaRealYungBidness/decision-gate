// system-tests/tests/suites/docs_search.rs
// ============================================================================
// Module: Docs Search Tests
// Description: End-to-end validation for the docs search tool.
// Purpose: Ensure deterministic docs search behavior over HTTP MCP.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Docs search system tests.

use std::time::Duration;

use decision_gate_contract::ToolName;
use helpers::artifacts::TestReporter;
use helpers::docs::DocRole;
use helpers::docs::SearchResult;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use serde_json::json;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn docs_search_http_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("docs_search_http_end_to_end")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.docs.max_sections = 4;
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let tools = client.list_tools().await?;
    if !tools.iter().any(|tool| tool.name == ToolName::DecisionGateDocsSearch) {
        return Err("tools/list missing decision_gate_docs_search".into());
    }

    let search_input = json!({
        "query": "trust lanes",
        "max_sections": 4
    });
    let result_value = client.call_tool("decision_gate_docs_search", search_input.clone()).await?;
    let result: SearchResult = serde_json::from_value(result_value)?;
    if !result.docs_covered.iter().any(|doc| doc.doc_id == "evidence_flow_and_execution_model") {
        return Err("docs search missing evidence flow doc".into());
    }

    let result_repeat_value = client.call_tool("decision_gate_docs_search", search_input).await?;
    let result_repeat: SearchResult = serde_json::from_value(result_repeat_value)?;
    if result != result_repeat {
        return Err("docs search results are not deterministic".into());
    }

    let overview_value = client
        .call_tool(
            "decision_gate_docs_search",
            json!({
                "query": "",
                "max_sections": 100
            }),
        )
        .await?;
    let overview: SearchResult = serde_json::from_value(overview_value)?;
    if overview.sections.len() != 4 {
        return Err(
            format!("overview should return 4 sections, got {}", overview.sections.len()).into()
        );
    }
    let roles: Vec<DocRole> = overview.sections.iter().map(|section| section.doc_role).collect();
    let expected = vec![DocRole::Reasoning, DocRole::Decision, DocRole::Ontology, DocRole::Pattern];
    if roles != expected {
        return Err(format!("overview roles unexpected: {roles:?}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["docs search returns deterministic sections and overview".to_string()],
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
