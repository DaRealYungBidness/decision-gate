// decision-gate-mcp/tests/precheck_audit.rs
// ============================================================================
// Module: Precheck Audit Tests
// Description: Verify hash-only audit logging for precheck requests.
// Purpose: Ensure asserted payloads are never logged without opt-in.
// Dependencies: decision-gate-mcp, decision-gate-core
// ============================================================================

//! Precheck audit logging tests.

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

use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TrustLane;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::FederatedEvidenceProvider;
use decision_gate_mcp::McpAuditEvent;
use decision_gate_mcp::McpAuditSink;
use decision_gate_mcp::PrecheckAuditEvent;
use decision_gate_mcp::ToolRouter;
use decision_gate_mcp::auth::DefaultToolAuthz;
use decision_gate_mcp::auth::NoopAuditSink;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::namespace_authority::NoopNamespaceAuthority;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use decision_gate_mcp::tools::ToolRouterConfig;
use serde_json::json;

use crate::common::local_request_context;
use crate::common::sample_spec;

#[derive(Default)]
struct TestAuditSink {
    precheck_events: Mutex<Vec<PrecheckAuditEvent>>,
}

impl McpAuditSink for TestAuditSink {
    fn record(&self, _event: &McpAuditEvent) {}

    fn record_precheck(&self, event: &PrecheckAuditEvent) {
        self.precheck_events.lock().expect("precheck events lock").push(event.clone());
    }
}

fn build_router(mut config: DecisionGateConfig, audit: Arc<TestAuditSink>) -> ToolRouter {
    config.trust.min_lane = TrustLane::Asserted;
    let evidence = FederatedEvidenceProvider::from_config(&config).unwrap();
    let capabilities = CapabilityRegistry::from_config(&config).unwrap();
    let store = decision_gate_core::SharedRunStateStore::from_store(
        decision_gate_core::InMemoryRunStateStore::new(),
    );
    let schema_registry = decision_gate_core::SharedDataShapeRegistry::from_registry(
        decision_gate_core::InMemoryDataShapeRegistry::new(),
    );
    let provider_transports = config
        .providers
        .iter()
        .map(|provider| {
            let transport = match provider.provider_type {
                decision_gate_mcp::config::ProviderType::Builtin => {
                    decision_gate_mcp::tools::ProviderTransport::Builtin
                }
                decision_gate_mcp::config::ProviderType::Mcp => {
                    decision_gate_mcp::tools::ProviderTransport::Mcp
                }
            };
            (provider.name.clone(), transport)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let schema_registry_limits = decision_gate_mcp::tools::SchemaRegistryLimits {
        max_schema_bytes: config.schema_registry.max_schema_bytes,
        max_entries: config
            .schema_registry
            .max_entries
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
    };
    let authz = Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
    let auth_audit = Arc::new(NoopAuditSink);
    let trust_requirement = config.effective_trust_requirement();
    let allow_default_namespace = config.allow_default_namespace();
    let evidence_policy = config.evidence.clone();
    let validation = config.validation.clone();
    let anchor_policy = config.anchors.to_policy();
    let precheck_audit_payloads = config.server.audit.log_precheck_payloads;
    ToolRouter::new(ToolRouterConfig {
        evidence,
        evidence_policy,
        validation,
        dispatch_policy: config.policy.dispatch_policy().expect("dispatch policy"),
        store,
        schema_registry,
        provider_transports,
        schema_registry_limits,
        capabilities: Arc::new(capabilities),
        authz,
        audit: auth_audit,
        trust_requirement,
        anchor_policy,
        precheck_audit: audit,
        precheck_audit_payloads,
        allow_default_namespace,
        namespace_authority: Arc::new(NoopNamespaceAuthority),
    })
}

fn register_schema(router: &ToolRouter, tenant_id: TenantId, namespace_id: NamespaceId) {
    let record = DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("precheck audit schema".to_string()),
        created_at: Timestamp::Logical(1),
    };
    let request = SchemasRegisterRequest {
        record,
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "schemas_register",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
}

fn define_scenario(router: &ToolRouter) {
    let request = ScenarioDefineRequest {
        spec: sample_spec(),
    };
    let _ = router
        .handle_tool_call(
            &local_request_context(),
            "scenario_define",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
}

fn precheck(router: &ToolRouter, tenant_id: TenantId, namespace_id: NamespaceId) {
    let spec = sample_spec();
    let request = PrecheckToolRequest {
        tenant_id,
        namespace_id,
        scenario_id: Some(spec.scenario_id),
        spec: None,
        stage_id: None,
        data_shape: DataShapeRef {
            schema_id: DataShapeId::new("asserted"),
            version: DataShapeVersion::new("v1"),
        },
        payload: json!({"after": true}),
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "precheck",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    let _response: PrecheckToolResponse = serde_json::from_value(result).unwrap();
}

#[test]
fn precheck_audit_hash_only_by_default() {
    let mut config = common::sample_config();
    config.server.audit.log_precheck_payloads = false;
    let audit = Arc::new(TestAuditSink::default());
    let router = build_router(config, Arc::clone(&audit));
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    define_scenario(&router);
    register_schema(&router, tenant_id.clone(), namespace_id.clone());
    precheck(&router, tenant_id, namespace_id);

    let events = audit.precheck_events.lock().expect("precheck events lock");
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.redaction, "hash_only");
    assert!(event.request.is_none());
    assert!(event.response.is_none());
    assert!(!event.request_hash.value.is_empty());
    assert!(!event.response_hash.value.is_empty());
}

#[test]
fn precheck_audit_payloads_opt_in() {
    let mut config = common::sample_config();
    config.server.audit.log_precheck_payloads = true;
    let audit = Arc::new(TestAuditSink::default());
    let router = build_router(config, Arc::clone(&audit));
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    define_scenario(&router);
    register_schema(&router, tenant_id.clone(), namespace_id.clone());
    precheck(&router, tenant_id, namespace_id);

    let events = audit.precheck_events.lock().expect("precheck events lock");
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.redaction, "payload");
    assert!(event.request.is_some());
    assert!(event.response.is_some());
}
