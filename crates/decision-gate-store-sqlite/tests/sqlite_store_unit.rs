// crates/decision-gate-store-sqlite/tests/sqlite_store_unit.rs
// ============================================================================
// Module: SQLite Store Integrity Unit Tests
// Description: Targeted integrity tests for SQLite run state store
// Purpose: Validate path safety, schema versioning, size limits, retention,
//          and corruption detection.
// Threat Models: TM-STORE-001 (tampering), TM-STORE-002 (corruption),
//               TM-STORE-003 (concurrency)
// ============================================================================

//! ## Overview
//! Unit-level tests for `SQLite` store integrity invariants:
//! - Path safety checks (length/component/directory rejection)
//! - Schema version validation and upgrade path
//! - Hash algorithm validation and payload integrity
//! - Size limits for state payloads (save/load)
//! - Retention pruning and list APIs
//! - Concurrency safety (multi-threaded save/load)

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
    reason = "Test-only assertions and helpers are permitted."
)]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use decision_gate_core::AdvanceTo;
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
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_store_sqlite::MAX_STATE_BYTES;
use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use decision_gate_store_sqlite::SqliteStoreError;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use rusqlite::Connection;
use rusqlite::params;
use tempfile::TempDir;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn sample_spec() -> ScenarioSpec {
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
            on_timeout: TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

fn sample_state(run_id: &str) -> RunState {
    let spec = sample_spec();
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");
    RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
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
    }
}

const fn config_for_path(path: PathBuf, max_versions: Option<u64>) -> SqliteStoreConfig {
    SqliteStoreConfig {
        path,
        busy_timeout_ms: 1_000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions,
        schema_registry_max_schema_bytes: None,
        schema_registry_max_entries: None,
        writer_queue_capacity: 1_024,
        batch_max_ops: 64,
        batch_max_bytes: 512 * 1024,
        batch_max_wait_ms: 2,
        read_pool_size: 4,
    }
}

fn store_for(path: &Path, max_versions: Option<u64>) -> SqliteRunStateStore {
    SqliteRunStateStore::new(config_for_path(path.to_path_buf(), max_versions)).expect("store init")
}

fn store_bytes_for_message_len(message_len: usize) -> usize {
    let mut state = sample_state("run-1");
    let hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let error = decision_gate_core::ToolCallError {
        code: "err".to_string(),
        message: "a".repeat(message_len),
        details: None,
    };
    state.tool_calls.push(decision_gate_core::ToolCallRecord {
        call_id: "call-1".to_string(),
        method: "scenario.next".to_string(),
        request_hash: hash.clone(),
        response_hash: hash,
        called_at: Timestamp::Logical(1),
        correlation_id: None,
        error: Some(error),
    });
    canonical_json_bytes(&state).expect("canonical json").len()
}

fn run_state_with_message_len(message_len: usize) -> RunState {
    let mut state = sample_state("run-1");
    let hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload");
    let error = decision_gate_core::ToolCallError {
        code: "err".to_string(),
        message: "a".repeat(message_len),
        details: None,
    };
    state.tool_calls.push(decision_gate_core::ToolCallRecord {
        call_id: "call-1".to_string(),
        method: "scenario.next".to_string(),
        request_hash: hash.clone(),
        response_hash: hash,
        called_at: Timestamp::Logical(1),
        correlation_id: None,
        error: Some(error),
    });
    state
}

fn message_len_bounds(limit: usize) -> (usize, usize) {
    let mut low = 0_usize;
    let mut high = limit.saturating_add(2048);
    while low + 1 < high {
        let mid = usize::midpoint(low, high);
        let size = store_bytes_for_message_len(mid);
        if size <= limit {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low, high)
}

// ============================================================================
// SECTION: Path Validation
// ============================================================================

#[test]
fn sqlite_store_rejects_directory_path() {
    let temp = TempDir::new().unwrap();
    let config = config_for_path(temp.path().to_path_buf(), None);
    let Err(err) = SqliteRunStateStore::new(config) else {
        panic!("expected invalid directory path to fail");
    };
    assert!(matches!(err, SqliteStoreError::Invalid(_)));
}

#[test]
fn sqlite_store_rejects_empty_path() {
    let config = config_for_path(PathBuf::new(), None);
    let Err(err) = SqliteRunStateStore::new(config) else {
        panic!("expected empty path to fail");
    };
    assert!(matches!(err, SqliteStoreError::Invalid(_)));
}

#[test]
fn sqlite_store_rejects_overlong_component() {
    let temp = TempDir::new().unwrap();
    let long_name = "a".repeat(300);
    let path = temp.path().join(long_name);
    let config = config_for_path(path, None);
    let Err(err) = SqliteRunStateStore::new(config) else {
        panic!("expected overlong component to fail");
    };
    assert!(matches!(err, SqliteStoreError::Invalid(_)));
}

#[test]
fn sqlite_store_rejects_overlong_total_path() {
    let temp = TempDir::new().unwrap();
    let long_name = "a".repeat(5000);
    let path = temp.path().join(long_name);
    let config = config_for_path(path, None);
    let Err(err) = SqliteRunStateStore::new(config) else {
        panic!("expected overlong path to fail");
    };
    assert!(matches!(err, SqliteStoreError::Invalid(_)));
}

// ============================================================================
// SECTION: Schema Versioning
// ============================================================================

#[test]
fn sqlite_store_rejects_unknown_schema_version() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE store_meta (version INTEGER NOT NULL);").unwrap();
    conn.execute("INSERT INTO store_meta (version) VALUES (?1)", params![999_i64]).unwrap();

    let config = config_for_path(path, None);
    let Err(err) = SqliteRunStateStore::new(config) else {
        panic!("expected schema mismatch to fail");
    };
    assert!(matches!(err, SqliteStoreError::VersionMismatch(_)));
}

#[test]
fn sqlite_store_upgrades_schema_from_v3() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE store_meta (version INTEGER NOT NULL);
         INSERT INTO store_meta (version) VALUES (3);
         CREATE TABLE runs (
             tenant_id TEXT NOT NULL,
             namespace_id TEXT NOT NULL,
             run_id TEXT NOT NULL,
             latest_version INTEGER NOT NULL,
             PRIMARY KEY (tenant_id, namespace_id, run_id)
         );
         CREATE TABLE run_state_versions (
             tenant_id TEXT NOT NULL,
             namespace_id TEXT NOT NULL,
             run_id TEXT NOT NULL,
             version INTEGER NOT NULL,
             state_json BLOB NOT NULL,
             state_hash TEXT NOT NULL,
             hash_algorithm TEXT NOT NULL,
             saved_at INTEGER NOT NULL,
             PRIMARY KEY (tenant_id, namespace_id, run_id, version),
             FOREIGN KEY (tenant_id, namespace_id, run_id)
                 REFERENCES runs(tenant_id, namespace_id, run_id) ON DELETE CASCADE
         );
         CREATE TABLE data_shapes (
             tenant_id TEXT NOT NULL,
             namespace_id TEXT NOT NULL,
             schema_id TEXT NOT NULL,
             version TEXT NOT NULL,
             schema_json BLOB NOT NULL,
             schema_hash TEXT NOT NULL,
             hash_algorithm TEXT NOT NULL,
             description TEXT,
             created_at_json TEXT NOT NULL,
             PRIMARY KEY (tenant_id, namespace_id, schema_id, version)
         );",
    )
    .unwrap();

    let config = config_for_path(path.clone(), None);
    SqliteRunStateStore::new(config).expect("upgrade should succeed");

    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("SELECT version FROM store_meta LIMIT 1", params![], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 5, "schema version should be upgraded to 5");

    let mut stmt = conn.prepare("PRAGMA table_info(data_shapes)").unwrap();
    let columns: Vec<String> =
        stmt.query_map([], |row| row.get::<_, String>(1)).unwrap().filter_map(Result::ok).collect();
    assert!(columns.contains(&"signing_key_id".to_string()));
    assert!(columns.contains(&"signing_signature".to_string()));
    assert!(columns.contains(&"signing_algorithm".to_string()));
    assert!(columns.contains(&"schema_size_bytes".to_string()));

    let counters_table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = \
             'registry_namespace_counters'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(counters_table_count, 1, "registry counters table should exist");
}

// ============================================================================
// SECTION: Hash Integrity and Corruption
// ============================================================================

#[test]
fn sqlite_store_load_missing_run_returns_none() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let result = store.load(
        &TenantId::from_raw(1).expect("tenant"),
        &NamespaceId::from_raw(1).expect("namespace"),
        &RunId::new("missing"),
    );
    assert!(result.unwrap().is_none());
}

#[test]
fn sqlite_store_rejects_unknown_hash_algorithm() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET hash_algorithm = 'md5' WHERE run_id = ?1",
        params![state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported hash algorithm"));
}

#[test]
fn sqlite_store_detects_hash_mismatch() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET state_hash = 'bad' WHERE run_id = ?1",
        params![state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("hash mismatch"));
}

#[test]
fn sqlite_store_detects_invalid_latest_version() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE runs SET latest_version = 0 WHERE run_id = ?1",
        params![state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("invalid latest_version"));
}

#[test]
fn sqlite_store_rejects_run_id_mismatch() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let mut corrupted = state.clone();
    corrupted.run_id = RunId::new("run-2");
    let bytes = canonical_json_bytes(&corrupted).unwrap();
    let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &bytes);

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET state_json = ?1, state_hash = ?2 WHERE run_id = ?3",
        params![bytes, digest.value, state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("run_id mismatch"));
}

#[test]
fn sqlite_store_rejects_tenant_namespace_mismatch() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let mut corrupted = state.clone();
    corrupted.tenant_id = TenantId::from_raw(2).expect("tenant");
    let bytes = canonical_json_bytes(&corrupted).unwrap();
    let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &bytes);

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET state_json = ?1, state_hash = ?2 WHERE run_id = ?3",
        params![bytes, digest.value, state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("tenant/namespace mismatch"));
}

#[test]
fn sqlite_store_rejects_oversized_payload_on_load() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let oversize = vec![b'x'; MAX_STATE_BYTES + 1];
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET state_json = ?1 WHERE run_id = ?2",
        params![oversize, state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load(&state.tenant_id, &state.namespace_id, &state.run_id);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("size limit"));
}

#[test]
fn sqlite_store_rejects_oversized_payload_on_load_version() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let oversize = vec![b'x'; MAX_STATE_BYTES + 1];
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET state_json = ?1 WHERE run_id = ?2 AND version = 1",
        params![oversize, state.run_id.as_str()],
    )
    .unwrap();

    let result = store.load_version(state.tenant_id, state.namespace_id, &state.run_id, 1);
    let err = result.unwrap_err();
    assert!(matches!(err, SqliteStoreError::TooLarge { .. }));
}

#[test]
fn sqlite_store_list_run_versions_descending() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);

    let mut state = sample_state("run-1");
    store.save(&state).unwrap();
    state.current_stage_id = StageId::new("stage-2");
    store.save(&state).unwrap();
    state.current_stage_id = StageId::new("stage-3");
    store.save(&state).unwrap();

    let versions =
        store.list_run_versions(state.tenant_id, state.namespace_id, &state.run_id).unwrap();
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[0].version, 3);
    assert_eq!(versions[1].version, 2);
    assert_eq!(versions[2].version, 1);
}

#[test]
fn sqlite_store_list_run_versions_rejects_oversized_payloads() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let oversize = vec![b'x'; MAX_STATE_BYTES + 1];
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET state_json = ?1 WHERE run_id = ?2 AND version = 1",
        params![oversize, state.run_id.as_str()],
    )
    .unwrap();

    let result = store.list_run_versions(state.tenant_id, state.namespace_id, &state.run_id);
    let err = result.unwrap_err();
    assert!(matches!(err, SqliteStoreError::TooLarge { .. }));
}

// ============================================================================
// SECTION: Size Limits
// ============================================================================

#[test]
fn sqlite_store_rejects_oversized_state_on_save() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);

    let (_max_under, min_over) = message_len_bounds(MAX_STATE_BYTES);
    let oversized = run_state_with_message_len(min_over);
    let result = store.save(&oversized);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("size limit"));
}

#[test]
fn sqlite_store_accepts_state_just_under_limit() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);

    let (max_under, _min_over) = message_len_bounds(MAX_STATE_BYTES);
    let state = run_state_with_message_len(max_under);
    let size = canonical_json_bytes(&state).unwrap().len();
    assert!(size <= MAX_STATE_BYTES, "expected size under limit");

    let result = store.save(&state);
    assert!(result.is_ok());
}

// ============================================================================
// SECTION: Retention and Listing
// ============================================================================

#[test]
fn sqlite_store_enforces_max_versions() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, Some(2));

    let mut state = sample_state("run-1");
    store.save(&state).unwrap();
    state.current_stage_id = StageId::new("stage-2");
    store.save(&state).unwrap();
    state.current_stage_id = StageId::new("stage-3");
    store.save(&state).unwrap();

    let versions =
        store.list_run_versions(state.tenant_id, state.namespace_id, &state.run_id).unwrap();
    assert_eq!(versions.len(), 2, "expected retention to prune versions");
    assert_eq!(versions[0].version, 3);
    assert_eq!(versions[1].version, 2);
}

#[test]
fn sqlite_store_rejects_zero_max_versions() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, Some(0));

    let state = sample_state("run-1");
    let result = store.save(&state);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("max_versions"));
}

#[test]
fn sqlite_store_list_runs_filters() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);

    let state_a = sample_state("run-a");
    store.save(&state_a).unwrap();

    let mut state_b = sample_state("run-b");
    state_b.tenant_id = TenantId::from_raw(2).expect("tenant");
    store.save(&state_b).unwrap();

    let all = store.list_runs(None, None).unwrap();
    assert_eq!(all.len(), 2);

    let filtered = store.list_runs(Some(TenantId::from_raw(1).expect("tenant")), None).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].run_id.as_str(), "run-a");
}

#[test]
fn sqlite_store_list_runs_sorted_by_saved_at_desc() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);

    let state_a = sample_state("run-a");
    store.save(&state_a).unwrap();
    let state_b = sample_state("run-b");
    store.save(&state_b).unwrap();

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE run_state_versions SET saved_at = 10 WHERE run_id = ?1",
        params![state_a.run_id.as_str()],
    )
    .unwrap();
    conn.execute(
        "UPDATE run_state_versions SET saved_at = 20 WHERE run_id = ?1",
        params![state_b.run_id.as_str()],
    )
    .unwrap();

    let runs = store.list_runs(None, None).unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].run_id.as_str(), "run-b");
    assert_eq!(runs[1].run_id.as_str(), "run-a");
}

// ============================================================================
// SECTION: Journal Mode and Concurrency
// ============================================================================

#[test]
fn sqlite_store_sets_wal_mode() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let _store = store_for(&path, None);

    let conn = Connection::open(&path).unwrap();
    let mode: String = conn.query_row("PRAGMA journal_mode", params![], |row| row.get(0)).unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
}

#[test]
fn sqlite_store_sets_delete_mode() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let config = SqliteStoreConfig {
        path: path.clone(),
        busy_timeout_ms: 1_000,
        journal_mode: SqliteStoreMode::Delete,
        sync_mode: SqliteSyncMode::Full,
        max_versions: None,
        schema_registry_max_schema_bytes: None,
        schema_registry_max_entries: None,
        writer_queue_capacity: 1_024,
        batch_max_ops: 64,
        batch_max_bytes: 512 * 1024,
        batch_max_wait_ms: 2,
        read_pool_size: 4,
    };
    let _store = SqliteRunStateStore::new(config).unwrap();

    let conn = Connection::open(&path).unwrap();
    let mode: String = conn.query_row("PRAGMA journal_mode", params![], |row| row.get(0)).unwrap();
    assert_eq!(mode.to_lowercase(), "delete");
}

#[test]
fn sqlite_store_supports_concurrent_reads() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path, None);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let store = Arc::new(store);
    let mut handles = Vec::new();
    for _ in 0 .. 4 {
        let store = Arc::clone(&store);
        let tenant = state.tenant_id;
        let namespace = state.namespace_id;
        let run_id = state.run_id.clone();
        handles.push(thread::spawn(move || {
            let loaded = store.load(&tenant, &namespace, &run_id).unwrap();
            assert!(loaded.is_some());
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn sqlite_store_supports_concurrent_writes() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = Arc::new(store_for(&path, None));

    let mut handles = Vec::new();
    for i in 0 .. 4 {
        let store = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            let state = sample_state(&format!("run-{i}"));
            store.save(&state).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let runs = store.list_runs(None, None).unwrap();
    assert_eq!(runs.len(), 4);
}

// ============================================================================
// SECTION: Overload Error Mapping
// ============================================================================

#[test]
fn sqlite_store_error_overloaded_maps_to_store_error_overloaded() {
    let mapped: StoreError = SqliteStoreError::Overloaded {
        message: "sqlite writer queue full".to_string(),
        retry_after_ms: Some(42),
    }
    .into();
    assert!(matches!(
        mapped,
        StoreError::Overloaded {
            message,
            retry_after_ms: Some(42)
        } if message == "sqlite writer queue full"
    ));
}
