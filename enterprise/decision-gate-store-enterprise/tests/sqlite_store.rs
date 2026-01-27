// enterprise/decision-gate-store-enterprise/tests/sqlite_store.rs
// ============================================================================
// Module: Enterprise SQLite Store Tests
// Description: Unit tests for the enterprise SQLite store wrapper.
// Purpose: Validate construction, delegation, and Deref semantics.
// ============================================================================

//! Enterprise SQLite store unit tests.

use decision_gate_store_enterprise::sqlite_store::EnterpriseSqliteStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use tempfile::TempDir;

#[test]
fn enterprise_sqlite_store_creates_successfully() {
    let dir = TempDir::new().expect("temp dir");
    let config = SqliteStoreConfig {
        path: dir.path().join("test.db"),
        busy_timeout_ms: 5_000,
        journal_mode: Default::default(),
        sync_mode: Default::default(),
        max_versions: None,
    };
    let store = EnterpriseSqliteStore::new(config);
    assert!(store.is_ok());
}

#[test]
fn enterprise_sqlite_store_inner_returns_reference() {
    let dir = TempDir::new().expect("temp dir");
    let config = SqliteStoreConfig {
        path: dir.path().join("test_inner.db"),
        busy_timeout_ms: 5_000,
        journal_mode: Default::default(),
        sync_mode: Default::default(),
        max_versions: None,
    };
    let store = EnterpriseSqliteStore::new(config).expect("create store");
    // Verify inner() returns a reference without panicking.
    let _inner = store.inner();
}

#[test]
fn enterprise_sqlite_store_deref_delegates() {
    use std::ops::Deref;
    let dir = TempDir::new().expect("temp dir");
    let config = SqliteStoreConfig {
        path: dir.path().join("test_deref.db"),
        busy_timeout_ms: 5_000,
        journal_mode: Default::default(),
        sync_mode: Default::default(),
        max_versions: None,
    };
    let store = EnterpriseSqliteStore::new(config).expect("create store");
    // Verify Deref resolves without panicking.
    let _deref = store.deref();
}
