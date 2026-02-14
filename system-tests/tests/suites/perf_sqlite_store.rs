// system-tests/tests/suites/perf_sqlite_store.rs
// ============================================================================
// Module: SQLite Store Performance Suite
// Description: Direct SQLite store contention microbenchmarks for run state and registry paths.
// Purpose: Attribute throughput/latency and DB contention independent from MCP transport overhead.
// Dependencies: decision-gate-store-sqlite, system-tests helpers
// ============================================================================

//! ## Overview
//! Direct SQLite store contention microbenchmarks for run-state and registry operations.
//! Purpose: expose low-level throughput, latency, and contention signals for local diagnosis.
//! Invariants:
//! - Inputs and IDs are deterministic.
//! - Workload distribution and sweep tiers are deterministic.
//! - SLO checks can run in report-only mode via target metadata.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use decision_gate_core::AdvanceTo;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_store_sqlite::SqlitePerfStatsSnapshot;
use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use helpers::artifacts::TestReporter;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::helpers;

const PERF_TARGETS_SQLITE_FILE: &str = "perf_targets_sqlite.toml";
const PERF_SKIP_SLO_ASSERTS_ENV: &str = "DECISION_GATE_PERF_SKIP_SLO_ASSERTS";
const RUN_STATE_TARGET_KEY: &str = "perf_sqlite_store_run_state_contention_release";
const REGISTRY_TARGET_KEY: &str = "perf_sqlite_store_registry_contention_release";

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
    #[serde(default = "default_sweep_workers")]
    sweep_workers: Vec<usize>,
    payload_profile: String,
    min_throughput_rps: f64,
    max_p95_ms: u64,
    max_error_rate: f64,
    #[serde(default = "default_sqlite_journal_mode")]
    journal_mode: String,
    #[serde(default = "default_sqlite_sync_mode")]
    sync_mode: String,
    #[serde(default = "default_sqlite_busy_timeout_ms")]
    busy_timeout_ms: u64,
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
}

#[derive(Debug, Clone, Serialize)]
struct PerfSummary {
    test_name: String,
    profile: String,
    target_meta: PerfTargetMeta,
    workload: PerfTarget,
    metrics: AggregateMetrics,
    tools: BTreeMap<String, ToolMetrics>,
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
struct SweepResult {
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
    total_calls: usize,
    throughput_rps: f64,
    p95_latency_ms: u64,
    error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
struct SqliteConfigReport {
    path: String,
    journal_mode: String,
    sync_mode: String,
    busy_timeout_ms: u64,
}

struct BenchContext {
    target_meta: PerfTargetMeta,
    target: PerfTarget,
    store: SqliteRunStateStore,
    sqlite_config: SqliteConfigReport,
    reporter: TestReporter,
}

#[test]
#[ignore = "run manually with release profile to validate local sqlite store throughput diagnostics"]
fn perf_sqlite_store_run_state_contention_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_context(RUN_STATE_TARGET_KEY, "sqlite-store-run-state.sqlite")?;
    context.reporter.artifacts().write_json("sqlite_config.json", &context.sqlite_config)?;
    context.reporter.artifacts().write_json("perf_target.json", &context.target)?;

    context.store.reset_perf_stats();
    let (samples, elapsed) = run_run_state_workload(
        &context.store,
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
    )?;
    let contention = context.store.perf_stats_snapshot();

    let sweep = run_run_state_sweep(
        &context.store,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        &context.target.sweep_workers,
    )?;

    finalize_report(&mut context, RUN_STATE_TARGET_KEY, samples, elapsed, contention, sweep)
}

#[test]
#[ignore = "run manually with release profile to validate local sqlite store throughput diagnostics"]
fn perf_sqlite_store_registry_contention_release() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = init_context(REGISTRY_TARGET_KEY, "sqlite-store-registry.sqlite")?;
    context.reporter.artifacts().write_json("sqlite_config.json", &context.sqlite_config)?;
    context.reporter.artifacts().write_json("perf_target.json", &context.target)?;

    context.store.reset_perf_stats();
    let (samples, elapsed) = run_registry_workload(
        &context.store,
        context.target.workers,
        context.target.warmup_iterations,
        context.target.measure_iterations,
    )?;
    let contention = context.store.perf_stats_snapshot();

    let sweep = run_registry_sweep(
        &context.store,
        context.target.warmup_iterations,
        context.target.measure_iterations,
        &context.target.sweep_workers,
    )?;

    finalize_report(&mut context, REGISTRY_TARGET_KEY, samples, elapsed, contention, sweep)
}

fn init_context(
    test_key: &str,
    sqlite_file: &str,
) -> Result<BenchContext, Box<dyn std::error::Error>> {
    let reporter = TestReporter::new(test_key)?;
    let targets = load_targets()?;
    let target = targets
        .tests
        .get(test_key)
        .cloned()
        .ok_or_else(|| format!("missing sqlite perf target `{test_key}`"))?;

    let path = reporter.artifacts().root().join(sqlite_file);
    let journal_mode = sqlite_store_mode_from_target(&target.journal_mode)?;
    let sync_mode = sqlite_sync_mode_from_target(&target.sync_mode)?;
    let store = SqliteRunStateStore::new(SqliteStoreConfig {
        path: path.clone(),
        busy_timeout_ms: target.busy_timeout_ms,
        journal_mode,
        sync_mode,
        max_versions: None,
        schema_registry_max_schema_bytes: None,
        schema_registry_max_entries: None,
    })?;

    Ok(BenchContext {
        target_meta: targets.meta,
        target: target.clone(),
        store,
        sqlite_config: SqliteConfigReport {
            path: path.display().to_string(),
            journal_mode: target.journal_mode,
            sync_mode: target.sync_mode,
            busy_timeout_ms: target.busy_timeout_ms,
        },
        reporter,
    })
}

fn run_run_state_workload(
    store: &SqliteRunStateStore,
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
) -> Result<(Vec<CallSample>, Duration), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::from_raw(1).ok_or("tenant id failed")?;
    let namespace_id = NamespaceId::from_raw(1).ok_or("namespace id failed")?;
    let warmup_distribution = distribute_iterations(warmup_iterations, workers)?;
    let measure_distribution = distribute_iterations(measure_iterations, workers)?;
    let started = Instant::now();
    let mut joins = Vec::new();

    for worker_idx in 0..workers {
        let store = store.clone();
        let warmup = warmup_distribution[worker_idx];
        let measured = measure_distribution[worker_idx];
        joins.push(std::thread::spawn(move || {
            let total = warmup.saturating_add(measured);
            let mut samples = Vec::new();
            for local_idx in 0..total {
                let measured_phase = local_idx >= warmup;
                let run_id = RunId::new(format!("sqlite-run-state-{worker_idx:02}-{local_idx:05}"));
                let state = sample_state(tenant_id, namespace_id, &run_id);

                let save_started = Instant::now();
                let save_result = store.save(&state);
                if measured_phase {
                    samples.push(CallSample {
                        tool: "run_state_save".to_string(),
                        duration_ms: duration_to_ms_u64(save_started.elapsed()).unwrap_or(u64::MAX),
                        success: save_result.is_ok(),
                    });
                }

                let load_started = Instant::now();
                let load_result = store.load(&tenant_id, &namespace_id, &run_id);
                let load_success = matches!(load_result, Ok(Some(_)));
                if measured_phase {
                    samples.push(CallSample {
                        tool: "run_state_load".to_string(),
                        duration_ms: duration_to_ms_u64(load_started.elapsed()).unwrap_or(u64::MAX),
                        success: load_success,
                    });
                }
            }
            samples
        }));
    }

    let mut merged = Vec::new();
    for join in joins {
        let worker_samples = join.join().map_err(|_| "run-state worker thread panicked")?;
        merged.extend(worker_samples);
    }
    Ok((merged, started.elapsed()))
}

fn run_registry_workload(
    store: &SqliteRunStateStore,
    workers: usize,
    warmup_iterations: usize,
    measure_iterations: usize,
) -> Result<(Vec<CallSample>, Duration), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::from_raw(1).ok_or("tenant id failed")?;
    let namespace_id = NamespaceId::from_raw(1).ok_or("namespace id failed")?;
    let warmup_distribution = distribute_iterations(warmup_iterations, workers)?;
    let measure_distribution = distribute_iterations(measure_iterations, workers)?;
    let started = Instant::now();
    let mut joins = Vec::new();

    for worker_idx in 0..workers {
        let store = store.clone();
        let warmup = warmup_distribution[worker_idx];
        let measured = measure_distribution[worker_idx];
        joins.push(std::thread::spawn(move || {
            let total = warmup.saturating_add(measured);
            let mut samples = Vec::new();
            for local_idx in 0..total {
                let measured_phase = local_idx >= warmup;
                let record = sample_schema_record(
                    tenant_id,
                    namespace_id,
                    &format!("sqlite-schema-{worker_idx:02}-{local_idx:05}"),
                );

                let register_started = Instant::now();
                let register_result = store.register(record);
                if measured_phase {
                    samples.push(CallSample {
                        tool: "schemas_register".to_string(),
                        duration_ms: duration_to_ms_u64(register_started.elapsed())
                            .unwrap_or(u64::MAX),
                        success: register_result.is_ok(),
                    });
                }

                let list_started = Instant::now();
                let list_result = store.list(&tenant_id, &namespace_id, None, 25);
                let list_success = matches!(list_result, Ok(ref page) if !page.items.is_empty());
                if measured_phase {
                    samples.push(CallSample {
                        tool: "schemas_list".to_string(),
                        duration_ms: duration_to_ms_u64(list_started.elapsed()).unwrap_or(u64::MAX),
                        success: list_success,
                    });
                }
            }
            samples
        }));
    }

    let mut merged = Vec::new();
    for join in joins {
        let worker_samples = join.join().map_err(|_| "registry worker thread panicked")?;
        merged.extend(worker_samples);
    }
    Ok((merged, started.elapsed()))
}

fn run_run_state_sweep(
    store: &SqliteRunStateStore,
    warmup_iterations: usize,
    measure_iterations: usize,
    workers: &[usize],
) -> Result<Vec<SweepResult>, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    for worker_count in workers {
        store.reset_perf_stats();
        let (samples, elapsed) =
            run_run_state_workload(store, *worker_count, warmup_iterations, measure_iterations)?;
        let metrics = summarize_samples(&samples, elapsed)?;
        output.push(SweepResult {
            workers: *worker_count,
            warmup_iterations,
            measure_iterations,
            total_calls: metrics.total_calls,
            throughput_rps: metrics.throughput_rps,
            p95_latency_ms: metrics.p95_latency_ms,
            error_rate: metrics.error_rate,
        });
    }
    Ok(output)
}

fn run_registry_sweep(
    store: &SqliteRunStateStore,
    warmup_iterations: usize,
    measure_iterations: usize,
    workers: &[usize],
) -> Result<Vec<SweepResult>, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    for worker_count in workers {
        store.reset_perf_stats();
        let (samples, elapsed) =
            run_registry_workload(store, *worker_count, warmup_iterations, measure_iterations)?;
        let metrics = summarize_samples(&samples, elapsed)?;
        output.push(SweepResult {
            workers: *worker_count,
            warmup_iterations,
            measure_iterations,
            total_calls: metrics.total_calls,
            throughput_rps: metrics.throughput_rps,
            p95_latency_ms: metrics.p95_latency_ms,
            error_rate: metrics.error_rate,
        });
    }
    Ok(output)
}

fn finalize_report(
    context: &mut BenchContext,
    test_name: &str,
    samples: Vec<CallSample>,
    elapsed: Duration,
    contention: SqlitePerfStatsSnapshot,
    sweep: Vec<SweepResult>,
) -> Result<(), Box<dyn std::error::Error>> {
    let metrics = summarize_samples(&samples, elapsed)?;
    let tools = build_tool_metrics(&samples, elapsed)?;
    let slo_enforced = target_meta_enforces_slo(&context.target_meta) && should_enforce_slo();
    let violations =
        if slo_enforced { evaluate_slo(&metrics, &context.target) } else { Vec::new() };

    let summary = PerfSummary {
        test_name: test_name.to_string(),
        profile: profile_name(),
        target_meta: context.target_meta.clone(),
        workload: context.target.clone(),
        metrics,
        tools,
        telemetry_latency_buckets_ms: Vec::new(),
        slo_enforced,
        slo_violations: violations.clone(),
    };

    context.reporter.artifacts().write_json("perf_summary.json", &summary)?;
    context.reporter.artifacts().write_json("sqlite_contention.json", &contention)?;
    context.reporter.artifacts().write_json("sqlite_sweep.json", &sweep)?;

    let mut notes = vec![format!(
        "measured {} calls at {} rps",
        summary.metrics.total_calls, summary.metrics.throughput_rps
    )];
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
    context.reporter.finish(
        status,
        notes,
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "perf_summary.json".to_string(),
            "perf_target.json".to_string(),
            "sqlite_config.json".to_string(),
            "sqlite_contention.json".to_string(),
            "sqlite_sweep.json".to_string(),
        ],
    )?;

    if !violations.is_empty() {
        return Err(format!("SLO violations: {}", violations.join("; ")).into());
    }
    Ok(())
}

fn load_targets() -> Result<PerfTargetsFile, Box<dyn std::error::Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PERF_TARGETS_SQLITE_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    let targets: PerfTargetsFile =
        toml::from_str(&text).map_err(|err| format!("failed parsing {}: {err}", path.display()))?;
    Ok(targets)
}

fn sample_spec(namespace_id: NamespaceId) -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("sqlite-perf-scenario"),
        namespace_id,
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn sample_state(tenant_id: TenantId, namespace_id: NamespaceId, run_id: &RunId) -> RunState {
    let spec = sample_spec(namespace_id);
    let spec_hash = spec
        .canonical_hash_with(DEFAULT_HASH_ALGORITHM)
        .expect("scenario spec hash should build deterministically");
    RunState {
        tenant_id,
        namespace_id,
        run_id: run_id.clone(),
        scenario_id: ScenarioId::new("sqlite-perf-scenario"),
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

fn sample_schema_record(
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    schema_id: &str,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new("v1"),
        schema: json!({
            "type": "object",
            "properties": {
                "value": { "type": "string" },
                "index": { "type": "integer" }
            },
            "required": ["value", "index"]
        }),
        description: Some("sqlite perf registry schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}

fn sqlite_store_mode_from_target(mode: &str) -> Result<SqliteStoreMode, String> {
    match mode.to_ascii_lowercase().as_str() {
        "wal" => Ok(SqliteStoreMode::Wal),
        "delete" => Ok(SqliteStoreMode::Delete),
        other => Err(format!("unsupported sqlite journal_mode `{other}`")),
    }
}

fn sqlite_sync_mode_from_target(mode: &str) -> Result<SqliteSyncMode, String> {
    match mode.to_ascii_lowercase().as_str() {
        "full" => Ok(SqliteSyncMode::Full),
        "normal" => Ok(SqliteSyncMode::Normal),
        other => Err(format!("unsupported sqlite sync_mode `{other}`")),
    }
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
            },
        );
    }
    Ok(output)
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

fn target_meta_enforces_slo(meta: &PerfTargetMeta) -> bool {
    !meta.enforcement_mode.eq_ignore_ascii_case("report_only")
}

fn should_enforce_slo() -> bool {
    !matches!(
        std::env::var(PERF_SKIP_SLO_ASSERTS_ENV).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn profile_name() -> String {
    if cfg!(debug_assertions) { "debug".to_string() } else { "release".to_string() }
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
