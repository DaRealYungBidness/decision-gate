// system-tests/tests/performance.rs
// ============================================================================
// Module: Performance Smoke Tests
// Description: Lightweight throughput check for MCP workflow.
// Purpose: Provide non-gated performance visibility.
// Dependencies: system-tests helpers
// ============================================================================

//! Performance smoke tests for Decision Gate system-tests.

mod helpers;

use std::time::Instant;

use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct PerformanceMetrics {
    iterations: usize,
    total_ms: u128,
    avg_ms: f64,
    throughput_per_sec: f64,
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "non-gated performance smoke"]
async fn performance_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("performance_smoke")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(10))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(10)).await?;

    let fixture = ScenarioFixture::time_after("perf-scenario", "run-0", 0);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let iterations = 25usize;
    let start = Instant::now();

    for idx in 0 .. iterations {
        let run_id = decision_gate_core::RunId::new(format!("run-{idx}"));
        let run_config = decision_gate_core::RunConfig {
            tenant_id: fixture.tenant_id.clone(),
            namespace_id: fixture.namespace_id.clone(),
            run_id: run_id.clone(),
            scenario_id: define_output.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        };

        let start_request = ScenarioStartRequest {
            scenario_id: define_output.scenario_id.clone(),
            run_config,
            started_at: Timestamp::Logical(idx as u64 + 1),
            issue_entry_packets: false,
        };
        let start_input = serde_json::to_value(&start_request)?;
        let _state: decision_gate_core::RunState =
            client.call_tool_typed("scenario_start", start_input).await?;

        let trigger_request = ScenarioTriggerRequest {
            scenario_id: define_output.scenario_id.clone(),
            trigger: decision_gate_core::TriggerEvent {
                run_id,
                tenant_id: fixture.tenant_id.clone(),
                namespace_id: fixture.namespace_id.clone(),
                trigger_id: TriggerId::new(format!("trigger-{idx}")),
                kind: TriggerKind::ExternalEvent,
                time: Timestamp::Logical(idx as u64 + 2),
                source_id: "perf".to_string(),
                payload: None,
                correlation_id: None,
            },
        };
        let trigger_input = serde_json::to_value(&trigger_request)?;
        let _trigger: decision_gate_core::runtime::TriggerResult =
            client.call_tool_typed("scenario_trigger", trigger_input).await?;
    }

    let elapsed = start.elapsed();
    let total_ms = elapsed.as_millis();
    let total_ms_u64 =
        u64::try_from(total_ms).map_err(|_| "elapsed milliseconds overflow".to_string())?;
    let total_ms_u32 =
        u32::try_from(total_ms_u64).map_err(|_| "elapsed milliseconds too large".to_string())?;
    let iterations_u64 =
        u64::try_from(iterations).map_err(|_| "iterations overflow".to_string())?;
    let iterations_u32 =
        u32::try_from(iterations_u64).map_err(|_| "iterations too large".to_string())?;
    let avg_ms = f64::from(total_ms_u32) / f64::from(iterations_u32);
    let throughput = f64::from(iterations_u32) / elapsed.as_secs_f64().max(0.000_001);

    let metrics = PerformanceMetrics {
        iterations,
        total_ms,
        avg_ms,
        throughput_per_sec: throughput,
    };

    reporter.artifacts().write_json("perf_metrics.json", &metrics)?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec![format!("completed {iterations} iterations")],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "perf_metrics.json".to_string(),
        ],
    )?;
    Ok(())
}
