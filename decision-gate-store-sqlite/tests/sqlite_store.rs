// decision-gate-store-sqlite/tests/sqlite_store.rs
// ============================================================================
// Module: SQLite Store Tests
// Description: Validate SQLite RunStateStore behavior.
// Purpose: Ensure durable persistence and integrity checks.
// Dependencies: decision-gate-store-sqlite, decision-gate-core, rusqlite, serde_json, tempfile
// ============================================================================

//! ## Overview
//! Conformance tests for the SQLite-backed run state store.
//! Exercises durability, integrity checks, and retention behavior with
//! adversarial storage conditions. Security posture: tests model untrusted
//! storage inputs per `Docs/security/threat_model.md`.

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

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::AdvanceTo;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketPayload;
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
use decision_gate_core::SubmissionRecord;
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
use tempfile::TempDir;

// ============================================================================
// SECTION: Helpers
// ============================================================================

fn sample_state(run_id: &str) -> RunState {
    let spec = ScenarioSpec {
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
        predicates: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };
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

fn store_for(path: &std::path::Path) -> SqliteRunStateStore {
    let config = SqliteStoreConfig {
        path: path.to_path_buf(),
        busy_timeout_ms: 1_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };
    SqliteRunStateStore::new(config).expect("store init")
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn sqlite_store_roundtrip() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let state = sample_state("run-1");
    store.save(&state).unwrap();
    let loaded = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &RunId::new("run-1"),
        )
        .unwrap();
    assert_eq!(loaded, Some(state));
}

#[test]
fn sqlite_store_returns_none_for_missing_run() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let loaded = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &RunId::new("missing"),
        )
        .unwrap();
    assert!(loaded.is_none());
}

#[test]
fn sqlite_store_persists_across_instances() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let state = sample_state("run-1");
    {
        let store = store_for(&path);
        store.save(&state).unwrap();
    }
    let store = store_for(&path);
    let loaded = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &RunId::new("run-1"),
        )
        .unwrap();
    assert_eq!(loaded, Some(state));
}

#[test]
fn sqlite_store_detects_corrupt_hash() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let state = sample_state("run-1");
    store.save(&state).unwrap();
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE run_state_versions SET state_hash = 'bad' WHERE run_id = ?1",
                rusqlite::params![state.run_id.as_str()],
            )
            .unwrap();
    }
    let result = store.load(
        &TenantId::from_raw(1).expect("nonzero tenantid"),
        &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        &RunId::new("run-1"),
    );
    assert!(result.is_err());
}

#[test]
fn sqlite_store_rejects_oversized_state_payload() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let run_id = RunId::new("run-oversize");
    let oversized = vec![0_u8; MAX_STATE_BYTES + 1];
    let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &oversized);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO runs (tenant_id, namespace_id, run_id, latest_version) VALUES (?1, ?2, \
             ?3, 1)",
            rusqlite::params!["1", "1", run_id.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO run_state_versions (tenant_id, namespace_id, run_id, version, \
             state_json, state_hash, hash_algorithm, saved_at) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, \
             ?7)",
            rusqlite::params!["1", "1", run_id.as_str(), oversized, digest.value, "sha256", 0_i64],
        )
        .unwrap();

    let result = store.load(
        &TenantId::from_raw(1).expect("nonzero tenantid"),
        &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        &run_id,
    );
    assert!(matches!(result, Err(StoreError::Invalid(_))));
}

#[test]
fn sqlite_store_rejects_oversized_state_on_save() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let mut state = sample_state("run-oversize-save");

    let payload_string = "x".repeat(MAX_STATE_BYTES + 64);
    let payload_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, payload_string.as_bytes());
    state.submissions.push(SubmissionRecord {
        submission_id: "submission-1".to_string(),
        run_id: state.run_id.clone(),
        payload: PacketPayload::Json {
            value: serde_json::Value::String(payload_string),
        },
        content_type: "application/json".to_string(),
        content_hash: payload_hash,
        submitted_at: Timestamp::Logical(0),
        correlation_id: None,
    });

    let result = store.save(&state);
    assert!(matches!(result, Err(StoreError::Invalid(_))));
}

#[test]
fn sqlite_store_enforces_max_versions() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let config = SqliteStoreConfig {
        path: path.clone(),
        busy_timeout_ms: 1_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: Some(2),
    };
    let store = SqliteRunStateStore::new(config).expect("store init");
    let mut state = sample_state("run-1");
    store.save(&state).unwrap();
    state.status = RunStatus::Completed;
    store.save(&state).unwrap();
    state.status = RunStatus::Failed;
    store.save(&state).unwrap();

    let connection = rusqlite::Connection::open(&path).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM run_state_versions WHERE run_id = ?1",
            rusqlite::params![state.run_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn sqlite_store_rejects_version_mismatch() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let _store = store_for(&path);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute("UPDATE store_meta SET version = 999", rusqlite::params![]).unwrap();

    let config = SqliteStoreConfig {
        path,
        busy_timeout_ms: 1_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };
    let result = SqliteRunStateStore::new(config);
    assert!(matches!(result, Err(SqliteStoreError::VersionMismatch(_))));
}

#[test]
fn sqlite_store_rejects_invalid_hash_algorithm() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE run_state_versions SET hash_algorithm = 'md5' WHERE run_id = ?1",
            rusqlite::params![state.run_id.as_str()],
        )
        .unwrap();

    let result = store.load(
        &TenantId::from_raw(1).expect("nonzero tenantid"),
        &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        &RunId::new("run-1"),
    );
    assert!(matches!(result, Err(StoreError::Invalid(_))));
}

#[test]
fn sqlite_store_rejects_run_id_mismatch() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let connection = rusqlite::Connection::open(&path).unwrap();
    let original: Vec<u8> = connection
        .query_row(
            "SELECT state_json FROM run_state_versions WHERE run_id = ?1",
            rusqlite::params![state.run_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    value["run_id"] = serde_json::Value::String(String::from("run-2"));
    let canonical = canonical_json_bytes(&value).unwrap();
    let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &canonical);
    connection
        .execute(
            "UPDATE run_state_versions SET state_json = ?1, state_hash = ?2 WHERE run_id = ?3",
            rusqlite::params![canonical, digest.value, state.run_id.as_str()],
        )
        .unwrap();

    let result = store.load(
        &TenantId::from_raw(1).expect("nonzero tenantid"),
        &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        &RunId::new("run-1"),
    );
    assert!(matches!(result, Err(StoreError::Invalid(_))));
}

#[test]
fn sqlite_store_rejects_invalid_latest_version_on_load() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE runs SET latest_version = -1 WHERE run_id = ?1",
            rusqlite::params![state.run_id.as_str()],
        )
        .unwrap();

    let result = store.load(
        &TenantId::from_raw(1).expect("nonzero tenantid"),
        &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        &RunId::new("run-1"),
    );
    assert!(matches!(result, Err(StoreError::Corrupt(_))));
}

#[test]
fn sqlite_store_rejects_latest_version_overflow_on_save() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = store_for(&path);
    let state = sample_state("run-1");
    store.save(&state).unwrap();

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE runs SET latest_version = ?1 WHERE run_id = ?2",
            rusqlite::params![i64::MAX, state.run_id.as_str()],
        )
        .unwrap();

    let result = store.save(&state);
    assert!(matches!(result, Err(StoreError::Corrupt(_))));
}

#[test]
fn sqlite_store_rejects_directory_path() {
    let temp = TempDir::new().unwrap();
    let config = SqliteStoreConfig {
        path: temp.path().to_path_buf(),
        busy_timeout_ms: 1_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };
    let result = SqliteRunStateStore::new(config);
    assert!(matches!(result, Err(SqliteStoreError::Invalid(_))));
}

#[test]
fn sqlite_store_rejects_overlong_path_component() {
    let temp = TempDir::new().unwrap();
    let component = "x".repeat(300);
    let config = SqliteStoreConfig {
        path: temp.path().join(component),
        busy_timeout_ms: 1_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };
    let result = SqliteRunStateStore::new(config);
    assert!(matches!(result, Err(SqliteStoreError::Invalid(_))));
}

#[test]
fn sqlite_store_rejects_overlong_total_path() {
    let temp = TempDir::new().unwrap();
    let component = "y".repeat(5_000);
    let config = SqliteStoreConfig {
        path: temp.path().join(component),
        busy_timeout_ms: 1_000,
        journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
        sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
        max_versions: None,
    };
    let result = SqliteRunStateStore::new(config);
    assert!(matches!(result, Err(SqliteStoreError::Invalid(_))));
}

#[test]
fn sqlite_store_allows_concurrent_saves() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("store.sqlite");
    let store = std::sync::Arc::new(store_for(&path));
    let mut handles = Vec::new();

    for index in 0 .. 10 {
        let store = std::sync::Arc::clone(&store);
        handles.push(std::thread::spawn(move || {
            let mut state = sample_state("run-1");
            state.status = match index % 3 {
                0 => RunStatus::Active,
                1 => RunStatus::Completed,
                _ => RunStatus::Failed,
            };
            store.save(&state).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let connection = rusqlite::Connection::open(&path).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM run_state_versions WHERE run_id = ?1",
            rusqlite::params!["run-1"],
            |row| row.get(0),
        )
        .unwrap();
    let latest: i64 = connection
        .query_row(
            "SELECT latest_version FROM runs WHERE run_id = ?1",
            rusqlite::params!["run-1"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 10);
    assert_eq!(latest, 10);
}
