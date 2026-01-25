// decision-gate-mcp/tests/tool_router.rs
// ============================================================================
// Module: Tool Router Tests
// Description: Comprehensive tests for MCP tool routing and error handling.
// Purpose: Ensure all tools function correctly and errors are handled safely.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Tests all MCP tools for happy path, error handling, and edge cases.
//!
//! Security posture: Validates fail-closed behavior for invalid inputs.
//! Threat model: TM-MCP-001 - Tool routing bypass or injection.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

mod common;

use decision_gate_core::Comparator;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::NextResult;
use decision_gate_core::runtime::ScenarioStatus;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_core::runtime::SubmitRequest;
use decision_gate_core::runtime::SubmitResult;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::SchemaRegistryConfig;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::EvidenceQueryResponse;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ProvidersListRequest;
use decision_gate_mcp::tools::ProvidersListResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioSubmitRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::ScenariosListRequest;
use decision_gate_mcp::tools::ScenariosListResponse;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasGetResponse;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasListResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use decision_gate_mcp::tools::SchemasRegisterResponse;
use ret_logic::TriState;
use serde_json::json;

use crate::common::define_scenario;
use crate::common::local_request_context;
use crate::common::router_with_config;
use crate::common::sample_config;
use crate::common::sample_context;
use crate::common::sample_router;
use crate::common::sample_run_config_with_ids;
use crate::common::sample_spec;
use crate::common::sample_spec_with_id;
use crate::common::sample_spec_with_two_predicates;
use crate::common::setup_scenario_with_run;
use crate::common::start_run;

// ============================================================================
// SECTION: Tool Listing Tests
// ============================================================================

fn sample_shape_record(schema_id: &str, version: &str) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("test schema".to_string()),
        created_at: Timestamp::Logical(1),
    }
}

/// Verifies all expected tools are listed.
#[test]
fn list_tools_returns_all_fifteen_tools() {
    let router = sample_router();
    let tools = router.list_tools(&local_request_context()).unwrap();

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"scenario_define"));
    assert!(names.contains(&"scenario_start"));
    assert!(names.contains(&"scenario_status"));
    assert!(names.contains(&"scenario_next"));
    assert!(names.contains(&"scenario_submit"));
    assert!(names.contains(&"scenario_trigger"));
    assert!(names.contains(&"evidence_query"));
    assert!(names.contains(&"runpack_export"));
    assert!(names.contains(&"runpack_verify"));
    assert!(names.contains(&"providers_list"));
    assert!(names.contains(&"schemas_register"));
    assert!(names.contains(&"schemas_list"));
    assert!(names.contains(&"schemas_get"));
    assert!(names.contains(&"scenarios_list"));
    assert!(names.contains(&"precheck"));
    assert_eq!(tools.len(), 15);
}

// ============================================================================
// SECTION: Unknown Tool Tests
// ============================================================================

/// Verifies unknown tool names are rejected.
#[test]
fn unknown_tool_returns_error() {
    let router = sample_router();
    let result = router.handle_tool_call(&local_request_context(), "nonexistent_tool", json!({}));
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("unknown tool"));
}

// ============================================================================
// SECTION: scenario_define Tests
// ============================================================================

/// Verifies `scenario_define` registers a new scenario.
#[test]
fn scenario_define_registers_scenario() {
    let router = sample_router();
    let spec = sample_spec();
    let result = define_scenario(&router, spec);
    assert!(result.is_ok());
}

/// Verifies `scenario_define` returns scenario ID and spec hash.
#[test]
fn scenario_define_returns_id_and_hash() {
    let router = sample_router();
    let spec = sample_spec();
    let request = ScenarioDefineRequest {
        spec,
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let response: ScenarioDefineResponse = serde_json::from_value(result).unwrap();

    assert_eq!(response.scenario_id.as_str(), "test-scenario");
    assert!(!response.spec_hash.value.is_empty());
}

/// Verifies defining the same scenario twice returns a conflict error.
#[test]
fn scenario_define_duplicate_returns_conflict() {
    let router = sample_router();
    let spec = sample_spec();
    define_scenario(&router, spec.clone()).unwrap();

    let result = define_scenario(&router, spec);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.contains("conflict") || error.contains("already defined"));
}

/// Verifies invalid params are rejected.
#[test]
fn scenario_define_invalid_params_rejected() {
    let router = sample_router();
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_define",
        json!({"invalid": "params"}),
    );
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("invalid parameters"));
}

// ============================================================================
// SECTION: scenario_start Tests
// ============================================================================

/// Verifies `scenario_start` creates a new run.
#[test]
fn scenario_start_creates_run() {
    let router = sample_router();
    let spec = sample_spec();
    let scenario_id = define_scenario(&router, spec).unwrap();

    let run_config = sample_run_config_with_ids("tenant", "run-1", scenario_id.as_str());
    let result = start_run(&router, &scenario_id, run_config, Timestamp::Logical(1));
    assert!(result.is_ok());
}

/// Verifies starting a run for an undefined scenario fails.
#[test]
fn scenario_start_undefined_scenario_fails() {
    let router = sample_router();
    let scenario_id = ScenarioId::new("nonexistent");
    let run_config = sample_run_config_with_ids("tenant", "run-1", "nonexistent");

    let request = ScenarioStartRequest {
        scenario_id,
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_start",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("not found") || error.to_string().contains("not defined"));
}

/// Verifies invalid params are rejected.
#[test]
fn scenario_start_invalid_params_rejected() {
    let router = sample_router();
    let result =
        router.handle_tool_call(&local_request_context(), "scenario_start", json!({"bad": "data"}));
    assert!(result.is_err());
}

// ============================================================================
// SECTION: scenario_status Tests
// ============================================================================

/// Verifies `scenario_status` returns status for an active run.
#[test]
fn scenario_status_returns_status() {
    let (router, scenario_id, run_id) = setup_scenario_with_run();

    let request = ScenarioStatusRequest {
        scenario_id,
        request: StatusRequest {
            run_id,
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            requested_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_status",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let status: ScenarioStatus = serde_json::from_value(result).unwrap();

    assert_eq!(status.current_stage_id.as_str(), "stage-1");
}

/// Verifies status for nonexistent scenario fails.
#[test]
fn scenario_status_undefined_scenario_fails() {
    let router = sample_router();

    let request = ScenarioStatusRequest {
        scenario_id: ScenarioId::new("nonexistent"),
        request: StatusRequest {
            run_id: RunId::new("run-1"),
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            requested_at: Timestamp::Logical(1),
            correlation_id: None,
        },
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_status",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: scenario_next Tests
// ============================================================================

/// Verifies `scenario_next` advances evaluation.
#[test]
fn scenario_next_advances_evaluation() {
    let (router, scenario_id, run_id) = setup_scenario_with_run();

    let request = ScenarioNextRequest {
        scenario_id,
        request: NextRequest {
            run_id,
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "test-agent".to_string(),
            time: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_next",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let next_result: NextResult = serde_json::from_value(result).unwrap();

    // Should have evaluated (decision recorded with the trigger id)
    assert_eq!(next_result.decision.trigger_id, TriggerId::new("trigger-1"));
}

/// Verifies next for undefined scenario fails.
#[test]
fn scenario_next_undefined_scenario_fails() {
    let router = sample_router();

    let request = ScenarioNextRequest {
        scenario_id: ScenarioId::new("nonexistent"),
        request: NextRequest {
            run_id: RunId::new("run-1"),
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            trigger_id: TriggerId::new("trigger-1"),
            agent_id: "test-agent".to_string(),
            time: Timestamp::Logical(1),
            correlation_id: None,
        },
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_next",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: scenario_submit Tests
// ============================================================================

/// Verifies `scenario_submit` accepts submissions.
#[test]
fn scenario_submit_accepts_submissions() {
    let (router, scenario_id, run_id) = setup_scenario_with_run();

    let request = ScenarioSubmitRequest {
        scenario_id,
        request: SubmitRequest {
            run_id,
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            submission_id: "submission-1".to_string(),
            payload: PacketPayload::Json {
                value: json!({"artifact": "value"}),
            },
            content_type: "application/json".to_string(),
            submitted_at: Timestamp::Logical(2),
            correlation_id: None,
        },
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_submit",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let _submit_result: SubmitResult = serde_json::from_value(result).unwrap();
}

/// Verifies submit for undefined scenario fails.
#[test]
fn scenario_submit_undefined_scenario_fails() {
    let router = sample_router();

    let request = ScenarioSubmitRequest {
        scenario_id: ScenarioId::new("nonexistent"),
        request: SubmitRequest {
            run_id: RunId::new("run-1"),
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            submission_id: "submission-1".to_string(),
            payload: PacketPayload::Json {
                value: json!({"artifact": "value"}),
            },
            content_type: "application/json".to_string(),
            submitted_at: Timestamp::Logical(1),
            correlation_id: None,
        },
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_submit",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: scenario_trigger Tests
// ============================================================================

/// Verifies `scenario_trigger` processes trigger events.
#[test]
fn scenario_trigger_processes_event() {
    let (router, scenario_id, run_id) = setup_scenario_with_run();

    let request = ScenarioTriggerRequest {
        scenario_id,
        trigger: TriggerEvent {
            run_id,
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            trigger_id: TriggerId::new("external-trigger"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "external-agent".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_trigger",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let _trigger_result: TriggerResult = serde_json::from_value(result).unwrap();
}

/// Verifies trigger for undefined scenario fails.
#[test]
fn scenario_trigger_undefined_scenario_fails() {
    let router = sample_router();

    let request = ScenarioTriggerRequest {
        scenario_id: ScenarioId::new("nonexistent"),
        trigger: TriggerEvent {
            run_id: RunId::new("run-1"),
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(1),
            source_id: "test".to_string(),
            payload: None,
            correlation_id: None,
        },
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "scenario_trigger",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: evidence_query Tests
// ============================================================================

/// Verifies `evidence_query` returns results from time provider.
#[test]
fn evidence_query_returns_time_now() {
    let router = sample_router();

    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            predicate: "now".to_string(),
            params: None,
        },
        context: sample_context(),
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "evidence_query",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let response: EvidenceQueryResponse = serde_json::from_value(result).unwrap();

    // By default, raw values are redacted
    assert!(response.result.evidence_hash.is_some());
}

/// Verifies `evidence_query` for unknown provider fails.
#[test]
fn evidence_query_unknown_provider_fails() {
    let router = sample_router();

    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("nonexistent"),
            predicate: "test".to_string(),
            params: None,
        },
        context: sample_context(),
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "evidence_query",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

/// Verifies `evidence_query` invalid params rejected.
#[test]
fn evidence_query_invalid_params_rejected() {
    let router = sample_router();
    let result = router.handle_tool_call(
        &local_request_context(),
        "evidence_query",
        json!({"bad": "request"}),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: runpack_export Tests
// ============================================================================

/// Verifies `runpack_export` requires an existing run.
#[test]
fn runpack_export_missing_run_fails() {
    let router = sample_router();
    let spec = sample_spec();
    let scenario_id = define_scenario(&router, spec).unwrap();

    let request = decision_gate_mcp::tools::RunpackExportRequest {
        scenario_id,
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("nonexistent-run"),
        output_dir: "/tmp/test-runpack".to_string(),
        manifest_name: None,
        generated_at: Timestamp::Logical(1),
        include_verification: false,
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "runpack_export",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

/// Verifies `runpack_export` undefined scenario fails.
#[test]
fn runpack_export_undefined_scenario_fails() {
    let router = sample_router();

    let request = decision_gate_mcp::tools::RunpackExportRequest {
        scenario_id: ScenarioId::new("nonexistent"),
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
        output_dir: "/tmp/test-runpack".to_string(),
        manifest_name: None,
        generated_at: Timestamp::Logical(1),
        include_verification: false,
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "runpack_export",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: runpack_verify Tests
// ============================================================================

/// Verifies `runpack_verify` rejects missing runpack directory.
#[test]
fn runpack_verify_missing_directory_fails() {
    let router = sample_router();

    let request = decision_gate_mcp::tools::RunpackVerifyRequest {
        runpack_dir: "/nonexistent/runpack/path".to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    let result = router.handle_tool_call(
        &local_request_context(),
        "runpack_verify",
        serde_json::to_value(&request).unwrap(),
    );
    assert!(result.is_err());
}

/// Verifies `runpack_verify` invalid params rejected.
#[test]
fn runpack_verify_invalid_params_rejected() {
    let router = sample_router();
    let result = router.handle_tool_call(
        &local_request_context(),
        "runpack_verify",
        json!({"not": "valid"}),
    );
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Multiple Scenario Tests
// ============================================================================

/// Verifies multiple scenarios can be defined independently.
#[test]
fn multiple_scenarios_independent() {
    let router = sample_router();

    let spec1 = sample_spec_with_id("scenario-1");
    let spec2 = sample_spec_with_id("scenario-2");

    let id1 = define_scenario(&router, spec1).unwrap();
    let id2 = define_scenario(&router, spec2).unwrap();

    assert_eq!(id1.as_str(), "scenario-1");
    assert_eq!(id2.as_str(), "scenario-2");

    // Start runs on both
    let config1 = sample_run_config_with_ids("tenant", "run-1", "scenario-1");
    let config2 = sample_run_config_with_ids("tenant", "run-2", "scenario-2");

    start_run(&router, &id1, config1, Timestamp::Logical(1)).unwrap();
    start_run(&router, &id2, config2, Timestamp::Logical(1)).unwrap();
}

// ============================================================================
// SECTION: Idempotency Tests
// ============================================================================

/// Verifies `scenario_next` is idempotent for same trigger.
#[test]
fn scenario_next_idempotent_same_trigger() {
    let (router, scenario_id, run_id) = setup_scenario_with_run();

    let request = ScenarioNextRequest {
        scenario_id,
        request: NextRequest {
            run_id,
            tenant_id: TenantId::new("test-tenant"),
            namespace_id: NamespaceId::new("default"),
            trigger_id: TriggerId::new("same-trigger"),
            agent_id: "test-agent".to_string(),
            time: Timestamp::Logical(2),
            correlation_id: None,
        },
    };

    let result1 = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_next",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let result2 = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_next",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();

    let next1: NextResult = serde_json::from_value(result1).unwrap();
    let next2: NextResult = serde_json::from_value(result2).unwrap();

    // Same trigger should produce same decision
    assert_eq!(next1.decision, next2.decision);
}

// ============================================================================
// SECTION: Schema Registry Tools
// ============================================================================

#[test]
fn schemas_register_and_get_roundtrip() {
    let router = sample_router();
    let record = sample_shape_record("asserted", "v1");
    let register = SchemasRegisterRequest {
        record: record.clone(),
    };
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();
    let registered: SchemasRegisterResponse = serde_json::from_value(response).unwrap();
    assert_eq!(registered.record.schema_id, record.schema_id);

    let get_request = SchemasGetRequest {
        tenant_id: record.tenant_id.clone(),
        namespace_id: record.namespace_id.clone(),
        schema_id: record.schema_id.clone(),
        version: record.version.clone(),
    };
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_get",
            serde_json::to_value(&get_request).unwrap(),
        )
        .unwrap();
    let fetched: SchemasGetResponse = serde_json::from_value(response).unwrap();
    assert_eq!(fetched.record.schema_id, record.schema_id);
}

#[test]
fn schemas_register_duplicate_rejected() {
    let router = sample_router();
    let record = sample_shape_record("asserted", "v1");
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("conflict"));
}

#[test]
fn schemas_register_rejects_oversize_payload() {
    let router = sample_router();
    let max_bytes = SchemaRegistryConfig::default().max_schema_bytes;
    let large = "a".repeat(max_bytes.saturating_add(16));
    let mut record = sample_shape_record("oversize", "v1");
    record.schema = json!({
        "type": "object",
        "description": large
    });
    let register = SchemasRegisterRequest {
        record,
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("schema exceeds size limit"));
}

#[test]
fn schemas_list_pagination() {
    let router = sample_router();
    let record_a = sample_shape_record("alpha", "v1");
    let record_b = sample_shape_record("bravo", "v1");
    for record in [record_a.clone(), record_b] {
        let register = SchemasRegisterRequest {
            record,
        };
        let _ = router
            .handle_tool_call(
                &local_request_context(),
                "schemas_register",
                serde_json::to_value(&register).unwrap(),
            )
            .unwrap();
    }

    let tenant_id = record_a.tenant_id;
    let namespace_id = record_a.namespace_id;
    let list_request = SchemasListRequest {
        tenant_id: tenant_id.clone(),
        namespace_id: namespace_id.clone(),
        cursor: None,
        limit: Some(1),
    };
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_list",
            serde_json::to_value(&list_request).unwrap(),
        )
        .unwrap();
    let page: SchemasListResponse = serde_json::from_value(response).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.next_token.is_some());

    let next_request = SchemasListRequest {
        tenant_id,
        namespace_id,
        cursor: page.next_token,
        limit: Some(1),
    };
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_list",
            serde_json::to_value(&next_request).unwrap(),
        )
        .unwrap();
    let page: SchemasListResponse = serde_json::from_value(response).unwrap();
    assert_eq!(page.items.len(), 1);
}

#[test]
fn schemas_list_rejects_invalid_cursor() {
    let router = sample_router();
    let record = sample_shape_record("alpha", "v1");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let list_request = SchemasListRequest {
        tenant_id,
        namespace_id,
        cursor: Some("not-json".to_string()),
        limit: Some(1),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_list",
            serde_json::to_value(&list_request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("invalid cursor"));
}

#[test]
fn schemas_list_rejects_zero_limit() {
    let router = sample_router();
    let record = sample_shape_record("alpha", "v1");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let list_request = SchemasListRequest {
        tenant_id,
        namespace_id,
        cursor: None,
        limit: Some(0),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_list",
            serde_json::to_value(&list_request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("limit must be between"));
}

#[test]
fn schemas_register_rejects_when_registry_full() {
    let mut config = sample_config();
    config.schema_registry.max_entries = Some(1);
    let router = router_with_config(config);
    let record_a = sample_shape_record("alpha", "v1");
    let record_b = sample_shape_record("bravo", "v1");
    let register_a = SchemasRegisterRequest {
        record: record_a,
    };
    let register_b = SchemasRegisterRequest {
        record: record_b,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register_a).unwrap(),
        )
        .unwrap();
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register_b).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("max entries"));
}

#[test]
fn schemas_get_missing_rejected() {
    let router = sample_router();
    let request = SchemasGetRequest {
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        schema_id: DataShapeId::new("missing"),
        version: DataShapeVersion::new("v1"),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_get",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("not found"));
}

// ============================================================================
// SECTION: Discovery Tools
// ============================================================================

#[test]
fn providers_list_includes_builtin_provider() {
    let router = sample_router();
    let request = ProvidersListRequest {};
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "providers_list",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let response: ProvidersListResponse = serde_json::from_value(response).unwrap();
    assert!(response.providers.iter().any(|provider| provider.provider_id == "time"));
}

#[test]
fn scenarios_list_includes_defined_scenario() {
    let router = sample_router();
    let spec = sample_spec_with_id("scenario-list");
    let _ = define_scenario(&router, spec.clone()).unwrap();
    let request = ScenariosListRequest {
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        cursor: None,
        limit: None,
    };
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "scenarios_list",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let response: ScenariosListResponse = serde_json::from_value(response).unwrap();
    assert!(response.items.iter().any(|scenario| scenario.scenario_id == spec.scenario_id));
}

// ============================================================================
// SECTION: Precheck
// ============================================================================

#[test]
fn precheck_accepts_asserted_payload() {
    let mut config = sample_config();
    config.trust.min_lane = TrustLane::Asserted;
    let router = router_with_config(config);
    let spec = sample_spec_with_id("precheck-scenario");
    let _ = define_scenario(&router, spec.clone()).unwrap();
    let record = sample_shape_record("asserted", "v1");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: Some(spec.scenario_id),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": true}),
    };
    let response = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let response: PrecheckToolResponse = serde_json::from_value(response).unwrap();
    match response.decision {
        DecisionOutcome::Complete {
            stage_id,
        } => {
            assert_eq!(stage_id, spec.stages[0].stage_id);
        }
        other => panic!("unexpected decision: {other:?}"),
    }
    assert_eq!(response.gate_evaluations[0].status, TriState::True);
}

#[test]
fn precheck_rejects_payload_mismatch() {
    let mut config = sample_config();
    config.trust.min_lane = TrustLane::Asserted;
    let router = router_with_config(config);
    let spec = sample_spec_with_id("precheck-mismatch");
    let _ = define_scenario(&router, spec.clone()).unwrap();
    let record = sample_shape_record("asserted", "v1");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: Some(spec.scenario_id),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": "nope"}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("payload does not match schema"));
}

#[test]
fn precheck_rejects_comparator_schema_mismatch() {
    let mut config = sample_config();
    config.trust.min_lane = TrustLane::Asserted;
    let router = router_with_config(config);
    let mut spec = sample_spec_with_id("precheck-comparator-mismatch");
    spec.predicates[0].query.predicate = "now".to_string();
    spec.predicates[0].query.params = None;
    spec.predicates[0].comparator = Comparator::GreaterThan;
    spec.predicates[0].expected = Some(json!(10));
    let _ = define_scenario(&router, spec.clone()).unwrap();

    let mut record = sample_shape_record("asserted", "v1");
    record.schema = json!({
        "type": "object",
        "properties": {
            "after": { "type": "string" }
        },
        "required": ["after"],
        "additionalProperties": false
    });
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: Some(spec.scenario_id),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": "2024-01-01"}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("comparator greater_than not allowed"));
}

#[test]
fn precheck_rejects_missing_scenario_and_spec() {
    let router = sample_router();
    let record = sample_shape_record("asserted", "v1");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: None,
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": true}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("scenario_id or spec is required"));
}

#[test]
fn precheck_rejects_scenario_id_spec_mismatch() {
    let router = sample_router();
    let record = sample_shape_record("asserted", "v1");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let spec = sample_spec_with_id("scenario-a");
    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: Some(ScenarioId::new("scenario-b")),
        spec: Some(spec),
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": true}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("scenario_id does not match spec"));
}

#[test]
fn precheck_rejects_spec_namespace_mismatch() {
    let router = sample_router();
    let mut record = sample_shape_record("asserted", "v1");
    record.namespace_id = NamespaceId::new("other");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let spec = sample_spec_with_id("scenario-a");
    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: None,
        spec: Some(spec),
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": true}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("namespace_id does not match spec"));
}

#[test]
fn precheck_rejects_default_tenant_mismatch() {
    let router = sample_router();
    let mut record = sample_shape_record("asserted", "v1");
    record.tenant_id = TenantId::new("tenant-b");
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let mut spec = sample_spec_with_id("scenario-tenant");
    spec.default_tenant_id = Some(TenantId::new("tenant-a"));
    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: None,
        spec: Some(spec),
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!({"after": true}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("tenant_id does not match scenario tenant"));
}

#[test]
fn precheck_rejects_unknown_schema() {
    let router = sample_router();
    let request = PrecheckToolRequest {
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        scenario_id: Some(ScenarioId::new("scenario-precheck")),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: DataShapeId::new("missing"),
            version: DataShapeVersion::new("v1"),
        },
        payload: json!({"after": true}),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("schema not found"));
}

#[test]
fn precheck_rejects_non_object_payload_with_multiple_predicates() {
    let mut config = sample_config();
    config.trust.min_lane = TrustLane::Asserted;
    let router = router_with_config(config);
    let spec = sample_spec_with_two_predicates("precheck-multi");
    let record = DataShapeRecord {
        schema: json!({"type": "boolean"}),
        ..sample_shape_record("asserted", "v1")
    };
    let tenant_id = record.tenant_id.clone();
    let namespace_id = record.namespace_id.clone();
    let schema_id = record.schema_id.clone();
    let version = record.version.clone();
    let register = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&register).unwrap(),
        )
        .unwrap();

    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: None,
        spec: Some(spec),
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id,
            version,
        },
        payload: json!(true),
    };
    let error = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("non-object data shape requires exactly one predicate"));
}
