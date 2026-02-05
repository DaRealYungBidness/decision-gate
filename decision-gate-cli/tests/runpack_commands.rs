// decision-gate-cli/tests/runpack_commands.rs
// ============================================================================
// Module: CLI Runpack Command Tests
// Description: Integration tests for CLI runpack export and verify workflows.
// Purpose: Validate CLI command wiring and offline verification outputs.
// Dependencies: decision-gate-cli binary, decision-gate-core, serde_json
// ============================================================================
//! ## Overview
//! Runs the CLI binary for runpack export and verification using temporary
//! artifacts. These tests ensure the CLI executes deterministic workflows and
//! emits expected status text.
//!
//! Security posture: CLI inputs are untrusted and must fail closed.
//! Threat model: TM-CLI-001 - Unsafe runpack output or verification bypass.

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

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_core::AdvanceTo;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::RunStatus;
use decision_gate_core::RunpackManifest;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::hashing::hash_canonical_json;
use decision_gate_core::runtime::VerificationReport;
use decision_gate_core::runtime::VerificationStatus;
use decision_gate_mcp::DefaultToolAuthz;
use decision_gate_mcp::FederatedEvidenceProvider;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::NoopAuditSink;
use decision_gate_mcp::NoopNamespaceAuthority;
use decision_gate_mcp::NoopTenantAuthorizer;
use decision_gate_mcp::NoopUsageMeter;
use decision_gate_mcp::RequestContext;
use decision_gate_mcp::ToolRouter;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::AnchorPolicyConfig;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::DevConfig;
use decision_gate_mcp::config::DocsConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::NamespaceConfig;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::config::ProviderDiscoveryConfig;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::SchemaRegistryConfig;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::config::ValidationConfig;
use decision_gate_mcp::docs::DocsCatalog;
use decision_gate_mcp::registry_acl::PrincipalResolver;
use decision_gate_mcp::registry_acl::RegistryAcl;
use decision_gate_mcp::tools::RunpackExportRequest;
use decision_gate_mcp::tools::RunpackExportResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::SchemaRegistryLimits;
use decision_gate_mcp::tools::ToolRouterConfig;
use serde_json::Value;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn decision_gate_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_decision-gate"))
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-cli-{label}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let bytes = serde_json::to_vec(value).expect("serialize");
    fs::write(path, bytes).expect("write json");
}

fn assert_pretty_json(path: &Path) {
    let text = fs::read_to_string(path).expect("read pretty json");
    assert!(text.ends_with('\n'), "expected trailing newline in pretty json");
    let trimmed = text.trim();
    if trimmed != "[]" && trimmed != "{}" {
        assert!(text.contains("  "), "expected indentation in pretty json");
    }
}

fn minimal_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn spec_with_default_tenant() -> ScenarioSpec {
    let mut spec = minimal_spec();
    spec.default_tenant_id = Some(TenantId::from_raw(1).expect("nonzero tenantid"));
    spec
}

fn minimal_state(spec: &ScenarioSpec) -> RunState {
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");
    RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
        status: RunStatus::Active,
        dispatch_targets: Vec::new(),
        triggers: Vec::new(),
        gate_evals: Vec::new(),
        decisions: Vec::new(),
        packets: Vec::new(),
        submissions: Vec::new(),
        tool_calls: Vec::new(),
    }
}

fn export_runpack(root: &Path) -> PathBuf {
    let spec = minimal_spec();
    let state = minimal_state(&spec);
    let spec_path = root.join("spec.json");
    let state_path = root.join("state.json");
    write_json(&spec_path, &spec);
    write_json(&state_path, &state);

    let manifest_path = root.join("runpack.json");
    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "export",
            "--spec",
            spec_path.to_string_lossy().as_ref(),
            "--state",
            state_path.to_string_lossy().as_ref(),
            "--output-dir",
            root.to_string_lossy().as_ref(),
            "--manifest-name",
            "runpack.json",
            "--generated-at-unix-ms",
            "1700000000000",
        ])
        .output()
        .expect("runpack export");

    assert!(output.status.success(), "export failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Runpack manifest written"),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(manifest_path.exists(), "manifest not written");
    manifest_path
}

fn export_runpack_with_inputs(root: &Path, spec: &ScenarioSpec, state: &RunState) -> PathBuf {
    let spec_path = root.join("spec.json");
    let state_path = root.join("state.json");
    write_json(&spec_path, spec);
    write_json(&state_path, state);

    let manifest_path = root.join("runpack.json");
    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "export",
            "--spec",
            spec_path.to_string_lossy().as_ref(),
            "--state",
            state_path.to_string_lossy().as_ref(),
            "--output-dir",
            root.to_string_lossy().as_ref(),
            "--manifest-name",
            "runpack.json",
            "--generated-at-unix-ms",
            "1700000000000",
        ])
        .output()
        .expect("runpack export");

    assert!(output.status.success(), "export failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Runpack manifest written"),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(manifest_path.exists(), "manifest not written");
    manifest_path
}

fn read_manifest(path: &Path) -> RunpackManifest {
    let bytes = fs::read(path).expect("read manifest");
    serde_json::from_slice(&bytes).expect("parse manifest")
}

fn assert_manifest_integrity(manifest: &RunpackManifest, output_dir: &Path) {
    for entry in &manifest.integrity.file_hashes {
        let bytes = fs::read(output_dir.join(&entry.path)).expect("read artifact");
        let actual = hash_bytes(manifest.hash_algorithm, &bytes);
        assert_eq!(actual, entry.hash, "hash mismatch for {}", entry.path);
    }
    let root_hash = hash_canonical_json(manifest.hash_algorithm, &manifest.integrity.file_hashes)
        .expect("root hash");
    assert_eq!(root_hash, manifest.integrity.root_hash);
}

fn minimal_mcp_config() -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::from_raw(1).expect("nonzero tenantid")],
            ..NamespaceConfig::default()
        },
        dev: DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,
        source_modified_at: None,
    }
}

fn build_mcp_router(store: SharedRunStateStore) -> ToolRouter {
    let config = minimal_mcp_config();
    let evidence = FederatedEvidenceProvider::from_config(&config).expect("evidence config");
    let capabilities = CapabilityRegistry::from_config(&config).expect("capabilities");
    let schema_registry = SharedDataShapeRegistry::from_registry(InMemoryDataShapeRegistry::new());
    let provider_transports = BTreeMap::new();
    let schema_registry_limits = SchemaRegistryLimits {
        max_schema_bytes: config.schema_registry.max_schema_bytes,
        max_entries: config
            .schema_registry
            .max_entries
            .map(|limit| usize::try_from(limit).expect("schema registry max_entries overflow")),
    };
    let authz = std::sync::Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
    let tenant_authorizer = std::sync::Arc::new(NoopTenantAuthorizer);
    let usage_meter = std::sync::Arc::new(NoopUsageMeter);
    let principal_resolver = PrincipalResolver::from_config(config.server.auth.as_ref());
    let registry_acl = RegistryAcl::new(&config.schema_registry.acl);
    let docs_catalog = DocsCatalog::from_config(&config.docs).expect("docs catalog");
    let default_namespace_tenants: BTreeSet<_> =
        config.namespace.default_tenants.iter().copied().collect();
    ToolRouter::new(ToolRouterConfig {
        evidence,
        evidence_policy: config.evidence.clone(),
        validation: config.validation.clone(),
        dispatch_policy: config.policy.dispatch_policy().expect("dispatch policy"),
        store,
        schema_registry,
        provider_transports,
        schema_registry_limits,
        capabilities: std::sync::Arc::new(capabilities),
        provider_discovery: config.provider_discovery.clone(),
        authz,
        tenant_authorizer,
        usage_meter,
        runpack_storage: None,
        runpack_object_store: None,
        audit: std::sync::Arc::new(NoopAuditSink),
        trust_requirement: config.effective_trust_requirement(),
        anchor_policy: config.anchors.to_policy(),
        provider_trust_overrides: BTreeMap::new(),
        runpack_security_context: None,
        precheck_audit: std::sync::Arc::new(McpNoopAuditSink),
        precheck_audit_payloads: config.server.audit.log_precheck_payloads,
        registry_acl,
        principal_resolver,
        scenario_next_feedback: config.server.feedback.scenario_next.clone(),
        docs_config: config.docs.clone(),
        docs_catalog,
        tools: config.server.tools.clone(),
        docs_provider: None,
        tool_visibility_resolver: None,
        allow_default_namespace: config.allow_default_namespace(),
        default_namespace_tenants,
        namespace_authority: std::sync::Arc::new(NoopNamespaceAuthority),
    })
}

fn handle_tool_call_sync(
    router: &ToolRouter,
    context: &RequestContext,
    name: &str,
    payload: Value,
) -> Value {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(router.handle_tool_call(context, name, payload))
        .expect("tool call")
}

// ============================================================================
// SECTION: Version Tests
// ============================================================================

/// Verifies the version flag prints a version string.
#[test]
fn cli_version_flag_prints_version() {
    let output = Command::new(decision_gate_bin())
        .arg("--version")
        .output()
        .expect("run decision-gate --version");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("decision-gate"));
}

// ============================================================================
// SECTION: Runpack Export Tests
// ============================================================================

/// Verifies runpack export writes a manifest to disk.
#[test]
fn cli_runpack_export_writes_manifest() {
    let root = temp_root("export");
    let manifest_path = export_runpack(&root);
    assert!(manifest_path.exists());
    cleanup(&root);
}

/// Verifies runpack export rejects manifest path traversal.
#[test]
fn cli_runpack_export_rejects_manifest_traversal() {
    let root = temp_root("export-manifest-traversal");
    let spec = minimal_spec();
    let state = minimal_state(&spec);
    let spec_path = root.join("spec.json");
    let state_path = root.join("state.json");
    write_json(&spec_path, &spec);
    write_json(&state_path, &state);

    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "export",
            "--spec",
            spec_path.to_string_lossy().as_ref(),
            "--state",
            state_path.to_string_lossy().as_ref(),
            "--output-dir",
            root.to_string_lossy().as_ref(),
            "--manifest-name",
            "../runpack.json",
            "--generated-at-unix-ms",
            "1700000000000",
        ])
        .output()
        .expect("runpack export traversal");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("runpack sink"), "unexpected stderr: {stderr}");

    cleanup(&root);
}

/// Verifies CLI and MCP runpack exports produce identical hashes for the same inputs.
#[test]
fn cli_and_mcp_runpack_export_produce_same_hashes() {
    let spec = spec_with_default_tenant();
    let state = minimal_state(&spec);

    let cli_root = temp_root("export-cli-vs-mcp-cli");
    let mcp_root = temp_root("export-cli-vs-mcp-mcp");

    let cli_manifest_path = export_runpack_with_inputs(&cli_root, &spec, &state);
    let cli_manifest = read_manifest(&cli_manifest_path);
    assert_manifest_integrity(&cli_manifest, &cli_root);

    let store = SharedRunStateStore::from_store(InMemoryRunStateStore::new());
    store.save(&state).expect("save state");
    let router = build_mcp_router(store);
    let context = RequestContext::stdio().with_server_correlation_id("test-server");

    let define_request = ScenarioDefineRequest {
        spec: spec.clone(),
    };
    handle_tool_call_sync(
        &router,
        &context,
        "scenario_define",
        serde_json::to_value(&define_request).expect("define request"),
    );

    let export_request = RunpackExportRequest {
        scenario_id: spec.scenario_id,
        tenant_id: state.tenant_id,
        namespace_id: state.namespace_id,
        run_id: state.run_id,
        output_dir: Some(mcp_root.to_string_lossy().to_string()),
        manifest_name: Some("runpack.json".to_string()),
        generated_at: Timestamp::UnixMillis(1_700_000_000_000),
        include_verification: false,
    };
    let export_value = handle_tool_call_sync(
        &router,
        &context,
        "runpack_export",
        serde_json::to_value(&export_request).expect("export request"),
    );
    let export_response: RunpackExportResponse =
        serde_json::from_value(export_value).expect("export response");

    let mcp_manifest_path = mcp_root.join("runpack.json");
    let mcp_manifest = read_manifest(&mcp_manifest_path);
    assert_eq!(mcp_manifest, export_response.manifest);
    assert_manifest_integrity(&mcp_manifest, &mcp_root);

    assert_eq!(cli_manifest.integrity, mcp_manifest.integrity);
    assert_eq!(cli_manifest.artifacts, mcp_manifest.artifacts);

    for entry in &cli_manifest.integrity.file_hashes {
        let cli_bytes = fs::read(cli_root.join(&entry.path)).expect("cli artifact");
        let mcp_bytes = fs::read(mcp_root.join(&entry.path)).expect("mcp artifact");
        assert_eq!(cli_bytes, mcp_bytes, "artifact mismatch: {}", entry.path);
    }

    cleanup(&cli_root);
    cleanup(&mcp_root);
}

// ============================================================================
// SECTION: Runpack Verify Tests
// ============================================================================

/// Verifies runpack verification succeeds with JSON output.
#[test]
fn cli_runpack_verify_outputs_json_report() {
    let root = temp_root("verify-json");
    let manifest = export_runpack(&root);

    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "verify",
            "--manifest",
            manifest.to_string_lossy().as_ref(),
            "--format",
            "json",
        ])
        .output()
        .expect("runpack verify");

    assert!(output.status.success());
    let report: VerificationReport = serde_json::from_slice(&output.stdout).expect("parse report");
    assert_eq!(report.status, VerificationStatus::Pass);

    cleanup(&root);
}

/// Verifies runpack verification renders markdown summaries.
#[test]
fn cli_runpack_verify_outputs_markdown_report() {
    let root = temp_root("verify-markdown");
    let manifest = export_runpack(&root);

    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "verify",
            "--manifest",
            manifest.to_string_lossy().as_ref(),
            "--format",
            "markdown",
        ])
        .output()
        .expect("runpack verify markdown");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Decision Gate Runpack Verification"));
    assert!(stdout.contains("Status: pass"));

    cleanup(&root);
}

// ============================================================================
// SECTION: Runpack Pretty Tests
// ============================================================================

/// Verifies runpack pretty output writes formatted JSON artifacts.
#[test]
fn cli_runpack_pretty_outputs_formatted_json() {
    let root = temp_root("pretty");
    let manifest = export_runpack(&root);
    let pretty_dir = root.join("pretty");

    let output = Command::new(decision_gate_bin())
        .args([
            "runpack",
            "pretty",
            "--manifest",
            manifest.to_string_lossy().as_ref(),
            "--output-dir",
            pretty_dir.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("runpack pretty");

    assert!(output.status.success(), "pretty failed: {}", String::from_utf8_lossy(&output.stderr));

    let manifest_out = pretty_dir.join("runpack.json");
    assert!(manifest_out.exists(), "pretty manifest missing");
    assert_pretty_json(&manifest_out);

    let gate_evals = pretty_dir.join("artifacts").join("gate_evals.json");
    assert!(gate_evals.exists(), "pretty gate_evals missing");
    assert_pretty_json(&gate_evals);

    cleanup(&root);
}
