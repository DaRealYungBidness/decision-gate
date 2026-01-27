// enterprise/decision-gate-store-enterprise/tests/sqlite_store.rs
// ============================================================================
// Module: Enterprise SQLite Store Tests
// Description: Unit tests for the enterprise SQLite store wrapper.
// Purpose: Validate construction, delegation, and Deref semantics.
// ============================================================================

//! Enterprise `SQLite` store unit tests.

use decision_gate_store_enterprise::sqlite_store::EnterpriseSqliteStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use tempfile::TempDir;

#[test]
fn enterprise_sqlite_store_creates_successfully() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let config = SqliteStoreConfig {
        path: dir.path().join("test.db"),
        busy_timeout_ms: 5_000,
        journal_mode: SqliteStoreMode::default(),
        sync_mode: SqliteSyncMode::default(),
        max_versions: None,
    };
    let _store = EnterpriseSqliteStore::new(config)?;
    Ok(())
}

#[test]
fn enterprise_sqlite_store_inner_returns_reference() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let config = SqliteStoreConfig {
        path: dir.path().join("test_inner.db"),
        busy_timeout_ms: 5_000,
        journal_mode: SqliteStoreMode::default(),
        sync_mode: SqliteSyncMode::default(),
        max_versions: None,
    };
    let store = EnterpriseSqliteStore::new(config)?;
    // Verify inner() returns a reference without panicking.
    let _inner = store.inner();
    Ok(())
}

#[test]
fn enterprise_sqlite_store_deref_delegates() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let config = SqliteStoreConfig {
        path: dir.path().join("test_deref.db"),
        busy_timeout_ms: 5_000,
        journal_mode: SqliteStoreMode::default(),
        sync_mode: SqliteSyncMode::default(),
        max_versions: None,
    };
    let store = EnterpriseSqliteStore::new(config)?;
    // Verify Deref resolves without panicking.
    let _deref = &*store;
    Ok(())
}
