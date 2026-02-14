// system-tests/tests/suites/evidence_fuzz.rs
// ============================================================================
// Module: Evidence Fuzz Tests
// Description: Deterministic fuzz-style coverage for evidence_query inputs.
// Purpose: Ensure evidence_query rejects malformed payloads without panicking.
// Dependencies: system-tests helpers
// ============================================================================

//! ## Overview
//! Deterministic fuzz-style coverage for `evidence_query` inputs.
//! Purpose: Ensure `evidence_query` rejects malformed payloads without panicking.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::time::Duration;

use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use serde_json::Value;
use serde_json::json;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
async fn evidence_query_fuzz_inputs_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("evidence_query_fuzz_inputs_fail_closed")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, Duration::from_secs(5)).await?;

    let cases: Vec<Value> = vec![
        Value::Null,
        Value::String("not-an-object".to_string()),
        json!({}),
        json!({"query": 1}),
        json!({"query": {"provider_id": 1, "check_id": true}, "context": {}}),
        json!({"query": {"provider_id": "time", "check_id": "after"}, "context": {"tenant_id": 1}}),
        json!({
            "query": {"provider_id": "time", "check_id": "after", "params": {"timestamp": "bad"}},
            "context": {"tenant_id": 1, "namespace_id": 1, "run_id": "run-1", "trigger_id": "t1"}
        }),
        json!({
            "query": {"provider_id": "json", "check_id": "path", "params": "oops"},
            "context": {"tenant_id": 1, "namespace_id": 1, "run_id": "run-1", "trigger_id": "t1"}
        }),
        json!({
            "query": {"provider_id": "env", "check_id": "get", "params": {"key": 123}},
            "context": {"tenant_id": 1, "namespace_id": 1, "run_id": "run-1", "trigger_id": "t1"}
        }),
    ];

    let mut errors = Vec::new();
    for (index, input) in cases.into_iter().enumerate() {
        match client.call_tool("evidence_query", input).await {
            Ok(_) => return Err(format!("expected error for fuzz case {index}").into()),
            Err(err) => errors.push(format!("case {index}: {err}")),
        }
    }

    reporter.artifacts().write_json("errors.json", &errors)?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["evidence_query rejects malformed inputs without panicking".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "errors.json".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}
