// system-tests/tests/suites/assetcore_integration.rs
// ============================================================================
// Module: Asset Core Integration Tests
// Description: System-tests for DG + ASC alignment boundaries.
// Purpose: Validate anchor enforcement, namespace authority, and correlation IDs.
// Dependencies: system-tests helpers, decision-gate-core, decision-gate-mcp
// ============================================================================

//! `AssetCore` alignment tests for Decision Gate system-tests.


use std::time::Duration;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::CorrelationId;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::NamespaceId;
use decision_gate_core::PredicateKey;
use decision_gate_core::PredicateSpec;
use decision_gate_core::ProviderId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_mcp::config::AnchorProviderConfig;
use decision_gate_mcp::config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_mcp::config::NamespaceAuthorityConfig;
use decision_gate_mcp::config::NamespaceAuthorityMode;
use decision_gate_mcp::config::PrincipalConfig;
use decision_gate_mcp::config::PrincipalRoleConfig;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::config_with_provider;
use helpers::harness::spawn_mcp_server;
use helpers::namespace_authority_stub::spawn_namespace_authority_stub;
use helpers::provider_stub::ProviderFixture;
use helpers::provider_stub::spawn_provider_fixture_stub;
use helpers::readiness::wait_for_ready;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use serde_json::Value;
use serde_json::json;

use crate::helpers;

const ASSETCORE_PROVIDER_ID: &str = "assetcore_read";
const ASSETCORE_ANCHOR_TYPE: &str = "assetcore.anchor_set";
const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(10);
static ASSETCORE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test(flavor = "multi_thread")]
async fn assetcore_anchor_missing_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = ASSETCORE_TEST_LOCK.lock().expect("assetcore test lock");
    let mut reporter = TestReporter::new("assetcore_anchor_missing_fails_closed")?;
    let fixture = assetcore_fixture("assetcore-anchor-missing", "run-anchor-missing");
    let provider_fixture = ProviderFixture {
        predicate: "slot_occupied".to_string(),
        params: json!({"container_id": "slots-gear", "slot_index": 1}),
        result: json!(true),
        anchor: None,
    };
    let provider = spawn_provider_fixture_stub(vec![provider_fixture]).await?;

    let bind = allocate_bind_addr()?.to_string();
    let provider_contract = fixture_root("assetcore/providers").join("assetcore_read.json");
    let mut config =
        config_with_provider(&bind, ASSETCORE_PROVIDER_ID, provider.base_url(), &provider_contract);
    config.anchors.providers.push(assetcore_anchor_policy());

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, SERVER_READY_TIMEOUT).await?;

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: fixture.trigger(None),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match trigger_result.decision.outcome {
        DecisionOutcome::Hold {
            ..
        } => {}
        other => return Err(format!("expected hold decision, got {other:?}").into()),
    }

    wait_for_ready(
        || async {
            if provider.requests().is_empty() {
                Err("no provider requests captured".to_string())
            } else {
                Ok(())
            }
        },
        Duration::from_secs(5),
        "provider request",
    )
    .await?;
    let provider_requests = provider.requests();

    reporter.artifacts().write_json("assetcore_spec.json", &fixture.spec)?;
    reporter.artifacts().write_json("provider_requests.json", &provider_requests)?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["missing AssetCore anchors fail closed".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "assetcore_spec.json".to_string(),
            "provider_requests.json".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn assetcore_correlation_id_passthrough() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = ASSETCORE_TEST_LOCK.lock().expect("assetcore test lock");
    let mut reporter = TestReporter::new("assetcore_correlation_id_passthrough")?;
    let fixture = assetcore_fixture("assetcore-correlation", "run-correlation");
    let anchor = assetcore_anchor(1, "commit-1", 42);
    let provider_fixture = ProviderFixture {
        predicate: "slot_occupied".to_string(),
        params: json!({"container_id": "slots-gear", "slot_index": 1}),
        result: json!(true),
        anchor: Some(anchor),
    };
    let provider = spawn_provider_fixture_stub(vec![provider_fixture]).await?;

    let bind = allocate_bind_addr()?.to_string();
    let provider_contract = fixture_root("assetcore/providers").join("assetcore_read.json");
    let mut config =
        config_with_provider(&bind, ASSETCORE_PROVIDER_ID, provider.base_url(), &provider_contract);
    config.anchors.providers.push(assetcore_anchor_policy());

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, SERVER_READY_TIMEOUT).await?;

    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id.clone(),
        run_config: fixture.run_config(),
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let _state: decision_gate_core::RunState =
        client.call_tool_typed("scenario_start", start_input).await?;

    let correlation_id = CorrelationId::new("corr-assetcore-1");
    let trigger_request = ScenarioTriggerRequest {
        scenario_id: define_output.scenario_id.clone(),
        trigger: fixture.trigger(Some(correlation_id.clone())),
    };
    let trigger_input = serde_json::to_value(&trigger_request)?;
    let trigger_result: TriggerResult =
        client.call_tool_typed("scenario_trigger", trigger_input).await?;

    match trigger_result.decision.outcome {
        DecisionOutcome::Complete {
            ..
        } => {}
        other => return Err(format!("expected complete decision, got {other:?}").into()),
    }

    wait_for_ready(
        || async {
            if provider.requests().is_empty() {
                Err("no provider requests captured".to_string())
            } else {
                Ok(())
            }
        },
        Duration::from_secs(5),
        "provider request",
    )
    .await?;
    let requests = provider.requests();
    let first = requests.first().ok_or_else(|| "missing provider request".to_string())?;
    if first.correlation_id.as_deref() != Some(correlation_id.as_str()) {
        return Err(format!(
            "expected correlation id {}, got {:?}",
            correlation_id.as_str(),
            first.correlation_id
        )
        .into());
    }
    if first.request_id != Value::String(correlation_id.as_str().to_string()) {
        return Err(format!(
            "expected request id {}, got {:?}",
            correlation_id.as_str(),
            first.request_id
        )
        .into());
    }

    reporter.artifacts().write_json("provider_requests.json", &requests)?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["correlation id forwarded to AssetCore provider".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "provider_requests.json".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn namespace_authority_allows_known_namespace() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = ASSETCORE_TEST_LOCK.lock().expect("assetcore test lock");
    let mut reporter = TestReporter::new("namespace_authority_allows_known_namespace")?;
    let authority = spawn_namespace_authority_stub(vec![1]).await?;

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(assetcore_authority_config(authority.base_url())),
    };

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, SERVER_READY_TIMEOUT).await?;

    let mut fixture = ScenarioFixture::time_after("namespace-allow", "run-allow", 0);
    fixture.spec.namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    fixture.spec.default_tenant_id = Some(TenantId::from_raw(1).expect("nonzero tenantid"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let _define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    reporter.artifacts().write_json("namespace_requests.json", &authority.requests())?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["namespace authority allowed known namespace".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "namespace_requests.json".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn namespace_authority_denies_unknown_namespace() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = ASSETCORE_TEST_LOCK.lock().expect("assetcore test lock");
    let mut reporter = TestReporter::new("namespace_authority_denies_unknown_namespace")?;
    let authority = spawn_namespace_authority_stub(Vec::new()).await?;

    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(assetcore_authority_config(authority.base_url())),
    };

    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, SERVER_READY_TIMEOUT).await?;

    let mut fixture = ScenarioFixture::time_after("namespace-deny", "run-deny", 0);
    fixture.spec.namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    fixture.spec.default_tenant_id = Some(TenantId::from_raw(1).expect("nonzero tenantid"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let error =
        client.call_tool_typed::<ScenarioDefineResponse>("scenario_define", define_input).await;
    if error.is_ok() {
        return Err("expected namespace authority denial".into());
    }
    let err = error.err().unwrap_or_default();
    if !err.contains("unauthorized") {
        return Err(format!("unexpected error: {err}").into());
    }

    let requests = authority.requests();
    if requests.is_empty() {
        return Err("expected namespace authority request".into());
    }
    reporter.artifacts().write_json("namespace_requests.json", &requests)?;
    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["namespace authority denied unknown namespace".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "namespace_requests.json".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn namespace_mismatch_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = ASSETCORE_TEST_LOCK.lock().expect("assetcore test lock");
    let mut reporter = TestReporter::new("namespace_mismatch_rejected")?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    if let Some(auth) = config.server.auth.as_mut() {
        let role = PrincipalRoleConfig {
            name: "TenantAdmin".to_string(),
            tenant_id: Some(TenantId::from_raw(1).expect("nonzero tenantid")),
            namespace_id: Some(NamespaceId::from_raw(2).expect("nonzero namespaceid")),
        };
        if let Some(principal) = auth.principals.iter_mut().find(|p| p.subject == "loopback") {
            principal.roles.push(role);
        } else {
            auth.principals.push(PrincipalConfig {
                subject: "loopback".to_string(),
                policy_class: Some("prod".to_string()),
                roles: vec![role],
            });
        }
    }
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(5))?;
    wait_for_server_ready(&client, SERVER_READY_TIMEOUT).await?;

    let mut fixture = ScenarioFixture::time_after("namespace-mismatch", "run-mismatch", 0);
    fixture.spec.namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    fixture.spec.default_tenant_id = Some(TenantId::from_raw(1).expect("nonzero tenantid"));
    let define_request = ScenarioDefineRequest {
        spec: fixture.spec.clone(),
    };
    let define_input = serde_json::to_value(&define_request)?;
    let define_output: ScenarioDefineResponse =
        client.call_tool_typed("scenario_define", define_input).await?;

    let mut run_config = fixture.run_config();
    run_config.namespace_id = NamespaceId::from_raw(2).expect("nonzero namespaceid");
    let start_request = ScenarioStartRequest {
        scenario_id: define_output.scenario_id,
        run_config,
        started_at: Timestamp::Logical(1),
        issue_entry_packets: false,
    };
    let start_input = serde_json::to_value(&start_request)?;
    let error =
        client.call_tool_typed::<decision_gate_core::RunState>("scenario_start", start_input).await;
    if error.is_ok() {
        return Err("expected namespace mismatch rejection".into());
    }
    let err = error.err().unwrap_or_default();
    if !err.contains("namespace mismatch") {
        return Err(format!("unexpected error: {err}").into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &client.transcript())?;
    reporter.finish(
        "pass",
        vec!["namespace mismatch rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

fn assetcore_anchor_policy() -> AnchorProviderConfig {
    AnchorProviderConfig {
        provider_id: ASSETCORE_PROVIDER_ID.to_string(),
        anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
        required_fields: vec![
            "assetcore.namespace_id".to_string(),
            "assetcore.commit_id".to_string(),
            "assetcore.world_seq".to_string(),
        ],
    }
}

fn assetcore_anchor(namespace_id: u64, commit_id: &str, world_seq: u64) -> EvidenceAnchor {
    let anchor_value = json!({
        "assetcore.namespace_id": namespace_id,
        "assetcore.commit_id": commit_id,
        "assetcore.world_seq": world_seq
    });
    EvidenceAnchor {
        anchor_type: ASSETCORE_ANCHOR_TYPE.to_string(),
        anchor_value: serde_json::to_string(&anchor_value).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn assetcore_fixture(scenario: &str, run: &str) -> AssetcoreFixture {
    let scenario_id = ScenarioId::new(scenario);
    let namespace_id = NamespaceId::from_raw(11).expect("nonzero namespaceid");
    let stage_id = StageId::new("stage-1");
    let predicate_key = PredicateKey::new("slot_occupied");
    let spec = ScenarioSpec {
        scenario_id: scenario_id.clone(),
        namespace_id: namespace_id.clone(),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id,
            entry_packets: Vec::new(),
            gates: vec![GateSpec {
                gate_id: GateId::new("gate-1"),
                requirement: ret_logic::Requirement::predicate(predicate_key.clone()),
                trust: None,
            }],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        predicates: vec![PredicateSpec {
            predicate: predicate_key,
            query: EvidenceQuery {
                provider_id: ProviderId::new(ASSETCORE_PROVIDER_ID),
                predicate: "slot_occupied".to_string(),
                params: Some(json!({"container_id": "slots-gear", "slot_index": 1})),
            },
            comparator: Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        }],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };
    AssetcoreFixture {
        scenario_id,
        run_id: RunId::new(run),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id,
        spec,
    }
}

fn assetcore_authority_config(base_url: &str) -> AssetCoreNamespaceAuthorityConfig {
    AssetCoreNamespaceAuthorityConfig {
        base_url: base_url.to_string(),
        auth_token: None,
        connect_timeout_ms: 500,
        request_timeout_ms: 2000,
    }
}

fn fixture_root(path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(path)
}

struct AssetcoreFixture {
    scenario_id: ScenarioId,
    run_id: RunId,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    spec: ScenarioSpec,
}

impl AssetcoreFixture {
    fn run_config(&self) -> RunConfig {
        RunConfig {
            tenant_id: self.tenant_id.clone(),
            namespace_id: self.namespace_id.clone(),
            run_id: self.run_id.clone(),
            scenario_id: self.scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        }
    }

    fn trigger(&self, correlation_id: Option<CorrelationId>) -> TriggerEvent {
        TriggerEvent {
            run_id: self.run_id.clone(),
            tenant_id: self.tenant_id.clone(),
            namespace_id: self.namespace_id.clone(),
            trigger_id: TriggerId::new("trigger-1"),
            kind: TriggerKind::ExternalEvent,
            time: Timestamp::Logical(2),
            source_id: "assetcore-test".to_string(),
            payload: None,
            correlation_id,
        }
    }
}
