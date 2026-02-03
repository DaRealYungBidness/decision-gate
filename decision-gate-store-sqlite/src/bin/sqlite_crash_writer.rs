//! `SQLite` crash writer for store durability tests.
// decision-gate-store-sqlite/src/bin/sqlite_crash_writer.rs
// ============================================================================
// Binary: SQLite Crash Writer
// Description: Simulates a crash during an uncommitted run-state write.
// Purpose: Support durability tests for rollback/crash recovery behavior.
// Dependencies: decision-gate-core, decision-gate-store-sqlite, rusqlite
// ============================================================================

use std::env;
use std::path::PathBuf;

use decision_gate_core::AdvanceTo;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
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
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use rusqlite::params;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing sqlite path")
    })?;
    let run_id = args.next().unwrap_or_else(|| "run-1".to_string());
    let path = PathBuf::from(path);

    let config = SqliteStoreConfig {
        path: path.clone(),
        busy_timeout_ms: 1_000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions: None,
    };
    let _store = SqliteRunStateStore::new(config)?;
    let state = sample_state(&run_id)?;
    let canonical_json = canonical_json_bytes(&state)?;
    let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &canonical_json);

    let mut conn = rusqlite::Connection::open(&path)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = wal; PRAGMA synchronous = full;",
    )?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO runs (tenant_id, namespace_id, run_id, latest_version) VALUES (?1, ?2, ?3, 1)",
        params!["1", "1", run_id.as_str()],
    )?;
    tx.execute(
        "INSERT INTO run_state_versions (tenant_id, namespace_id, run_id, version, state_json, \
         state_hash, hash_algorithm, saved_at) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)",
        params!["1", "1", run_id.as_str(), canonical_json, digest.value, "sha256", 0_i64],
    )?;

    std::process::abort();
}

/// Builds a minimal run state used by the crash writer.
///
/// # Errors
///
/// Returns an error if identifiers or hashing fail.
fn sample_state(run_id: &str) -> Result<RunState, Box<dyn std::error::Error>> {
    let namespace_id = NamespaceId::from_raw(1).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "nonzero namespaceid")
    })?;
    let spec = ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
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
    };
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM)?;
    let tenant_id = TenantId::from_raw(1)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "nonzero tenantid"))?;
    Ok(RunState {
        tenant_id,
        namespace_id,
        run_id: RunId::new(run_id),
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
    })
}
