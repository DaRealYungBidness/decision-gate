// enterprise/decision-gate-enterprise/src/usage_sqlite.rs
// ============================================================================
// Module: SQLite Usage Ledger
// Description: SQLite-backed usage ledger for managed deployments.
// Purpose: Provide persistent usage accounting with idempotency support.
// ============================================================================

use std::path::Path;
use std::sync::Mutex;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_mcp::UsageMetric;
use rusqlite::Connection;
use rusqlite::params;
use thiserror::Error;

use crate::usage::UsageEvent;
use crate::usage::UsageLedger;
use crate::usage::UsageLedgerError;
use crate::usage::metric_label;

/// `SQLite` usage ledger errors.
#[derive(Debug, Error)]
pub enum SqliteUsageLedgerError {
    /// Underlying `SQLite` error.
    #[error("sqlite usage ledger error: {0}")]
    Sqlite(String),
}

/// SQLite-backed usage ledger.
pub struct SqliteUsageLedger {
    /// Shared `SQLite` connection.
    connection: Mutex<Connection>,
}

impl SqliteUsageLedger {
    /// Opens a `SQLite` usage ledger at the provided path.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteUsageLedgerError`] when initialization fails.
    pub fn new(path: &Path) -> Result<Self, SqliteUsageLedgerError> {
        let conn = Connection::open(path)
            .map_err(|err| SqliteUsageLedgerError::Sqlite(err.to_string()))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;CREATE TABLE IF NOT EXISTS \
             usage_events (tenant_id TEXT NOT NULL,namespace_id TEXT NOT NULL,metric TEXT NOT \
             NULL,units INTEGER NOT NULL,timestamp_ms INTEGER NOT NULL,idempotency_key \
             TEXT);CREATE INDEX IF NOT EXISTS idx_usage_events_scope ON usage_events(tenant_id, \
             namespace_id, metric, timestamp_ms);CREATE UNIQUE INDEX IF NOT EXISTS \
             idx_usage_events_idempotency ON usage_events(idempotency_key) WHERE idempotency_key \
             IS NOT NULL;",
        )
        .map_err(|err| SqliteUsageLedgerError::Sqlite(err.to_string()))?;
        Ok(Self {
            connection: Mutex::new(conn),
        })
    }
}

impl UsageLedger for SqliteUsageLedger {
    fn append(&self, event: UsageEvent) -> Result<(), UsageLedgerError> {
        let metric_label = metric_label(event.metric);
        let idempotency = event.idempotency_key.as_deref();
        {
            let conn = self
                .connection
                .lock()
                .map_err(|_| UsageLedgerError::Storage("usage ledger lock poisoned".to_string()))?;
            conn.execute(
                "INSERT OR IGNORE INTO usage_events (tenant_id, namespace_id, metric, units, \
                 timestamp_ms, idempotency_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    event.tenant_id.as_str(),
                    event.namespace_id.as_str(),
                    metric_label,
                    i64::try_from(event.units).unwrap_or(i64::MAX),
                    i64::try_from(event.timestamp_ms).unwrap_or(i64::MAX),
                    idempotency,
                ],
            )
            .map_err(|err| UsageLedgerError::Storage(err.to_string()))?;
        }
        Ok(())
    }

    fn sum_since(
        &self,
        scope_key: &str,
        metric: UsageMetric,
        since_ms: u128,
    ) -> Result<u64, UsageLedgerError> {
        let mut parts = scope_key.splitn(2, '/');
        let tenant = parts.next().unwrap_or("default");
        let namespace = parts.next();
        let metric_label = metric_label(metric);
        let sum: Option<i64> = {
            let conn = self
                .connection
                .lock()
                .map_err(|_| UsageLedgerError::Storage("usage ledger lock poisoned".to_string()))?;
            let sum = if namespace == Some("*") || namespace.is_none() {
                let mut stmt = conn
                    .prepare(
                        "SELECT SUM(units) FROM usage_events WHERE tenant_id = ?1 AND metric = ?2 \
                         AND timestamp_ms >= ?3",
                    )
                    .map_err(|err| UsageLedgerError::Storage(err.to_string()))?;
                stmt.query_row(
                    params![tenant, metric_label, i64::try_from(since_ms).unwrap_or(i64::MAX),],
                    |row| row.get(0),
                )
                .map_err(|err| UsageLedgerError::Storage(err.to_string()))?
            } else {
                let mut stmt = conn
                    .prepare(
                        "SELECT SUM(units) FROM usage_events WHERE tenant_id = ?1 AND \
                         namespace_id = ?2 AND metric = ?3 AND timestamp_ms >= ?4",
                    )
                    .map_err(|err| UsageLedgerError::Storage(err.to_string()))?;
                stmt.query_row(
                    params![
                        tenant,
                        namespace.unwrap_or(""),
                        metric_label,
                        i64::try_from(since_ms).unwrap_or(i64::MAX),
                    ],
                    |row| row.get(0),
                )
                .map_err(|err| UsageLedgerError::Storage(err.to_string()))?
            };
            drop(conn);
            sum
        };
        Ok(sum.unwrap_or(0).try_into().unwrap_or(u64::MAX))
    }

    fn seen_idempotency(&self, key: &str) -> Result<bool, UsageLedgerError> {
        let exists: Option<i64> = {
            let conn = self
                .connection
                .lock()
                .map_err(|_| UsageLedgerError::Storage("usage ledger lock poisoned".to_string()))?;
            let mut stmt = conn
                .prepare("SELECT 1 FROM usage_events WHERE idempotency_key = ?1 LIMIT 1")
                .map_err(|err| UsageLedgerError::Storage(err.to_string()))?;
            let exists = stmt.query_row(params![key], |row| row.get(0)).unwrap_or(None);
            drop(stmt);
            drop(conn);
            exists
        };
        Ok(exists.is_some())
    }
}

/// Helper to build a usage event from raw values.
#[must_use]
pub const fn usage_event(
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    metric: UsageMetric,
    units: u64,
    timestamp_ms: u128,
    idempotency_key: Option<String>,
) -> UsageEvent {
    UsageEvent {
        tenant_id,
        namespace_id,
        metric,
        units,
        timestamp_ms,
        idempotency_key,
    }
}
