// decision-gate-mcp/tests/provider_capabilities.rs
// ============================================================================
// Module: Provider Capability Validation Tests
// Description: Ensure scenario/evidence validation enforces capability contracts.
// Purpose: Validate comparator allow-lists and schema enforcement at the MCP layer.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================

//! MCP capability validation tests for scenario and evidence inputs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test-only assertions use unwrap/expect for clarity."
)]

mod common;

use decision_gate_core::Comparator;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ToolError;
use serde_json::json;

use crate::common::local_request_context;

#[test]
fn scenario_define_rejects_disallowed_comparator() {
    let router = common::sample_router();
    let mut spec = common::sample_spec();
    spec.predicates[0].comparator = Comparator::GreaterThan;

    let request = ScenarioDefineRequest {
        spec,
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_define",
        serde_json::to_value(&request).unwrap(),
    );
    let err = result.expect_err("expected comparator violation");
    match err {
        ToolError::CapabilityViolation {
            code, ..
        } => assert_eq!(code, "comparator_not_allowed"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn scenario_define_rejects_expected_schema_mismatch() {
    let router = common::sample_router();
    let mut spec = common::sample_spec();
    spec.predicates[0].expected = Some(json!("not-a-boolean"));

    let request = ScenarioDefineRequest {
        spec,
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_define",
        serde_json::to_value(&request).unwrap(),
    );
    let err = result.expect_err("expected schema violation");
    match err {
        ToolError::CapabilityViolation {
            code, ..
        } => assert_eq!(code, "expected_invalid"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn evidence_query_rejects_missing_params() {
    let router = common::sample_router();
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            predicate: "get".to_string(),
            params: None,
        },
        context: common::sample_context(),
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "evidence_query",
        serde_json::to_value(&request).unwrap(),
    );
    let err = result.expect_err("expected params missing error");
    match err {
        ToolError::CapabilityViolation {
            code, ..
        } => assert_eq!(code, "params_missing"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn evidence_query_rejects_invalid_params() {
    let router = common::sample_router();
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            predicate: "after".to_string(),
            params: Some(json!({ "timestamp": true })),
        },
        context: common::sample_context(),
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "evidence_query",
        serde_json::to_value(&request).unwrap(),
    );
    let err = result.expect_err("expected params invalid error");
    match err {
        ToolError::CapabilityViolation {
            code, ..
        } => assert_eq!(code, "params_invalid"),
        other => panic!("unexpected error: {other:?}"),
    }
}
