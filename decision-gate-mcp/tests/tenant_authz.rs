// decision-gate-mcp/tests/tenant_authz.rs
// ============================================================================
// Module: Tenant Authorization Tests
// Description: Verify tenant authorization hook enforces deny decisions.
// Purpose: Ensure tenant authz can block tool calls before execution.
// Dependencies: decision-gate-mcp, decision-gate-core
// ============================================================================

//! Tenant authorization hook tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unwrap_in_result,
    reason = "Test setup uses unwraps for clarity."
)]

mod common;

use std::sync::Arc;

use common::local_request_context;
use common::router_with_authorizer;
use common::sample_config;
use common::sample_spec;
use common::ToolRouterSyncExt;
use decision_gate_mcp::TenantAccessRequest;
use decision_gate_mcp::TenantAuthorizer;
use decision_gate_mcp::TenantAuthzAction;
use decision_gate_mcp::TenantAuthzDecision;
use decision_gate_mcp::tools::ScenarioDefineRequest;

struct DenyTenantAuthorizer;

impl TenantAuthorizer for DenyTenantAuthorizer {
    fn authorize(
        &self,
        _auth: &decision_gate_mcp::AuthContext,
        _request: TenantAccessRequest<'_>,
    ) -> TenantAuthzDecision {
        TenantAuthzDecision {
            allowed: false,
            reason: "tenant_access_denied".to_string(),
        }
    }
}

#[test]
fn tenant_authz_denies_tool_call() {
    let config = sample_config();
    let router = router_with_authorizer(&config, Arc::new(DenyTenantAuthorizer));
    let request = ScenarioDefineRequest {
        spec: sample_spec(),
    };
    let error = router
        .handle_tool_call_sync(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("tenant_access_denied"));
}

#[test]
fn tenant_authz_receives_tool_action() {
    struct InspectingAuthorizer;

    impl TenantAuthorizer for InspectingAuthorizer {
        fn authorize(
            &self,
            _auth: &decision_gate_mcp::AuthContext,
            request: TenantAccessRequest<'_>,
        ) -> TenantAuthzDecision {
            match request.action {
                TenantAuthzAction::ToolCall(tool) => {
                    assert_eq!(tool.as_str(), "scenario_define");
                }
            }
            TenantAuthzDecision {
                allowed: true,
                reason: "allow".to_string(),
            }
        }
    }

    let config = sample_config();
    let router = router_with_authorizer(&config, Arc::new(InspectingAuthorizer));
    let request = ScenarioDefineRequest {
        spec: sample_spec(),
    };
    router
        .handle_tool_call_sync(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        )
        .expect("tool call should succeed");
}
