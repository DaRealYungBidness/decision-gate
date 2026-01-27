// decision-gate-mcp/tests/usage_meter.rs
// ============================================================================
// Module: Usage Meter Tests
// Description: Verify usage meter hook enforces quota decisions.
// Purpose: Ensure usage metering can block tool calls before execution.
// Dependencies: decision-gate-mcp, decision-gate-core
// ============================================================================

//! Usage metering hook tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unwrap_in_result,
    reason = "Test setup uses unwraps for clarity."
)]

mod common;

use std::sync::Arc;
use std::sync::Mutex;

use common::local_request_context;
use common::router_with_authorizer_and_usage;
use common::sample_config;
use common::sample_spec;
use decision_gate_mcp::UsageCheckRequest;
use decision_gate_mcp::UsageDecision;
use decision_gate_mcp::UsageMeter;
use decision_gate_mcp::UsageMetric;
use decision_gate_mcp::UsageRecord;
use decision_gate_mcp::tools::ScenarioDefineRequest;

struct DenyUsageMeter;

impl UsageMeter for DenyUsageMeter {
    fn check(
        &self,
        _auth: &decision_gate_mcp::AuthContext,
        _request: UsageCheckRequest<'_>,
    ) -> UsageDecision {
        UsageDecision {
            allowed: false,
            reason: "quota_exceeded".to_string(),
        }
    }

    fn record(&self, _auth: &decision_gate_mcp::AuthContext, _record: UsageRecord<'_>) {}
}

#[test]
fn usage_meter_denies_tool_call() {
    let config = sample_config();
    let router = router_with_authorizer_and_usage(
        &config,
        Arc::new(decision_gate_mcp::NoopTenantAuthorizer),
        Arc::new(DenyUsageMeter),
    );
    let request = ScenarioDefineRequest {
        spec: sample_spec(),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("quota_exceeded"));
}

#[test]
fn usage_meter_receives_metric() {
    struct InspectingMeter {
        seen: Mutex<Vec<UsageMetric>>,
    }

    impl UsageMeter for InspectingMeter {
        fn check(
            &self,
            _auth: &decision_gate_mcp::AuthContext,
            request: UsageCheckRequest<'_>,
        ) -> UsageDecision {
            self.seen.lock().expect("seen lock").push(request.metric);
            UsageDecision {
                allowed: true,
                reason: "allow".to_string(),
            }
        }

        fn record(&self, _auth: &decision_gate_mcp::AuthContext, record: UsageRecord<'_>) {
            self.seen.lock().expect("seen lock").push(record.metric);
        }
    }

    let config = sample_config();
    let meter = Arc::new(InspectingMeter {
        seen: Mutex::new(Vec::new()),
    });
    let meter_dyn: Arc<dyn UsageMeter> = meter.clone();
    let router = router_with_authorizer_and_usage(
        &config,
        Arc::new(decision_gate_mcp::NoopTenantAuthorizer),
        meter_dyn,
    );
    let request = ScenarioDefineRequest {
        spec: sample_spec(),
    };
    router
        .handle_tool_call(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        )
        .expect("tool call should succeed");
    let seen_tool_call = {
        let seen = meter.seen.lock().expect("seen lock");
        seen.contains(&UsageMetric::ToolCall)
    };
    assert!(seen_tool_call);
}
