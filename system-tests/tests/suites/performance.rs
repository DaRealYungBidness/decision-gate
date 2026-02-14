// system-tests/tests/suites/performance.rs
// ============================================================================
// Module: Performance Throughput Tests
// Description: Release-profile throughput and latency gates for MCP workflows.
// Purpose: Enforce absolute SLOs with deterministic workload definitions.
// Dependencies: system-tests helpers, decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Release-profile throughput and latency gates for MCP workflows.
//! Purpose: enforce absolute SLOs while preserving deterministic, fail-closed behavior.
//! Invariants:
//! - Workloads use deterministic payloads, IDs, and logical timestamps.
//! - Stress suites validate correctness; this suite owns throughput SLO gates.
//! - SLO failures fail closed and emit auditable artifacts.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TrustLane;
use decision_gate_mcp::MCP_LATENCY_BUCKETS_MS;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::RunStateStoreType;
use decision_gate_mcp::config::SchemaRegistryType;
use decision_gate_mcp::tools::PrecheckToolRequest;
use decision_gate_mcp::tools::PrecheckToolResponse;
use decision_gate_mcp::tools::ScenarioDefineRequest;
use decision_gate_mcp::tools::ScenarioDefineResponse;
use decision_gate_mcp::tools::ScenarioStartRequest;
use decision_gate_mcp::tools::ScenarioTriggerRequest;
use decision_gate_mcp::tools::SchemasListRequest;
use decision_gate_mcp::tools::SchemasListResponse;
use decision_gate_mcp::tools::SchemasRegisterRequest;
use helpers::artifacts::TestReporter;
use helpers::harness::allocate_bind_addr;
use helpers::harness::base_http_config;
use helpers::harness::spawn_mcp_server;
use helpers::mcp_client::McpHttpClient;
use helpers::mcp_client::TranscriptEntry;
use helpers::readiness::wait_for_server_ready;
use helpers::scenarios::ScenarioFixture;
use ret_logic::TriState;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::task::JoinSet;

use crate::helpers;

const PERF_TARGETS_FILE: &str = "perf_targets.toml";
const PERF_TARGETS_SQLITE_FILE: &str = "perf_targets_sqlite.toml";
const PERF_SKIP_SLO_ASSERTS_ENV: &str = "DECISION_GATE_PERF_SKIP_SLO_ASSERTS";
const CORE_TARGET_KEY: &str = "perf_core_mcp_throughput_release";
const PRECHECK_TARGET_KEY: &str = "perf_precheck_throughput_release";
const REGISTRY_TARGET_KEY: &str = "perf_registry_mixed_throughput_release";
const SQLITE_CORE_TARGET_KEY: &str = "perf_sqlite_core_mcp_throughput_release";
const SQLITE_PRECHECK_TARGET_KEY: &str = "perf_sqlite_precheck_throughput_release";
const SQLITE_REGISTRY_TARGET_KEY: &str = "perf_sqlite_registry_mixed_throughput_release";

#[derive(Debug, Clone, Deserialize)]
struct PerfTargetsFile {
    meta: PerfTargetMeta,
    tests: BTreeMap<String, PerfTarget>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PerfTargetMeta {
    version: u64,
    runner_class: String,
    profile: String,
    notes: String,
    #[serde(default = "default_enforcement_mode")]
    enforcement_mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PerfTarget {
    description: String,
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    payload_profile: String,
    min_throughput_rps: f64,
    max_p95_ms: u64,
    max_error_rate: f64,
    #[serde(default = "default_sweep_workers")]
    sweep_workers: Vec<usize>,
    #[serde(default = "default_sqlite_journal_mode")]
    journal_mode: String,
    #[serde(default = "default_sqlite_sync_mode")]
    sync_mode: String,
    #[serde(default = "default_sqlite_busy_timeout_ms")]
    busy_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct HistogramBucket {
    upper_bound_ms: u64,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct LatencyHistogram {
    buckets: Vec<HistogramBucket>,
    overflow_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct AggregateMetrics {
    total_calls: usize,
    successful_calls: usize,
    failed_calls: usize,
    total_duration_ms: u64,
    throughput_rps: f64,
    error_rate: f64,
    p50_latency_ms: u64,
    p95_latency_ms: u64,
    latency_histogram: LatencyHistogram,
}

#[derive(Debug, Clone, Serialize)]
struct ToolMetrics {
    tool: String,
    calls: usize,
    failed_calls: usize,
    total_duration_ms: u64,
    throughput_rps: f64,
    p50_latency_ms: u64,
    p95_latency_ms: u64,
    latency_histogram: LatencyHistogram,
}

#[derive(Debug, Clone, Serialize)]
struct WorkloadConfig {
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    payload_profile: String,
}

#[derive(Debug, Clone, Serialize)]
struct SloThresholds {
    min_throughput_rps: f64,
    max_p95_ms: u64,
    max_error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
struct PerfSummary {
    test_name: String,
    profile: String,
    target_meta: PerfTargetMeta,
    workload: WorkloadConfig,
    thresholds: SloThresholds,
    metrics: AggregateMetrics,
    tools: BTreeMap<String, ToolMetrics>,
    transcript_method_counts: BTreeMap<String, u64>,
    transcript_tool_counts: BTreeMap<String, u64>,
    telemetry_latency_buckets_ms: Vec<u64>,
    slo_enforced: bool,
    slo_violations: Vec<String>,
}

#[derive(Debug, Clone)]
struct CallSample {
    tool: String,
    duration_ms: u64,
    success: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ToolLatencyReport {
    tool: String,
    calls: usize,
    failed_calls: usize,
    p50_latency_ms: u64,
    p95_latency_ms: u64,
    total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ToolLatencyRanking {
    by_p95_desc: Vec<ToolLatencyReport>,
    by_total_duration_desc: Vec<ToolLatencyReport>,
}

struct PerfContext {
    target_meta: PerfTargetMeta,
    target: PerfTarget,
    client: McpHttpClient,
    reporter: TestReporter,
    sqlite_context: Option<SqlitePerfContext>,
}

#[derive(Debug, Clone, Serialize)]
struct SqliteConfigReport {
    run_state_path: String,
    registry_path: String,
    journal_mode: String,
    sync_mode: String,
    busy_timeout_ms: u64,
}

struct SqlitePerfContext {
    _temp_dir: TempDir,
    config: SqliteConfigReport,
}

#[derive(Debug, Clone, Serialize)]
struct SweepResult {
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    total_calls: usize,
    throughput_rps: f64,
    p95_latency_ms: u64,
    error_rate: f64,
}

fn default_enforcement_mode() -> String {
    "fail_closed".to_string()
}

fn default_sweep_workers() -> Vec<usize> {
    vec![1, 4, 8, 16]
}

fn default_sqlite_journal_mode() -> String {
    "wal".to_string()
}

fn default_sqlite_sync_mode() -> String {
    "full".to_string()
}

const fn default_sqlite_busy_timeout_ms() -> u64 {
    5_000
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "run manually with release profile to validate local throughput SLOs"]
async fn perf_core_mcp_throughput_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_perf_context(CORE_TARGET_KEY, None).await?;
    let fixture = ScenarioFixture::time_after("perf-core", "run-0", 0);
    let scenario_id = define_fixture_scenario(&context.client, fixture.clone()).await?;
    let (samples, elapsed) = run_core_workload(
        &context.client,
        scenario_id,
        fixture,
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        "gate",
    )
    .await?;
    finalize_perf_report(&mut context, CORE_TARGET_KEY, samples, elapsed, &[])?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "run manually with release profile to validate local throughput SLOs"]
async fn perf_precheck_throughput_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_perf_context(PRECHECK_TARGET_KEY, Some(TrustLane::Asserted)).await?;
    let mut fixture = ScenarioFixture::time_after("perf-precheck", "run-0", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let scenario_id = define_fixture_scenario(&context.client, fixture.clone()).await?;
    let schema_record =
        perf_schema_record(fixture.tenant_id, fixture.namespace_id, "perf-precheck", "v1");
    register_schema(&context.client, schema_record.clone()).await?;
    let (samples, elapsed) = run_precheck_workload(
        &context.client,
        fixture.tenant_id,
        fixture.namespace_id,
        scenario_id,
        schema_record,
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
    )
    .await?;
    finalize_perf_report(&mut context, PRECHECK_TARGET_KEY, samples, elapsed, &[])?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "run manually with release profile to validate local throughput SLOs"]
async fn perf_registry_mixed_throughput_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_perf_context(REGISTRY_TARGET_KEY, None).await?;
    let fixture = ScenarioFixture::time_after("perf-registry", "run-0", 0);
    let (samples, elapsed) = run_registry_workload(
        &context.client,
        fixture.tenant_id,
        fixture.namespace_id,
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        "gate",
    )
    .await?;
    finalize_perf_report(&mut context, REGISTRY_TARGET_KEY, samples, elapsed, &[])?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "run manually with release profile to validate local sqlite throughput diagnostics"]
async fn perf_sqlite_core_mcp_throughput_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_sqlite_perf_context(SQLITE_CORE_TARGET_KEY, None).await?;
    let fixture = ScenarioFixture::time_after("perf-sqlite-core", "run-0", 0);
    let scenario_id = define_fixture_scenario(&context.client, fixture.clone()).await?;
    let (samples, elapsed) = run_core_workload(
        &context.client,
        scenario_id.clone(),
        fixture.clone(),
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        "gate",
    )
    .await?;
    let sweep = run_core_sweep(
        &context.client,
        scenario_id,
        fixture,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        &context.target.sweep_workers,
    )
    .await?;
    context.reporter.artifacts().write_json("sqlite_sweep.json", &sweep)?;
    finalize_perf_report(
        &mut context,
        SQLITE_CORE_TARGET_KEY,
        samples,
        elapsed,
        &["sqlite_config.json", "sqlite_sweep.json"],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "run manually with release profile to validate local sqlite throughput diagnostics"]
async fn perf_sqlite_precheck_throughput_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context =
        init_sqlite_perf_context(SQLITE_PRECHECK_TARGET_KEY, Some(TrustLane::Asserted)).await?;
    let mut fixture = ScenarioFixture::time_after("perf-sqlite-precheck", "run-0", 0);
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let scenario_id = define_fixture_scenario(&context.client, fixture.clone()).await?;
    let schema_record =
        perf_schema_record(fixture.tenant_id, fixture.namespace_id, "perf-sqlite-precheck", "v1");
    register_schema(&context.client, schema_record.clone()).await?;
    let (samples, elapsed) = run_precheck_workload(
        &context.client,
        fixture.tenant_id,
        fixture.namespace_id,
        scenario_id.clone(),
        schema_record.clone(),
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
    )
    .await?;
    let sweep = run_precheck_sweep(
        &context.client,
        fixture.tenant_id,
        fixture.namespace_id,
        scenario_id,
        schema_record,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        &context.target.sweep_workers,
    )
    .await?;
    context.reporter.artifacts().write_json("sqlite_sweep.json", &sweep)?;
    finalize_perf_report(
        &mut context,
        SQLITE_PRECHECK_TARGET_KEY,
        samples,
        elapsed,
        &["sqlite_config.json", "sqlite_sweep.json"],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "run manually with release profile to validate local sqlite throughput diagnostics"]
async fn perf_sqlite_registry_mixed_throughput_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_sqlite_perf_context(SQLITE_REGISTRY_TARGET_KEY, None).await?;
    let fixture = ScenarioFixture::time_after("perf-sqlite-registry", "run-0", 0);
    let (samples, elapsed) = run_registry_workload(
        &context.client,
        fixture.tenant_id,
        fixture.namespace_id,
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        "gate",
    )
    .await?;
    let sweep = run_registry_sweep(
        &context.client,
        fixture.tenant_id,
        fixture.namespace_id,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        &context.target.sweep_workers,
    )
    .await?;
    context.reporter.artifacts().write_json("sqlite_sweep.json", &sweep)?;
    finalize_perf_report(
        &mut context,
        SQLITE_REGISTRY_TARGET_KEY,
        samples,
        elapsed,
        &["sqlite_config.json", "sqlite_sweep.json"],
    )?;
    Ok(())
}

async fn init_perf_context(
    test_key: &str,
    min_trust_lane: Option<TrustLane>,
) -> Result<PerfContext, Box<dyn std::error::Error>> {
    init_perf_context_with_targets(PERF_TARGETS_FILE, test_key, min_trust_lane, false).await
}

async fn init_sqlite_perf_context(
    test_key: &str,
    min_trust_lane: Option<TrustLane>,
) -> Result<PerfContext, Box<dyn std::error::Error>> {
    init_perf_context_with_targets(PERF_TARGETS_SQLITE_FILE, test_key, min_trust_lane, true).await
}

async fn init_perf_context_with_targets(
    target_file: &str,
    test_key: &str,
    min_trust_lane: Option<TrustLane>,
    sqlite_enabled: bool,
) -> Result<PerfContext, Box<dyn std::error::Error>> {
    let reporter = TestReporter::new(test_key)?;
    let target_file_data = load_perf_targets(target_file)?;
    let target = target_file_data
        .tests
        .get(test_key)
        .cloned()
        .ok_or_else(|| format!("missing performance target `{test_key}` in `{target_file}`"))?;
    let bind = allocate_bind_addr()?.to_string();
    let mut config = base_http_config(&bind);
    if let Some(min_lane) = min_trust_lane {
        config.trust.min_lane = min_lane;
    }
    let sqlite_context = if sqlite_enabled {
        let sqlite = configure_sqlite_backend(&mut config, &target)?;
        reporter.artifacts().write_json("sqlite_config.json", &sqlite.config)?;
        Some(sqlite)
    } else {
        None
    };
    let server = spawn_mcp_server(config).await?;
    let client = server.client(Duration::from_secs(20))?;
    wait_for_server_ready(&client, Duration::from_secs(20)).await?;
    reporter.artifacts().write_json("perf_target.json", &target)?;
    Ok(PerfContext { target_meta: target_file_data.meta, target, client, reporter, sqlite_context })
}

fn load_perf_targets(target_file: &str) -> Result<PerfTargetsFile, Box<dyn std::error::Error>> {
    let path = perf_targets_path(target_file);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    let targets: PerfTargetsFile =
        toml::from_str(&text).map_err(|err| format!("failed parsing {}: {err}", path.display()))?;
    Ok(targets)
}

fn perf_targets_path(target_file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(target_file)
}

async fn define_fixture_scenario(
    client: &McpHttpClient,
    mut fixture: ScenarioFixture,
) -> Result<decision_gate_core::ScenarioId, String> {
    fixture.spec.default_tenant_id = Some(fixture.tenant_id);
    let define_request = ScenarioDefineRequest { spec: fixture.spec };
    let define_input = serde_json::to_value(&define_request)
        .map_err(|err| format!("serialize scenario_define: {err}"))?;
    let define_output: ScenarioDefineResponse = client
        .call_tool_typed("scenario_define", define_input)
        .await
        .map_err(|err| format!("scenario_define failed: {err}"))?;
    Ok(define_output.scenario_id)
}

fn perf_schema_record(
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    schema_id: &str,
    version: &str,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema: json!({
            "type": "object",
            "properties": {
                "after": { "type": "boolean" }
            },
            "required": ["after"]
        }),
        description: Some("performance schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}

async fn register_schema(client: &McpHttpClient, record: DataShapeRecord) -> Result<(), String> {
    let request = SchemasRegisterRequest { record };
    let input = serde_json::to_value(&request)
        .map_err(|err| format!("serialize schemas_register: {err}"))?;
    let _: Value = client
        .call_tool_typed("schemas_register", input)
        .await
        .map_err(|err| format!("schemas_register failed: {err}"))?;
    Ok(())
}

fn sqlite_store_mode_from_target(
    mode: &str,
) -> Result<decision_gate_store_sqlite::SqliteStoreMode, String> {
    match mode.to_ascii_lowercase().as_str() {
        "wal" => Ok(decision_gate_store_sqlite::SqliteStoreMode::Wal),
        "delete" => Ok(decision_gate_store_sqlite::SqliteStoreMode::Delete),
        other => Err(format!("unsupported sqlite journal_mode `{other}`")),
    }
}

fn sqlite_sync_mode_from_target(
    mode: &str,
) -> Result<decision_gate_store_sqlite::SqliteSyncMode, String> {
    match mode.to_ascii_lowercase().as_str() {
        "full" => Ok(decision_gate_store_sqlite::SqliteSyncMode::Full),
        "normal" => Ok(decision_gate_store_sqlite::SqliteSyncMode::Normal),
        other => Err(format!("unsupported sqlite sync_mode `{other}`")),
    }
}

fn configure_sqlite_backend(
    config: &mut decision_gate_mcp::config::DecisionGateConfig,
    target: &PerfTarget,
) -> Result<SqlitePerfContext, Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let run_state_path = temp_dir.path().join("run_state.sqlite");
    let registry_path = temp_dir.path().join("registry.sqlite");
    let journal_mode = sqlite_store_mode_from_target(&target.journal_mode)?;
    let sync_mode = sqlite_sync_mode_from_target(&target.sync_mode)?;
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: Some(run_state_path.clone()),
        busy_timeout_ms: target.busy_timeout_ms,
        journal_mode,
        sync_mode,
        max_versions: None,
    };
    config.schema_registry.registry_type = SchemaRegistryType::Sqlite;
    config.schema_registry.path = Some(registry_path.clone());
    config.schema_registry.busy_timeout_ms = target.busy_timeout_ms;
    config.schema_registry.journal_mode = journal_mode;
    config.schema_registry.sync_mode = sync_mode;
    Ok(SqlitePerfContext {
        _temp_dir: temp_dir,
        config: SqliteConfigReport {
            run_state_path: run_state_path.display().to_string(),
            registry_path: registry_path.display().to_string(),
            journal_mode: target.journal_mode.clone(),
            sync_mode: target.sync_mode.clone(),
            busy_timeout_ms: target.busy_timeout_ms,
        },
    })
}

async fn run_core_workload(
    client: &McpHttpClient,
    scenario_id: decision_gate_core::ScenarioId,
    fixture: ScenarioFixture,
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    workload_label: &str,
) -> Result<(Vec<CallSample>, Duration), String> {
    let warmup_distribution = distribute_iterations(warmup_iterations, workers)?;
    let measure_distribution = distribute_iterations(measure_iterations, workers)?;
    let started = Instant::now();
    let mut joins = JoinSet::new();
    let workload_label = workload_label.to_string();
    for worker_idx in 0..workers {
        let client = client.clone();
        let scenario_id = scenario_id.clone();
        let fixture = fixture.clone();
        let workload_label = workload_label.clone();
        let warmup = warmup_distribution[worker_idx];
        let measured = measure_distribution[worker_idx];
        joins.spawn(async move {
            run_core_worker(
                &client,
                scenario_id,
                fixture,
                worker_idx,
                workers,
                warmup,
                measured,
                &workload_label,
            )
            .await
        });
    }
    let samples = gather_worker_samples(&mut joins).await?;
    Ok((samples, started.elapsed()))
}

async fn run_precheck_workload(
    client: &McpHttpClient,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    scenario_id: decision_gate_core::ScenarioId,
    schema_record: DataShapeRecord,
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
) -> Result<(Vec<CallSample>, Duration), String> {
    let warmup_distribution = distribute_iterations(warmup_iterations, workers)?;
    let measure_distribution = distribute_iterations(measure_iterations, workers)?;
    let started = Instant::now();
    let mut joins = JoinSet::new();
    for worker_idx in 0..workers {
        let client = client.clone();
        let scenario_id = scenario_id.clone();
        let schema_record = schema_record.clone();
        let warmup = warmup_distribution[worker_idx];
        let measured = measure_distribution[worker_idx];
        joins.spawn(async move {
            run_precheck_worker(
                &client,
                tenant_id,
                namespace_id,
                scenario_id,
                schema_record,
                worker_idx,
                warmup,
                measured,
            )
            .await
        });
    }
    let samples = gather_worker_samples(&mut joins).await?;
    Ok((samples, started.elapsed()))
}

async fn run_registry_workload(
    client: &McpHttpClient,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    workload_label: &str,
) -> Result<(Vec<CallSample>, Duration), String> {
    let warmup_distribution = distribute_iterations(warmup_iterations, workers)?;
    let measure_distribution = distribute_iterations(measure_iterations, workers)?;
    let started = Instant::now();
    let mut joins = JoinSet::new();
    let workload_label = workload_label.to_string();
    for worker_idx in 0..workers {
        let client = client.clone();
        let workload_label = workload_label.clone();
        let warmup = warmup_distribution[worker_idx];
        let measured = measure_distribution[worker_idx];
        joins.spawn(async move {
            run_registry_worker(
                &client,
                tenant_id,
                namespace_id,
                worker_idx,
                warmup,
                measured,
                &workload_label,
            )
            .await
        });
    }
    let samples = gather_worker_samples(&mut joins).await?;
    Ok((samples, started.elapsed()))
}

async fn run_core_sweep(
    client: &McpHttpClient,
    scenario_id: decision_gate_core::ScenarioId,
    fixture: ScenarioFixture,
    warmup_iterations: usize,
    measure_iterations: usize,
    workers: &[usize],
) -> Result<Vec<SweepResult>, String> {
    let mut output = Vec::new();
    for worker_count in workers {
        let (samples, elapsed) = run_core_workload(
            client,
            scenario_id.clone(),
            fixture.clone(),
            *worker_count,
            warmup_iterations,
            measure_iterations,
            &format!("sweep-{worker_count:02}"),
        )
        .await?;
        let aggregate = summarize_samples(&samples, elapsed)?;
        output.push(SweepResult {
            workers: *worker_count,
            warmup_iterations,
            measure_iterations,
            total_calls: aggregate.total_calls,
            throughput_rps: aggregate.throughput_rps,
            p95_latency_ms: aggregate.p95_latency_ms,
            error_rate: aggregate.error_rate,
        });
    }
    Ok(output)
}

async fn run_precheck_sweep(
    client: &McpHttpClient,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    scenario_id: decision_gate_core::ScenarioId,
    schema_record: DataShapeRecord,
    warmup_iterations: usize,
    measure_iterations: usize,
    workers: &[usize],
) -> Result<Vec<SweepResult>, String> {
    let mut output = Vec::new();
    for worker_count in workers {
        let (samples, elapsed) = run_precheck_workload(
            client,
            tenant_id,
            namespace_id,
            scenario_id.clone(),
            schema_record.clone(),
            *worker_count,
            warmup_iterations,
            measure_iterations,
        )
        .await?;
        let aggregate = summarize_samples(&samples, elapsed)?;
        output.push(SweepResult {
            workers: *worker_count,
            warmup_iterations,
            measure_iterations,
            total_calls: aggregate.total_calls,
            throughput_rps: aggregate.throughput_rps,
            p95_latency_ms: aggregate.p95_latency_ms,
            error_rate: aggregate.error_rate,
        });
    }
    Ok(output)
}

async fn run_registry_sweep(
    client: &McpHttpClient,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    warmup_iterations: usize,
    measure_iterations: usize,
    workers: &[usize],
) -> Result<Vec<SweepResult>, String> {
    let mut output = Vec::new();
    for worker_count in workers {
        let (samples, elapsed) = run_registry_workload(
            client,
            tenant_id,
            namespace_id,
            *worker_count,
            warmup_iterations,
            measure_iterations,
            &format!("sweep-{worker_count:02}"),
        )
        .await?;
        let aggregate = summarize_samples(&samples, elapsed)?;
        output.push(SweepResult {
            workers: *worker_count,
            warmup_iterations,
            measure_iterations,
            total_calls: aggregate.total_calls,
            throughput_rps: aggregate.throughput_rps,
            p95_latency_ms: aggregate.p95_latency_ms,
            error_rate: aggregate.error_rate,
        });
    }
    Ok(output)
}

async fn run_core_worker(
    client: &McpHttpClient,
    scenario_id: decision_gate_core::ScenarioId,
    fixture: ScenarioFixture,
    worker_idx: usize,
    workers: usize,
    warmup: usize,
    measured: usize,
    workload_label: &str,
) -> Result<Vec<CallSample>, String> {
    let workers_u64 = u64::try_from(workers).map_err(|_| "workers overflow".to_string())?;
    let mut samples = Vec::new();
    let total = warmup.saturating_add(measured);
    for local_idx in 0..total {
        let seq_local =
            u64::try_from(local_idx).map_err(|_| "worker iteration overflow".to_string())?;
        let global_seq = seq_local.saturating_mul(workers_u64).saturating_add(1);
        let run_id = decision_gate_core::RunId::new(format!(
            "perf-core-{workload_label}-{worker_idx:02}-{local_idx:05}"
        ));
        let run_config = decision_gate_core::RunConfig {
            tenant_id: fixture.tenant_id,
            namespace_id: fixture.namespace_id,
            run_id: run_id.clone(),
            scenario_id: scenario_id.clone(),
            dispatch_targets: Vec::new(),
            policy_tags: Vec::new(),
        };
        let start_request = ScenarioStartRequest {
            scenario_id: scenario_id.clone(),
            run_config,
            started_at: Timestamp::Logical(global_seq),
            issue_entry_packets: false,
        };
        let start_input = serde_json::to_value(&start_request)
            .map_err(|err| format!("serialize scenario_start: {err}"))?;
        let start_measured = local_idx >= warmup;
        let _: decision_gate_core::RunState =
            timed_tool_call(client, "scenario_start", start_input, start_measured, &mut samples)
                .await?;

        let trigger_request = ScenarioTriggerRequest {
            scenario_id: scenario_id.clone(),
            trigger: decision_gate_core::TriggerEvent {
                run_id,
                tenant_id: fixture.tenant_id,
                namespace_id: fixture.namespace_id,
                trigger_id: TriggerId::new(format!(
                    "trigger-{workload_label}-{worker_idx:02}-{local_idx:05}"
                )),
                kind: TriggerKind::ExternalEvent,
                time: Timestamp::Logical(global_seq.saturating_add(1)),
                source_id: "perf-core".to_string(),
                payload: None,
                correlation_id: None,
            },
        };
        let trigger_input = serde_json::to_value(&trigger_request)
            .map_err(|err| format!("serialize scenario_trigger: {err}"))?;
        let _: decision_gate_core::runtime::TriggerResult = timed_tool_call(
            client,
            "scenario_trigger",
            trigger_input,
            start_measured,
            &mut samples,
        )
        .await?;
    }
    Ok(samples)
}

async fn run_precheck_worker(
    client: &McpHttpClient,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    scenario_id: decision_gate_core::ScenarioId,
    schema_record: DataShapeRecord,
    worker_idx: usize,
    warmup: usize,
    measured: usize,
) -> Result<Vec<CallSample>, String> {
    let mut samples = Vec::new();
    let total = warmup.saturating_add(measured);
    for local_idx in 0..total {
        let request = PrecheckToolRequest {
            tenant_id,
            namespace_id,
            scenario_id: Some(scenario_id.clone()),
            spec: None,
            stage_id: None,
            data_shape: DataShapeRef {
                schema_id: schema_record.schema_id.clone(),
                version: schema_record.version.clone(),
            },
            payload: json!({
                "after": true,
                "worker": format!("{worker_idx:02}"),
                "iteration": format!("{local_idx:05}")
            }),
        };
        let input =
            serde_json::to_value(&request).map_err(|err| format!("serialize precheck: {err}"))?;
        let is_measured = local_idx >= warmup;
        let response: PrecheckToolResponse =
            timed_tool_call(client, "precheck", input, is_measured, &mut samples).await?;
        match response.decision {
            DecisionOutcome::Complete { .. } => {}
            other => return Err(format!("unexpected precheck decision: {other:?}")),
        }
        if response.gate_evaluations.is_empty() {
            return Err("precheck returned no gate evaluations".to_string());
        }
        if response.gate_evaluations[0].status != TriState::True {
            return Err("precheck returned non-true gate status".to_string());
        }
    }
    Ok(samples)
}

async fn run_registry_worker(
    client: &McpHttpClient,
    tenant_id: decision_gate_core::TenantId,
    namespace_id: decision_gate_core::NamespaceId,
    worker_idx: usize,
    warmup: usize,
    measured: usize,
    workload_label: &str,
) -> Result<Vec<CallSample>, String> {
    let mut samples = Vec::new();
    let total = warmup.saturating_add(measured);
    for local_idx in 0..total {
        let record = perf_schema_record(
            tenant_id,
            namespace_id,
            &format!("perf-registry-{workload_label}-{worker_idx:02}-{local_idx:05}"),
            "v1",
        );
        let register_request = SchemasRegisterRequest { record };
        let register_input = serde_json::to_value(&register_request)
            .map_err(|err| format!("serialize schemas_register: {err}"))?;
        let is_measured = local_idx >= warmup;
        let _: Value =
            timed_tool_call(client, "schemas_register", register_input, is_measured, &mut samples)
                .await?;

        let list_request =
            SchemasListRequest { tenant_id, namespace_id, cursor: None, limit: Some(25) };
        let list_input = serde_json::to_value(&list_request)
            .map_err(|err| format!("serialize schemas_list: {err}"))?;
        let response: SchemasListResponse =
            timed_tool_call(client, "schemas_list", list_input, is_measured, &mut samples).await?;
        if response.items.is_empty() {
            return Err("schemas_list returned empty response during mixed workload".to_string());
        }
    }
    Ok(samples)
}

async fn gather_worker_samples(
    joins: &mut JoinSet<Result<Vec<CallSample>, String>>,
) -> Result<Vec<CallSample>, String> {
    let mut merged = Vec::new();
    while let Some(result) = joins.join_next().await {
        let worker_samples = result
            .map_err(|err| format!("worker join error: {err}"))?
            .map_err(|err| format!("worker execution failed: {err}"))?;
        merged.extend(worker_samples);
    }
    Ok(merged)
}

async fn timed_tool_call<T: for<'de> serde::Deserialize<'de>>(
    client: &McpHttpClient,
    tool_name: &str,
    arguments: Value,
    measured: bool,
    samples: &mut Vec<CallSample>,
) -> Result<T, String> {
    let started = Instant::now();
    let response = client.call_tool_typed(tool_name, arguments).await;
    if measured {
        let duration_ms = duration_to_ms_u64(started.elapsed())?;
        samples.push(CallSample {
            tool: tool_name.to_string(),
            duration_ms,
            success: response.is_ok(),
        });
    }
    response
}

fn duration_to_ms_u64(duration: Duration) -> Result<u64, String> {
    u64::try_from(duration.as_millis()).map_err(|_| "duration milliseconds overflow".to_string())
}

fn distribute_iterations(total: usize, workers: usize) -> Result<Vec<usize>, String> {
    if workers == 0 {
        return Err("workers must be > 0".to_string());
    }
    let base = total / workers;
    let remainder = total % workers;
    let mut distribution = vec![base; workers];
    for idx in 0..remainder {
        if let Some(item) = distribution.get_mut(idx) {
            *item = item.saturating_add(1);
        }
    }
    Ok(distribution)
}

fn build_histogram(latencies: &[u64]) -> LatencyHistogram {
    let mut buckets = Vec::new();
    let mut overflow_count = 0usize;
    let mut counts = vec![0usize; MCP_LATENCY_BUCKETS_MS.len()];
    for latency in latencies {
        let mut matched = false;
        for (idx, upper_bound) in MCP_LATENCY_BUCKETS_MS.iter().enumerate() {
            if latency <= upper_bound {
                if let Some(slot) = counts.get_mut(idx) {
                    *slot = slot.saturating_add(1);
                }
                matched = true;
                break;
            }
        }
        if !matched {
            overflow_count = overflow_count.saturating_add(1);
        }
    }
    for (idx, upper_bound) in MCP_LATENCY_BUCKETS_MS.iter().enumerate() {
        buckets.push(HistogramBucket {
            upper_bound_ms: *upper_bound,
            count: counts.get(idx).copied().unwrap_or(0),
        });
    }
    LatencyHistogram { buckets, overflow_count }
}

fn percentile_ms(latencies: &[u64], percentile: u32) -> Result<u64, String> {
    if latencies.is_empty() {
        return Err("no latency samples available for percentile calculation".to_string());
    }
    if percentile == 0 || percentile > 100 {
        return Err("percentile must be in range 1..=100".to_string());
    }
    let mut sorted = latencies.to_vec();
    sorted.sort_unstable();
    let len = sorted.len();
    let len_u128 = u128::try_from(len).map_err(|_| "latency sample length overflow".to_string())?;
    let percentile_u128 = u128::from(percentile);
    let rank =
        len_u128.saturating_mul(percentile_u128).saturating_add(99).saturating_div(100).max(1);
    let index_u128 = rank.saturating_sub(1);
    let index = usize::try_from(index_u128).map_err(|_| "percentile index overflow".to_string())?;
    sorted.get(index).copied().ok_or_else(|| "percentile index out of bounds".to_string())
}

fn summarize_samples(
    samples: &[CallSample],
    elapsed: Duration,
) -> Result<AggregateMetrics, String> {
    let total_calls = samples.len();
    if total_calls == 0 {
        return Err("measured sample set is empty".to_string());
    }
    let failed_calls = samples.iter().filter(|sample| !sample.success).count();
    let successful_calls = total_calls.saturating_sub(failed_calls);
    let total_duration_ms =
        samples.iter().fold(0u64, |acc, sample| acc.saturating_add(sample.duration_ms));
    let latencies: Vec<u64> = samples.iter().map(|sample| sample.duration_ms).collect();
    let p50_latency_ms = percentile_ms(&latencies, 50)?;
    let p95_latency_ms = percentile_ms(&latencies, 95)?;
    let total_calls_u64 =
        u64::try_from(total_calls).map_err(|_| "total calls overflow".to_string())?;
    let failed_calls_u64 =
        u64::try_from(failed_calls).map_err(|_| "failed calls overflow".to_string())?;
    let total_calls_u32 = u32::try_from(total_calls_u64)
        .map_err(|_| "total calls too large for f64 conversion".to_string())?;
    let failed_calls_u32 = u32::try_from(failed_calls_u64)
        .map_err(|_| "failed calls too large for f64 conversion".to_string())?;
    let throughput_rps = f64::from(total_calls_u32) / elapsed.as_secs_f64().max(0.000_001);
    let error_rate = f64::from(failed_calls_u32) / f64::from(total_calls_u32);
    Ok(AggregateMetrics {
        total_calls,
        successful_calls,
        failed_calls,
        total_duration_ms,
        throughput_rps,
        error_rate,
        p50_latency_ms,
        p95_latency_ms,
        latency_histogram: build_histogram(&latencies),
    })
}

fn build_tool_metrics(
    samples: &[CallSample],
    elapsed: Duration,
) -> Result<BTreeMap<String, ToolMetrics>, String> {
    let mut by_tool: BTreeMap<String, Vec<&CallSample>> = BTreeMap::new();
    for sample in samples {
        by_tool.entry(sample.tool.clone()).or_default().push(sample);
    }
    let mut output = BTreeMap::new();
    for (tool, entries) in by_tool {
        let calls = entries.len();
        let failed_calls = entries.iter().filter(|entry| !entry.success).count();
        let latencies: Vec<u64> = entries.iter().map(|entry| entry.duration_ms).collect();
        let total_duration_ms =
            entries.iter().fold(0u64, |acc, entry| acc.saturating_add(entry.duration_ms));
        let calls_u64 = u64::try_from(calls).map_err(|_| "tool calls overflow".to_string())?;
        let calls_u32 = u32::try_from(calls_u64)
            .map_err(|_| "tool calls too large for f64 conversion".to_string())?;
        let throughput_rps = f64::from(calls_u32) / elapsed.as_secs_f64().max(0.000_001);
        output.insert(
            tool.clone(),
            ToolMetrics {
                tool,
                calls,
                failed_calls,
                total_duration_ms,
                throughput_rps,
                p50_latency_ms: percentile_ms(&latencies, 50)?,
                p95_latency_ms: percentile_ms(&latencies, 95)?,
                latency_histogram: build_histogram(&latencies),
            },
        );
    }
    Ok(output)
}

fn transcript_method_counts(entries: &[TranscriptEntry]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        let value = counts.entry(entry.method.clone()).or_insert(0_u64);
        *value = value.saturating_add(1);
    }
    counts
}

fn transcript_tool_counts(entries: &[TranscriptEntry]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        if entry.method != "tools/call" {
            continue;
        }
        let Some(tool_name) = entry
            .request
            .get("params")
            .and_then(|params| params.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let value = counts.entry(tool_name.to_string()).or_insert(0_u64);
        *value = value.saturating_add(1);
    }
    counts
}

fn evaluate_slo(metrics: &AggregateMetrics, target: &PerfTarget) -> Vec<String> {
    let mut violations = Vec::new();
    if metrics.throughput_rps < target.min_throughput_rps {
        violations.push(format!(
            "throughput {} rps below minimum {} rps",
            metrics.throughput_rps, target.min_throughput_rps
        ));
    }
    if metrics.p95_latency_ms > target.max_p95_ms {
        violations.push(format!(
            "p95 latency {}ms exceeds maximum {}ms",
            metrics.p95_latency_ms, target.max_p95_ms
        ));
    }
    if metrics.error_rate > target.max_error_rate {
        violations.push(format!(
            "error rate {} exceeds maximum {}",
            metrics.error_rate, target.max_error_rate
        ));
    }
    violations
}

fn build_tool_latency_ranking(tools: &BTreeMap<String, ToolMetrics>) -> ToolLatencyRanking {
    let mut reports: Vec<ToolLatencyReport> = tools
        .values()
        .map(|metric| ToolLatencyReport {
            tool: metric.tool.clone(),
            calls: metric.calls,
            failed_calls: metric.failed_calls,
            p50_latency_ms: metric.p50_latency_ms,
            p95_latency_ms: metric.p95_latency_ms,
            total_duration_ms: metric.total_duration_ms,
        })
        .collect();
    let mut by_p95_desc = reports.clone();
    by_p95_desc.sort_by(|left, right| {
        right
            .p95_latency_ms
            .cmp(&left.p95_latency_ms)
            .then_with(|| right.total_duration_ms.cmp(&left.total_duration_ms))
    });
    reports.sort_by(|left, right| right.total_duration_ms.cmp(&left.total_duration_ms));
    ToolLatencyRanking { by_p95_desc, by_total_duration_desc: reports }
}

fn should_enforce_slo() -> bool {
    !matches!(
        std::env::var(PERF_SKIP_SLO_ASSERTS_ENV).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn target_meta_enforces_slo(meta: &PerfTargetMeta) -> bool {
    !meta.enforcement_mode.eq_ignore_ascii_case("report_only")
}

fn profile_name() -> String {
    if cfg!(debug_assertions) { "debug".to_string() } else { "release".to_string() }
}

fn finalize_perf_report(
    context: &mut PerfContext,
    test_name: &str,
    samples: Vec<CallSample>,
    elapsed: Duration,
    extra_artifacts: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let metrics = summarize_samples(&samples, elapsed)?;
    let tools = build_tool_metrics(&samples, elapsed)?;
    let transcript = context.client.transcript();
    let slo_enforced = target_meta_enforces_slo(&context.target_meta) && should_enforce_slo();
    let violations =
        if slo_enforced { evaluate_slo(&metrics, &context.target) } else { Vec::new() };
    let summary = PerfSummary {
        test_name: test_name.to_string(),
        profile: profile_name(),
        target_meta: context.target_meta.clone(),
        workload: WorkloadConfig {
            workers: context.target.workers,
            warmup_iterations: context.target.warmup_iterations,
            measure_iterations: context.target.measure_iterations,
            payload_profile: context.target.payload_profile.clone(),
        },
        thresholds: SloThresholds {
            min_throughput_rps: context.target.min_throughput_rps,
            max_p95_ms: context.target.max_p95_ms,
            max_error_rate: context.target.max_error_rate,
        },
        metrics,
        tools: tools.clone(),
        transcript_method_counts: transcript_method_counts(&transcript),
        transcript_tool_counts: transcript_tool_counts(&transcript),
        telemetry_latency_buckets_ms: MCP_LATENCY_BUCKETS_MS.to_vec(),
        slo_enforced,
        slo_violations: violations.clone(),
    };
    let tool_latency_ranking = build_tool_latency_ranking(&tools);
    context.reporter.artifacts().write_json("perf_summary.json", &summary)?;
    context.reporter.artifacts().write_json("perf_tool_metrics.json", &tool_latency_ranking)?;
    context.reporter.artifacts().write_json("tool_transcript.json", &transcript)?;

    let mut notes = vec![format!(
        "measured {} calls at {} rps",
        summary.metrics.total_calls, summary.metrics.throughput_rps
    )];
    if let Some(sqlite_context) = &context.sqlite_context {
        notes.push(format!(
            "sqlite profile journal_mode={} sync_mode={} busy_timeout_ms={}",
            sqlite_context.config.journal_mode,
            sqlite_context.config.sync_mode,
            sqlite_context.config.busy_timeout_ms
        ));
    }
    if !slo_enforced {
        if target_meta_enforces_slo(&context.target_meta) {
            notes.push(format!("SLO assertions skipped via {}", PERF_SKIP_SLO_ASSERTS_ENV));
        } else {
            notes.push(
                "SLO assertions disabled by target meta enforcement_mode=report_only".to_string(),
            );
        }
    } else if !violations.is_empty() {
        for violation in &violations {
            notes.push(format!("slo_violation: {violation}"));
        }
    }
    let status = if violations.is_empty() { "pass" } else { "fail" };
    let mut artifacts = vec![
        "summary.json".to_string(),
        "summary.md".to_string(),
        "tool_transcript.json".to_string(),
        "perf_summary.json".to_string(),
        "perf_tool_metrics.json".to_string(),
        "perf_target.json".to_string(),
    ];
    for artifact in extra_artifacts {
        artifacts.push((*artifact).to_string());
    }
    context.reporter.finish(status, notes, artifacts)?;

    if !violations.is_empty() {
        return Err(format!("SLO violations: {}", violations.join("; ")).into());
    }
    Ok(())
}
