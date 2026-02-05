// system-tests/tests/suites/precheck.rs
// ============================================================================
// Module: Precheck Correctness Tests
// Description: Validate read-only and trust-lane precheck behavior.
// Purpose: Ensure precheck never mutates run state.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Validate read-only and trust-lane precheck behavior.
//! Purpose: Ensure precheck never mutates run state.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::RunState;
use decision_gate_core::Timestamp;
use decision_gate_core::runtime::ScenarioStatus;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::json;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines, reason = "Precheck read-only flow is clearer as one sequence.")]
async fn precheck_read_only_does_not_mutate_run_state() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("precheck_read_only_does_not_mutate_run_state")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let client = server.client(std::time::Duration::from_secs(5))?;
    wait_for_server_ready(&client, std::time::Duration::from_secs(5)).await?;

    let mut fixture = ScenarioFixture::time_after("precheck-readonly", "run-1", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let record = DataShapeRecord {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("precheck schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let register_request = SchemasRegisterRequest {
        record: record.clone(),
    };
    let register_input = serde_json::to_value(&register_request)?;
    let _register_output: serde_json::Value =
        client.call_tool_typed("schemas_register", register_input).await?;

    let run_config = fixture.run_config();
    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: run_config.clone(),
        started_at: Timestamp::Logical(2),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: RunState = client.call_tool_typed("scenario_start", start_input).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: StatusRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            requested_at: Timestamp::Logical(3),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status_before: ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    let precheck_request = PrecheckToolRequest {
        tenant_id: fixture.tenant_id,
        namespace_id: fixture.namespace_id,
        scenario_id: Some(define_output.scenario_id.clone()),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: record.schema_id.clone(),
            version: record.version.clone(),
        },
        payload: json!({"after": true}),
    };
    let precheck_input = serde_json::to_value(&precheck_request)?;
    let _precheck_output: PrecheckToolResponse =
        client.call_tool_typed("precheck", precheck_input).await?;

    let status_request = ScenarioStatusRequest {
        scenario_id: define_output.scenario_id.clone(),
        request: StatusRequest {
            run_id: run_config.run_id.clone(),
            tenant_id: run_config.tenant_id,
            namespace_id: run_config.namespace_id,
            requested_at: Timestamp::Logical(4),
            correlation_id: None,
        },
    };
    let status_input = serde_json::to_value(&status_request)?;
    let status_after: ScenarioStatus =
        client.call_tool_typed("scenario_status", status_input).await?;

    if status_before != status_after {
        return Err("precheck mutated run state".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["precheck leaves run state unchanged".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    drop(reporter);
    server.shutdown().await;
    Ok(())
}
