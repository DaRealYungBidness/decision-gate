// enterprise/decision-gate-store-enterprise/src/sqlite_store.rs
// ============================================================================
// Module: Enterprise SQLite Store
// Description: Wrapper around Decision Gate SQLite store for managed deployments.
// Purpose: Provide a durable, multi-tenant store using SQLite for early phases.
// ============================================================================

use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use decision_gate_store_sqlite::SqliteStoreError;

/// Enterprise `SQLite` store wrapper.
///
/// This is suitable for single-node managed deployments and test environments.
pub struct EnterpriseSqliteStore {
    /// Inner `SQLite` run state store.
    inner: SqliteRunStateStore,
}

impl EnterpriseSqliteStore {
    /// Creates a new enterprise `SQLite` store.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] when initialization fails.
    pub fn new(config: SqliteStoreConfig) -> Result<Self, SqliteStoreError> {
        let inner = SqliteRunStateStore::new(config)?;
        Ok(Self {
            inner,
        })
    }

    /// Returns a reference to the underlying store.
    #[must_use]
    pub const fn inner(&self) -> &SqliteRunStateStore {
        &self.inner
    }
}

impl std::ops::Deref for EnterpriseSqliteStore {
    type Target = SqliteRunStateStore;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
