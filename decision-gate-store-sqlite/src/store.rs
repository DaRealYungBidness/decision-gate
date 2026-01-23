// decision-gate-store-sqlite/src/store.rs
// ============================================================================
// Module: SQLite Run State Store
// Description: Durable RunStateStore backed by SQLite WAL.
// Purpose: Persist run state snapshots with deterministic serialization.
// Dependencies: decision-gate-core, rusqlite, serde, serde_json, thiserror
// ============================================================================

//! ## Overview
//! This module implements a durable [`RunStateStore`] using `SQLite`. Each save
//! produces a canonical JSON snapshot stored in an append-only version table.
//! Loads verify integrity via stored hashes and fail closed on corruption.
//! Security posture: database contents are untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================//
// SECTION: Imports
// ============================================================================//

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::StoreError;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::params;
use serde::Deserialize;
use thiserror::Error;

// ============================================================================//
// SECTION: Constants
// ============================================================================//

/// `SQLite` schema version for the store.
const SCHEMA_VERSION: i64 = 1;
/// Default busy timeout (ms).
const DEFAULT_BUSY_TIMEOUT_MS: u64 = 5_000;
/// Maximum length of a single path component.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;
/// Maximum run state snapshot size accepted by the store.
pub const MAX_STATE_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;

// ============================================================================//
// SECTION: Config
// ============================================================================//

/// `SQLite` journal mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SqliteStoreMode {
    /// WAL journal mode (recommended).
    #[default]
    Wal,
    /// Delete journal mode (legacy).
    Delete,
}

impl SqliteStoreMode {
    /// Returns the `SQLite` pragma value.
    #[must_use]
    pub const fn pragma_value(self) -> &'static str {
        match self {
            Self::Wal => "wal",
            Self::Delete => "delete",
        }
    }
}

/// `SQLite` sync mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SqliteSyncMode {
    /// Full synchronous mode (safest).
    #[default]
    Full,
    /// Normal synchronous mode (balanced).
    Normal,
}

impl SqliteSyncMode {
    /// Returns the `SQLite` pragma value.
    #[must_use]
    pub const fn pragma_value(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Normal => "normal",
        }
    }
}

/// Configuration for the `SQLite` run state store.
#[derive(Debug, Clone, Deserialize)]
pub struct SqliteStoreConfig {
    /// Path to the `SQLite` database file.
    pub path: PathBuf,
    /// Busy timeout in milliseconds.
    #[serde(default = "default_busy_timeout_ms")]
    pub busy_timeout_ms: u64,
    /// `SQLite` journal mode.
    #[serde(default)]
    pub journal_mode: SqliteStoreMode,
    /// `SQLite` sync mode.
    #[serde(default)]
    pub sync_mode: SqliteSyncMode,
    /// Optional maximum versions per run (older versions pruned).
    #[serde(default)]
    pub max_versions: Option<u64>,
}

/// Returns the default busy timeout for `SQLite` connections.
const fn default_busy_timeout_ms() -> u64 {
    DEFAULT_BUSY_TIMEOUT_MS
}

// ============================================================================//
// SECTION: Errors
// ============================================================================//

/// `SQLite` store errors.
#[derive(Debug, Error)]
pub enum SqliteStoreError {
    /// Store I/O error.
    #[error("sqlite store io error: {0}")]
    Io(String),
    /// `SQLite` engine error.
    #[error("sqlite store db error: {0}")]
    Db(String),
    /// Store corruption or hash mismatch.
    #[error("sqlite store corruption: {0}")]
    Corrupt(String),
    /// Store schema version mismatch.
    #[error("sqlite store version mismatch: {0}")]
    VersionMismatch(String),
    /// Invalid store data.
    #[error("sqlite store invalid data: {0}")]
    Invalid(String),
    /// Store payload exceeded configured size limits.
    #[error("sqlite store payload too large: {actual_bytes} bytes (max {max_bytes})")]
    TooLarge {
        /// Maximum allowed bytes.
        max_bytes: usize,
        /// Actual payload size in bytes.
        actual_bytes: usize,
    },
}

impl From<SqliteStoreError> for StoreError {
    fn from(error: SqliteStoreError) -> Self {
        match error {
            SqliteStoreError::Io(message) => Self::Io(message),
            SqliteStoreError::Db(message) => Self::Store(message),
            SqliteStoreError::Corrupt(message) => Self::Corrupt(message),
            SqliteStoreError::VersionMismatch(message) => Self::VersionMismatch(message),
            SqliteStoreError::Invalid(message) => Self::Invalid(message),
            SqliteStoreError::TooLarge {
                max_bytes,
                actual_bytes,
            } => Self::Invalid(format!(
                "state_json exceeds size limit: {actual_bytes} bytes (max {max_bytes})"
            )),
        }
    }
}

// ============================================================================//
// SECTION: Store
// ============================================================================//

/// `SQLite`-backed run state store with WAL support.
#[derive(Clone)]
pub struct SqliteRunStateStore {
    /// Store configuration.
    config: SqliteStoreConfig,
    /// Shared `SQLite` connection guarded by a mutex.
    connection: Arc<Mutex<Connection>>,
}

impl SqliteRunStateStore {
    /// Opens an `SQLite`-backed run state store.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] when the database cannot be opened or
    /// initialized.
    pub fn new(config: SqliteStoreConfig) -> Result<Self, SqliteStoreError> {
        validate_store_path(&config.path)?;
        ensure_parent_dir(&config.path)?;
        let mut connection = open_connection(&config)?;
        initialize_schema(&mut connection)?;
        Ok(Self {
            config,
            connection: Arc::new(Mutex::new(connection)),
        })
    }
}

impl RunStateStore for SqliteRunStateStore {
    fn load(&self, run_id: &RunId) -> Result<Option<RunState>, StoreError> {
        self.load_state(run_id).map_err(StoreError::from)
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        self.save_state(state).map_err(StoreError::from)
    }
}

impl SqliteRunStateStore {
    /// Loads run state for the provided run identifier.
    fn load_state(&self, run_id: &RunId) -> Result<Option<RunState>, SqliteStoreError> {
        let row = {
            let mut guard = self
                .connection
                .lock()
                .map_err(|_| SqliteStoreError::Db("mutex poisoned".to_string()))?;
            let tx = guard.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let latest_version: Option<i64> = tx
                .query_row(
                    "SELECT latest_version FROM runs WHERE run_id = ?1",
                    params![run_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let latest_version = match latest_version {
                None => None,
                Some(value) => {
                    if value < 1 {
                        return Err(SqliteStoreError::Corrupt(format!(
                            "invalid latest_version for run {}",
                            run_id.as_str()
                        )));
                    }
                    Some(value)
                }
            };
            let row = if let Some(latest_version) = latest_version {
                let metadata = tx
                    .query_row(
                        "SELECT length(state_json), state_hash, hash_algorithm FROM \
                         run_state_versions WHERE run_id = ?1 AND version = ?2",
                        params![run_id.as_str(), latest_version],
                        |row| {
                            let length: i64 = row.get(0)?;
                            let hash: String = row.get(1)?;
                            let algorithm: String = row.get(2)?;
                            Ok((length, hash, algorithm))
                        },
                    )
                    .optional()
                    .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
                let Some((length, hash, algorithm)) = metadata else {
                    return Err(SqliteStoreError::Corrupt(format!(
                        "missing run state version {latest_version} for run {}",
                        run_id.as_str()
                    )));
                };
                let length_usize = usize::try_from(length).map_err(|_| {
                    SqliteStoreError::Invalid(format!(
                        "negative run state length for run {}",
                        run_id.as_str()
                    ))
                })?;
                if length_usize > MAX_STATE_BYTES {
                    return Err(SqliteStoreError::TooLarge {
                        max_bytes: MAX_STATE_BYTES,
                        actual_bytes: length_usize,
                    });
                }
                let bytes: Vec<u8> = tx
                    .query_row(
                        "SELECT state_json FROM run_state_versions WHERE run_id = ?1 AND version \
                         = ?2",
                        params![run_id.as_str(), latest_version],
                        |row| row.get(0),
                    )
                    .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
                Some((bytes, hash, algorithm))
            } else {
                None
            };
            tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            drop(guard);
            row
        };
        let Some((bytes, hash_value, hash_algorithm)) = row else {
            return Ok(None);
        };
        let algorithm = parse_hash_algorithm(&hash_algorithm)?;
        let expected = hash_bytes(algorithm, &bytes);
        if expected.value != hash_value {
            return Err(SqliteStoreError::Corrupt(format!(
                "hash mismatch for run {}",
                run_id.as_str()
            )));
        }
        let state: RunState = serde_json::from_slice(&bytes)
            .map_err(|err| SqliteStoreError::Invalid(err.to_string()))?;
        if state.run_id.as_str() != run_id.as_str() {
            return Err(SqliteStoreError::Invalid(
                "run_id mismatch between key and payload".to_string(),
            ));
        }
        Ok(Some(state))
    }

    /// Saves run state to the `SQLite` store.
    fn save_state(&self, state: &RunState) -> Result<(), SqliteStoreError> {
        let canonical_json = canonical_json_bytes(state)
            .map_err(|err| SqliteStoreError::Invalid(err.to_string()))?;
        let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &canonical_json);
        let saved_at = unix_millis();
        {
            let mut guard = self
                .connection
                .lock()
                .map_err(|_| SqliteStoreError::Db("mutex poisoned".to_string()))?;
            let tx = guard.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let latest_version: Option<i64> = tx
                .query_row(
                    "SELECT latest_version FROM runs WHERE run_id = ?1",
                    params![state.run_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let next_version = match latest_version {
                None => 1,
                Some(value) => {
                    if value < 1 {
                        return Err(SqliteStoreError::Corrupt(format!(
                            "invalid latest_version for run {}",
                            state.run_id.as_str()
                        )));
                    }
                    value.checked_add(1).ok_or_else(|| {
                        SqliteStoreError::Corrupt(format!(
                            "run state version overflow for run {}",
                            state.run_id.as_str()
                        ))
                    })?
                }
            };
            tx.execute(
                "INSERT INTO runs (run_id, latest_version) VALUES (?1, ?2) ON CONFLICT(run_id) DO \
                 UPDATE SET latest_version = excluded.latest_version",
                params![state.run_id.as_str(), next_version],
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            tx.execute(
                "INSERT INTO run_state_versions (run_id, version, state_json, state_hash, \
                 hash_algorithm, saved_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    state.run_id.as_str(),
                    next_version,
                    canonical_json,
                    digest.value,
                    hash_algorithm_label(digest.algorithm),
                    saved_at
                ],
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            enforce_retention(&tx, state.run_id.as_str(), next_version, self.config.max_versions)?;
            tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            drop(guard);
        }
        Ok(())
    }
}

// ============================================================================//
// SECTION: Helpers
// ============================================================================//

/// Ensures the parent directory for the store exists.
fn ensure_parent_dir(path: &Path) -> Result<(), SqliteStoreError> {
    let Some(parent) = path.parent() else {
        return Err(SqliteStoreError::Io("store path missing parent directory".to_string()));
    };
    std::fs::create_dir_all(parent).map_err(|err| SqliteStoreError::Io(err.to_string()))
}

/// Validates store paths for safety limits.
fn validate_store_path(path: &Path) -> Result<(), SqliteStoreError> {
    let path_string = path.display().to_string();
    if path_string.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(SqliteStoreError::Invalid("store path exceeds length limit".to_string()));
    }
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(SqliteStoreError::Invalid(
                "store path contains an overlong component".to_string(),
            ));
        }
    }
    if path.exists() && path.is_dir() {
        return Err(SqliteStoreError::Invalid(
            "store path must be a file, not a directory".to_string(),
        ));
    }
    Ok(())
}

/// Opens an `SQLite` connection with secure defaults.
fn open_connection(config: &SqliteStoreConfig) -> Result<Connection, SqliteStoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_FULL_MUTEX;
    let connection = Connection::open_with_flags(&config.path, flags)
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    apply_pragmas(&connection, config)?;
    Ok(connection)
}

/// Applies `SQLite` pragmas required for durability.
fn apply_pragmas(
    connection: &Connection,
    config: &SqliteStoreConfig,
) -> Result<(), SqliteStoreError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    connection
        .execute_batch(&format!("PRAGMA journal_mode = {};", config.journal_mode.pragma_value()))
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    connection
        .execute_batch(&format!("PRAGMA synchronous = {};", config.sync_mode.pragma_value()))
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    connection
        .busy_timeout(std::time::Duration::from_millis(config.busy_timeout_ms))
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    Ok(())
}

/// Initializes the `SQLite` schema or validates existing version.
fn initialize_schema(connection: &mut Connection) -> Result<(), SqliteStoreError> {
    let tx = connection.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS store_meta (version INTEGER NOT NULL);")
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    let version: Option<i64> = tx
        .query_row("SELECT version FROM store_meta LIMIT 1", params![], |row| row.get(0))
        .optional()
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    match version {
        None => {
            tx.execute("INSERT INTO store_meta (version) VALUES (?1)", params![SCHEMA_VERSION])
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS runs (
                    run_id TEXT PRIMARY KEY,
                    latest_version INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS run_state_versions (
                    run_id TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    state_json BLOB NOT NULL,
                    state_hash TEXT NOT NULL,
                    hash_algorithm TEXT NOT NULL,
                    saved_at INTEGER NOT NULL,
                    PRIMARY KEY (run_id, version),
                    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_run_state_versions_run_id
                    ON run_state_versions (run_id);",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        }
        Some(value) if value == SCHEMA_VERSION => {}
        Some(value) => {
            return Err(SqliteStoreError::VersionMismatch(format!(
                "unsupported schema version: {value}"
            )));
        }
    }
    tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    Ok(())
}

/// Enforces version retention if configured.
fn enforce_retention(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
    latest_version: i64,
    max_versions: Option<u64>,
) -> Result<(), SqliteStoreError> {
    let Some(max_versions) = max_versions else {
        return Ok(());
    };
    if max_versions == 0 {
        return Err(SqliteStoreError::Invalid(
            "max_versions must be greater than zero".to_string(),
        ));
    }
    let max_versions = i64::try_from(max_versions)
        .map_err(|_| SqliteStoreError::Invalid("max_versions too large".to_string()))?;
    if latest_version > max_versions {
        let min_version = latest_version - max_versions + 1;
        tx.execute(
            "DELETE FROM run_state_versions WHERE run_id = ?1 AND version < ?2",
            params![run_id, min_version],
        )
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    }
    Ok(())
}

/// Returns the current unix epoch in milliseconds.
fn unix_millis() -> i64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    i64::try_from(now.as_millis()).unwrap_or(i64::MAX)
}

/// Returns the canonical hash algorithm label.
const fn hash_algorithm_label(algorithm: HashAlgorithm) -> &'static str {
    match algorithm {
        HashAlgorithm::Sha256 => "sha256",
    }
}

/// Parses a hash algorithm label.
fn parse_hash_algorithm(label: &str) -> Result<HashAlgorithm, SqliteStoreError> {
    match label {
        "sha256" => Ok(HashAlgorithm::Sha256),
        other => Err(SqliteStoreError::Invalid(format!("unsupported hash algorithm: {other}"))),
    }
}
