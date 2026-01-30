// system-tests/tests/suites/auth_matrix.rs
// ============================================================================
// Module: Auth Mapping Matrix Tests
// Description: Validate ASC role-to-DG tool mapping via proxy layer.
// Purpose: Ensure integration-layer auth mapping is deterministic and fail-closed.
// Dependencies: system-tests helpers, decision-gate-mcp
// ============================================================================

//! Auth mapping matrix tests for DG + ASC alignment.


use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use decision_gate_contract::ToolName;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::EvidenceContext;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_core::runtime::SubmitRequest;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::ProvidersListRequest;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackVerifyRequest;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioNextRequest;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioStatusRequest;
use decision_gate_mcp::tools::ScenarioSubmitRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::ScenariosListRequest;
use decision_gate_mcp::tools::SchemasGetRequest;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::auth_proxy::AscRole;
use helpers::auth_proxy::POLICY_CLASS_HEADER;
use helpers::auth_proxy::PolicyClass;
use helpers::auth_proxy::ROLE_HEADER;
use helpers::auth_proxy::allowed_tools_for_roles;
use helpers::auth_proxy::policy_class_to_header;
use helpers::auth_proxy::roles_to_header;
use helpers::auth_proxy::spawn_auth_proxy;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::readiness::wait_for_ready;
use helpers::scenarios::ScenarioFixture;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::helpers;

#[tokio::test(flavor = "multi_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "Auth matrix setup is clearer as a single end-to-end test."
)]
async fn asc_auth_mapping_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("asc_auth_mapping_matrix")?;
    let bind = allocate_bind_addr()?.to_string();
    let config = base_http_config(&bind);
    let server = spawn_mcp_server(config).await?;
    let proxy = spawn_auth_proxy(server.base_url().to_string()).await?;
    let proxy_url = format!("{}/rpc", proxy.base_url());

    let admin_roles = vec![AscRole::TenantAdmin];
    let admin_policy = PolicyClass::Prod;
    let proxy_url_ready = proxy_url.clone();
    let admin_roles_ready = admin_roles.clone();
    wait_for_ready(
        || {
            let proxy_url = proxy_url_ready.clone();
            let roles = admin_roles_ready.clone();
            async move {
                let mut probe = ProxyClient::new(&proxy_url, &roles, admin_policy)
                    .map_err(|err| err.to_string())?;
                probe.list_tools().await.map(|_| ())
            }
        },
        Duration::from_secs(5),
        "auth proxy",
    )
    .await?;

    let mut admin = ProxyClient::new(&proxy_url, &admin_roles, admin_policy)?;

    let mut admin_fixture = ScenarioFixture::time_after("admin-scenario", "run-admin", 0);
    admin_fixture.spec.default_tenant_id = Some(admin_fixture.tenant_id);
    let admin_define = ScenarioDefineRequest {
        spec: admin_fixture.spec.clone(),
    };
    let admin_define_input = serde_json::to_value(&admin_define)?;
    let admin_define_output: ScenarioDefineResponse =
        admin.call_tool_typed("scenario_define", admin_define_input).await?;

    let schema_record = DataShapeRecord {
        tenant_id: admin_fixture.tenant_id,
        namespace_id: admin_fixture.namespace_id,
        schema_id: DataShapeId::new("asserted"),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("auth matrix schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let schema_request = SchemasRegisterRequest {
        record: schema_record.clone(),
    };
    admin
        .call_tool_typed::<serde_json::Value>(
            "schemas_register",
            serde_json::to_value(&schema_request)?,
        )
        .await?;

    let start_request = ScenarioStartRequest {
        scenario_id: admin_define_output.scenario_id.clone(),
        run_config: admin_fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    admin
        .call_tool_typed::<decision_gate_core::RunState>(
            "scenario_start",
            serde_json::to_value(&start_request)?,
        )
        .await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: admin_define_output.scenario_id.clone(),
        trigger: admin_fixture.trigger_event("trigger-1", Timestamp::Logical(2)),
    };
    admin
        .call_tool_typed::<decision_gate_core::runtime::TriggerResult>(
            "scenario_trigger",
            serde_json::to_value(&trigger_request)?,
        )
        .await?;

    let runpack_dir = reporter.artifacts().runpack_dir();
    let runpack_request = RunpackExportRequest {
        scenario_id: admin_define_output.scenario_id.clone(),
        tenant_id: admin_fixture.tenant_id,
        namespace_id: admin_fixture.namespace_id,
        run_id: admin_fixture.run_id.clone(),
        output_dir: Some(runpack_dir.to_string_lossy().to_string()),
        manifest_name: Some("manifest.json".to_string()),
        generated_at: Timestamp::Logical(5),
        include_verification: true,
    };
    admin
        .call_tool_typed::<decision_gate_mcp::tools::RunpackExportResponse>(
            "runpack_export",
            serde_json::to_value(&runpack_request)?,
        )
        .await?;

    let cases = vec![
        RoleCase::new("tenant_admin", vec![AscRole::TenantAdmin], PolicyClass::Prod),
        RoleCase::new("namespace_owner", vec![AscRole::NamespaceOwner], PolicyClass::Prod),
        RoleCase::new("namespace_admin", vec![AscRole::NamespaceAdmin], PolicyClass::Prod),
        RoleCase::new("namespace_writer", vec![AscRole::NamespaceWriter], PolicyClass::Prod),
        RoleCase::new("namespace_reader", vec![AscRole::NamespaceReader], PolicyClass::Prod),
        RoleCase::new("schema_manager", vec![AscRole::SchemaManager], PolicyClass::Project),
        RoleCase::new("agent_sandbox", vec![AscRole::AgentSandbox], PolicyClass::Scratch),
        RoleCase::new(
            "namespace_delete_admin",
            vec![AscRole::NamespaceDeleteAdmin],
            PolicyClass::Prod,
        ),
        RoleCase::new("schema_manager_prod", vec![AscRole::SchemaManager], PolicyClass::Prod),
        RoleCase::new("agent_sandbox_project", vec![AscRole::AgentSandbox], PolicyClass::Project),
    ];

    let mut transcripts: Vec<TranscriptEntry> = Vec::new();
    for case in cases {
        let mut client = ProxyClient::new(&proxy_url, &case.roles, case.policy_class)?;
        let allowed = allowed_tools_for_roles(&case.roles_set(), case.policy_class);
        let context = RoleContext {
            scenario_id: admin_define_output.scenario_id.clone(),
            run_config: admin_fixture.run_config(),
            run_id: admin_fixture.run_id.clone(),
            tenant_id: admin_fixture.tenant_id,
            namespace_id: admin_fixture.namespace_id,
            schema_record: schema_record.clone(),
            runpack_dir: runpack_dir.clone(),
            role_label: case.label.clone(),
        };
        exercise_mapping(&mut client, &context, &allowed).await?;
        transcripts.extend(client.transcript());
    }

    reporter.artifacts().write_json("tool_transcript.json", &transcripts)?;
    reporter.finish(
        "pass",
        vec!["ASC auth mapping matrix validated".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
            "runpack/".to_string(),
        ],
    )?;
    drop(reporter);
    Ok(())
}

struct RoleCase {
    label: String,
    roles: Vec<AscRole>,
    policy_class: PolicyClass,
}

impl RoleCase {
    fn new(label: &str, roles: Vec<AscRole>, policy_class: PolicyClass) -> Self {
        Self {
            label: label.to_string(),
            roles,
            policy_class,
        }
    }

    fn roles_set(&self) -> BTreeSet<AscRole> {
        self.roles.iter().copied().collect()
    }
}

struct RoleContext {
    scenario_id: decision_gate_core::ScenarioId,
    run_config: decision_gate_core::RunConfig,
    run_id: decision_gate_core::RunId,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    schema_record: DataShapeRecord,
    runpack_dir: PathBuf,
    role_label: String,
}

#[allow(
    clippy::too_many_lines,
    reason = "Role mapping exercise keeps the sequence in one helper for clarity."
)]
async fn exercise_mapping(
    client: &mut ProxyClient,
    context: &RoleContext,
    allowed: &BTreeSet<ToolName>,
) -> Result<(), Box<dyn std::error::Error>> {
    if allowed.contains(&ToolName::ScenarioDefine) {
        let mut fixture =
            ScenarioFixture::time_after(&format!("role-{}", context.role_label), "run-role", 0);
        fixture.spec.default_tenant_id = Some(context.tenant_id);
        let request = ScenarioDefineRequest {
            spec: fixture.spec,
        };
        client
            .call_tool_typed::<ScenarioDefineResponse>(
                "scenario_define",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        let mut fixture =
            ScenarioFixture::time_after(&format!("deny-{}", context.role_label), "run-deny", 0);
        fixture.spec.default_tenant_id = Some(context.tenant_id);
        let request = ScenarioDefineRequest {
            spec: fixture.spec,
        };
        expect_unauthorized(
            client.call_tool("scenario_define", serde_json::to_value(&request)?).await,
        )?;
    }

    if allowed.contains(&ToolName::ProvidersList) {
        let request = ProvidersListRequest {};
        client
            .call_tool_typed::<decision_gate_mcp::tools::ProvidersListResponse>(
                "providers_list",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        expect_unauthorized(
            client
                .call_tool("providers_list", serde_json::to_value(&ProvidersListRequest {})?)
                .await,
        )?;
    }

    if allowed.contains(&ToolName::SchemasList) {
        let request = SchemasListRequest {
            tenant_id: context.tenant_id,
            namespace_id: context.namespace_id,
            cursor: None,
            limit: Some(20),
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::SchemasListResponse>(
                "schemas_list",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        expect_unauthorized(
            client
                .call_tool(
                    "schemas_list",
                    serde_json::to_value(&SchemasListRequest {
                        tenant_id: context.tenant_id,
                        namespace_id: context.namespace_id,
                        cursor: None,
                        limit: Some(20),
                    })?,
                )
                .await,
        )?;
    }

    if allowed.contains(&ToolName::SchemasGet) {
        let request = SchemasGetRequest {
            tenant_id: context.tenant_id,
            namespace_id: context.namespace_id,
            schema_id: context.schema_record.schema_id.clone(),
            version: context.schema_record.version.clone(),
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::SchemasGetResponse>(
                "schemas_get",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        expect_unauthorized(
            client
                .call_tool(
                    "schemas_get",
                    serde_json::to_value(&SchemasGetRequest {
                        tenant_id: context.tenant_id,
                        namespace_id: context.namespace_id,
                        schema_id: context.schema_record.schema_id.clone(),
                        version: context.schema_record.version.clone(),
                    })?,
                )
                .await,
        )?;
    }

    if allowed.contains(&ToolName::SchemasRegister) {
        let mut record = context.schema_record.clone();
        record.schema_id = DataShapeId::new(format!("role-{}", context.role_label));
        let request = SchemasRegisterRequest {
            record,
        };
        client
            .call_tool_typed::<serde_json::Value>(
                "schemas_register",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        let request = SchemasRegisterRequest {
            record: context.schema_record.clone(),
        };
        expect_unauthorized(
            client.call_tool("schemas_register", serde_json::to_value(&request)?).await,
        )?;
    }

    if allowed.contains(&ToolName::ScenariosList) {
        let request = ScenariosListRequest {
            tenant_id: context.tenant_id,
            namespace_id: context.namespace_id,
            cursor: None,
            limit: Some(20),
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::ScenariosListResponse>(
                "scenarios_list",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        expect_unauthorized(
            client
                .call_tool(
                    "scenarios_list",
                    serde_json::to_value(&ScenariosListRequest {
                        tenant_id: context.tenant_id,
                        namespace_id: context.namespace_id,
                        cursor: None,
                        limit: Some(20),
                    })?,
                )
                .await,
        )?;
    }

    if allowed.contains(&ToolName::EvidenceQuery) {
        let request = decision_gate_mcp::tools::EvidenceQueryRequest {
            query: decision_gate_core::EvidenceQuery {
                provider_id: decision_gate_core::ProviderId::new("time"),
                check_id: "after".to_string(),
                params: Some(json!({"timestamp": 0})),
            },
            context: EvidenceContext {
                tenant_id: context.tenant_id,
                namespace_id: context.namespace_id,
                run_id: context.run_id.clone(),
                scenario_id: context.scenario_id.clone(),
                stage_id: decision_gate_core::StageId::new("stage-1"),
                trigger_id: TriggerId::new("trigger-ev"),
                trigger_time: Timestamp::Logical(2),
                correlation_id: None,
            },
        };
        client
            .call_tool_typed::<decision_gate_core::EvidenceResult>(
                "evidence_query",
                serde_json::to_value(&request)?,
            )
            .await?;
    } else {
        let request = decision_gate_mcp::tools::EvidenceQueryRequest {
            query: decision_gate_core::EvidenceQuery {
                provider_id: decision_gate_core::ProviderId::new("time"),
                check_id: "after".to_string(),
                params: Some(json!({"timestamp": 0})),
            },
            context: EvidenceContext {
                tenant_id: context.tenant_id,
                namespace_id: context.namespace_id,
                run_id: context.run_id.clone(),
                scenario_id: context.scenario_id.clone(),
                stage_id: decision_gate_core::StageId::new("stage-1"),
                trigger_id: TriggerId::new("trigger-ev"),
                trigger_time: Timestamp::Logical(2),
                correlation_id: None,
            },
        };
        expect_unauthorized(
            client.call_tool("evidence_query", serde_json::to_value(&request)?).await,
        )?;
    }

    if allowed.contains(&ToolName::ScenarioStart) {
        let run_id = decision_gate_core::RunId::new(format!("run-{}", context.role_label));
        let mut run_config = context.run_config.clone();
        run_config.run_id = run_id.clone();
        let start_request = ScenarioStartRequest {
            scenario_id: context.scenario_id.clone(),
            run_config: run_config.clone(),
            started_at: Timestamp::Logical(3),
            issue_entry_packets: false,
        };
        client
            .call_tool_typed::<decision_gate_core::RunState>(
                "scenario_start",
                serde_json::to_value(&start_request)?,
            )
            .await?;

        if allowed.contains(&ToolName::ScenarioTrigger) {
            let trigger = decision_gate_core::TriggerEvent {
                run_id: run_id.clone(),
                tenant_id: run_config.tenant_id,
                namespace_id: run_config.namespace_id,
                trigger_id: TriggerId::new(format!("trigger-{}", context.role_label)),
                kind: TriggerKind::ExternalEvent,
                time: Timestamp::Logical(4),
                source_id: "auth-matrix".to_string(),
                payload: None,
                correlation_id: None,
            };
            let trigger_request = ScenarioTriggerRequest {
                scenario_id: context.scenario_id.clone(),
                trigger,
            };
            client
                .call_tool_typed::<decision_gate_core::runtime::TriggerResult>(
                    "scenario_trigger",
                    serde_json::to_value(&trigger_request)?,
                )
                .await?;
        }

        if allowed.contains(&ToolName::ScenarioNext) {
            let next_request = ScenarioNextRequest {
                scenario_id: context.scenario_id.clone(),
                request: NextRequest {
                    run_id: run_id.clone(),
                    tenant_id: run_config.tenant_id,
                    namespace_id: run_config.namespace_id,
                    trigger_id: TriggerId::new(format!("next-{}", context.role_label)),
                    agent_id: "auth-matrix".to_string(),
                    time: Timestamp::Logical(5),
                    correlation_id: None,
                },
            };
            client
                .call_tool_typed::<decision_gate_core::runtime::NextResult>(
                    "scenario_next",
                    serde_json::to_value(&next_request)?,
                )
                .await?;
        }

        if allowed.contains(&ToolName::ScenarioSubmit) {
            let submit_request = ScenarioSubmitRequest {
                scenario_id: context.scenario_id.clone(),
                request: SubmitRequest {
                    run_id: run_id.clone(),
                    tenant_id: run_config.tenant_id,
                    namespace_id: run_config.namespace_id,
                    submission_id: format!("submission-{}", context.role_label),
                    payload: decision_gate_core::PacketPayload::Json {
                        value: json!({"artifact": "value"}),
                    },
                    content_type: "application/json".to_string(),
                    submitted_at: Timestamp::Logical(6),
                    correlation_id: None,
                },
            };
            client
                .call_tool_typed::<decision_gate_core::runtime::SubmitResult>(
                    "scenario_submit",
                    serde_json::to_value(&submit_request)?,
                )
                .await?;
        }

        if allowed.contains(&ToolName::ScenarioStatus) {
            let status_request = ScenarioStatusRequest {
                scenario_id: context.scenario_id.clone(),
                request: StatusRequest {
                    run_id: run_id.clone(),
                    tenant_id: run_config.tenant_id,
                    namespace_id: run_config.namespace_id,
                    requested_at: Timestamp::Logical(7),
                    correlation_id: None,
                },
            };
            client
                .call_tool_typed::<decision_gate_core::runtime::ScenarioStatus>(
                    "scenario_status",
                    serde_json::to_value(&status_request)?,
                )
                .await?;
        }

        if allowed.contains(&ToolName::RunpackExport) {
            let runpack_dir = context.runpack_dir.join(format!("runpack-{}", context.role_label));
            let export_request = RunpackExportRequest {
                scenario_id: context.scenario_id.clone(),
                tenant_id: run_config.tenant_id,
                namespace_id: run_config.namespace_id,
                run_id: run_id.clone(),
                output_dir: Some(runpack_dir.to_string_lossy().to_string()),
                manifest_name: Some("manifest.json".to_string()),
                generated_at: Timestamp::Logical(8),
                include_verification: true,
            };
            client
                .call_tool_typed::<decision_gate_mcp::tools::RunpackExportResponse>(
                    "runpack_export",
                    serde_json::to_value(&export_request)?,
                )
                .await?;
        } else {
            let export_request = RunpackExportRequest {
                scenario_id: context.scenario_id.clone(),
                tenant_id: run_config.tenant_id,
                namespace_id: run_config.namespace_id,
                run_id: run_id.clone(),
                output_dir: Some(context.runpack_dir.to_string_lossy().to_string()),
                manifest_name: Some("manifest.json".to_string()),
                generated_at: Timestamp::Logical(8),
                include_verification: false,
            };
            expect_unauthorized(
                client.call_tool("runpack_export", serde_json::to_value(&export_request)?).await,
            )?;
        }
    } else {
        let start_request = ScenarioStartRequest {
            scenario_id: context.scenario_id.clone(),
            run_config: context.run_config.clone(),
            started_at: Timestamp::Logical(3),
            issue_entry_packets: false,
        };
        expect_unauthorized(
            client.call_tool("scenario_start", serde_json::to_value(&start_request)?).await,
        )?;
    }

    if allowed.contains(&ToolName::Precheck) {
        let precheck_request = PrecheckToolRequest {
            tenant_id: context.tenant_id,
            namespace_id: context.namespace_id,
            scenario_id: Some(context.scenario_id.clone()),
            spec: None,
            stage_id: None,
            data_shape: DataShapeRef {
                schema_id: context.schema_record.schema_id.clone(),
                version: context.schema_record.version.clone(),
            },
            payload: json!({"after": true}),
        };
        client
            .call_tool_typed::<decision_gate_mcp::tools::PrecheckToolResponse>(
                "precheck",
                serde_json::to_value(&precheck_request)?,
            )
            .await?;
    } else {
        let precheck_request = PrecheckToolRequest {
            tenant_id: context.tenant_id,
            namespace_id: context.namespace_id,
            scenario_id: Some(context.scenario_id.clone()),
            spec: None,
            stage_id: None,
            data_shape: DataShapeRef {
                schema_id: context.schema_record.schema_id.clone(),
                version: context.schema_record.version.clone(),
            },
            payload: json!({"after": true}),
        };
        expect_unauthorized(
            client.call_tool("precheck", serde_json::to_value(&precheck_request)?).await,
        )?;
    }

    if allowed.contains(&ToolName::ScenarioStatus) {
        let status_request = ScenarioStatusRequest {
            scenario_id: context.scenario_id.clone(),
            request: StatusRequest {
                run_id: context.run_id.clone(),
                tenant_id: context.tenant_id,
                namespace_id: context.namespace_id,
                requested_at: Timestamp::Logical(9),
                correlation_id: None,
            },
        };
        client
            .call_tool_typed::<decision_gate_core::runtime::ScenarioStatus>(
                "scenario_status",
                serde_json::to_value(&status_request)?,
            )
            .await?;
    }

    let verify_request = RunpackVerifyRequest {
        runpack_dir: context.runpack_dir.to_string_lossy().to_string(),
        manifest_path: "manifest.json".to_string(),
    };
    if allowed.contains(&ToolName::RunpackVerify) {
        client
            .call_tool_typed::<decision_gate_mcp::tools::RunpackVerifyResponse>(
                "runpack_verify",
                serde_json::to_value(&verify_request)?,
            )
            .await?;
    } else {
        expect_unauthorized(
            client.call_tool("runpack_verify", serde_json::to_value(&verify_request)?).await,
        )?;
    }

    Ok(())
}

fn expect_unauthorized(result: Result<Value, String>) -> Result<(), Box<dyn std::error::Error>> {
    match result {
        Ok(_) => Err("expected unauthorized error".into()),
        Err(err) => {
            if !err.contains("unauthorized") && !err.contains("unauthenticated") {
                return Err(format!("unexpected error: {err}").into());
            }
            Ok(())
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ToolCallResult {
    content: Vec<ToolContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    Json { json: Value },
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptEntry {
    sequence: u64,
    method: String,
    request: Value,
    response: Value,
    error: Option<String>,
}

struct ProxyClient {
    base_url: String,
    client: reqwest::Client,
    roles_header: String,
    policy_header: &'static str,
    transcript: Vec<TranscriptEntry>,
}

impl ProxyClient {
    fn new(
        base_url: &str,
        roles: &[AscRole],
        policy_class: PolicyClass,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?,
            roles_header: roles_to_header(roles),
            policy_header: policy_class_to_header(policy_class),
            transcript: Vec::new(),
        })
    }

    async fn list_tools(&mut self) -> Result<Value, String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/list".to_string(),
            params: None,
        };
        self.send_request(&request).await
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/call".to_string(),
            params: Some(params),
        };
        let response = self.send_request(&request).await?;
        let parsed: ToolCallResult = serde_json::from_value(response)
            .map_err(|err| format!("invalid tools/call payload: {err}"))?;
        parsed
            .content
            .into_iter()
            .map(|item| match item {
                ToolContent::Json {
                    json,
                } => json,
            })
            .next()
            .ok_or_else(|| "tool returned no json".to_string())
    }

    async fn call_tool_typed<T: for<'de> Deserialize<'de>>(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<T, String> {
        let json = self.call_tool(name, arguments).await?;
        serde_json::from_value(json).map_err(|err| format!("decode {name} response: {err}"))
    }

    async fn send_request(&mut self, request: &JsonRpcRequest) -> Result<Value, String> {
        let request_value =
            serde_json::to_value(request).map_err(|err| format!("serialize jsonrpc: {err}"))?;
        let response = self
            .client
            .post(&self.base_url)
            .header(ROLE_HEADER, &self.roles_header)
            .header(POLICY_CLASS_HEADER, self.policy_header)
            .json(&request_value)
            .send()
            .await
            .map_err(|err| format!("http request failed: {err}"))?;
        let payload: JsonRpcResponse =
            response.json().await.map_err(|err| format!("invalid json-rpc response: {err}"))?;
        let error_message = payload.error.as_ref().map(|err| err.message.clone());
        self.record_transcript(
            request_value,
            serde_json::to_value(&payload).unwrap_or(Value::Null),
            error_message,
        );
        if let Some(error) = payload.error {
            return Err(error.message);
        }
        payload.result.ok_or_else(|| "missing result in response".to_string())
    }

    fn record_transcript(&mut self, request: Value, response: Value, error: Option<String>) {
        let sequence = self.transcript.len() as u64 + 1;
        self.transcript.push(TranscriptEntry {
            sequence,
            method: request.get("method").and_then(Value::as_str).unwrap_or("unknown").to_string(),
            request,
            response,
            error,
        });
    }

    fn transcript(&self) -> Vec<TranscriptEntry> {
        self.transcript.clone()
    }
}
