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

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapePage;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeRegistryError;
use decision_gate_core::DataShapeSignature;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use rusqlite::Connection;
use rusqlite::ErrorCode;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::params;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// `SQLite` schema version for the store.
const SCHEMA_VERSION: i64 = 4;
/// Default busy timeout (ms).
const DEFAULT_BUSY_TIMEOUT_MS: u64 = 5_000;
/// Maximum length of a single path component.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;
/// Maximum run state snapshot size accepted by the store.
pub const MAX_STATE_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum schema payload size accepted by the registry.
pub const MAX_SCHEMA_BYTES: usize = 1024 * 1024;

/// Cursor payload for schema pagination.
#[derive(Debug, Serialize, Deserialize)]
struct RegistryCursor {
    /// Schema identifier for pagination.
    schema_id: String,
    /// Schema version for pagination.
    version: String,
}

// ============================================================================
// SECTION: Config
// ============================================================================

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

// ============================================================================
// SECTION: Errors
// ============================================================================

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
            SqliteStoreError::TooLarge { max_bytes, actual_bytes } => Self::Invalid(format!(
                "state_json exceeds size limit: {actual_bytes} bytes (max {max_bytes})"
            )),
        }
    }
}

// ============================================================================
// SECTION: Store
// ============================================================================

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
        Ok(Self { config, connection: Arc::new(Mutex::new(connection)) })
    }
}

impl RunStateStore for SqliteRunStateStore {
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        self.load_state(*tenant_id, *namespace_id, run_id).map_err(StoreError::from)
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        self.save_state(state).map_err(StoreError::from)
    }
}

impl DataShapeRegistry for SqliteRunStateStore {
    fn register(&self, record: DataShapeRecord) -> Result<(), DataShapeRegistryError> {
        let schema_bytes = canonical_json_bytes(&record.schema)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        if schema_bytes.len() > MAX_SCHEMA_BYTES {
            return Err(DataShapeRegistryError::Invalid(format!(
                "schema exceeds size limit: {} bytes (max {})",
                schema_bytes.len(),
                MAX_SCHEMA_BYTES
            )));
        }
        let schema_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &schema_bytes);
        let created_at_json = serde_json::to_string(&record.created_at)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let (signing_key_id, signing_signature, signing_algorithm) =
            record.signing.as_ref().map_or((None, None, None), |signing| {
                (
                    Some(signing.key_id.clone()),
                    Some(signing.signature.clone()),
                    signing.algorithm.clone(),
                )
            });
        let mut guard = self.connection.lock().map_err(|_| {
            DataShapeRegistryError::Io("schema registry mutex poisoned".to_string())
        })?;
        let result = {
            let tx =
                guard.transaction().map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            let result = tx.execute(
                "INSERT INTO data_shapes (
                    tenant_id, namespace_id, schema_id, version,
                    schema_json, schema_hash, hash_algorithm, description,
                    signing_key_id, signing_signature, signing_algorithm,
                    created_at_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    record.tenant_id.to_string(),
                    record.namespace_id.to_string(),
                    record.schema_id.as_str(),
                    record.version.as_str(),
                    schema_bytes,
                    schema_hash.value,
                    hash_algorithm_label(schema_hash.algorithm),
                    record.description.as_deref(),
                    signing_key_id.as_deref(),
                    signing_signature.as_deref(),
                    signing_algorithm.as_deref(),
                    created_at_json,
                ],
            );
            match result {
                Ok(_) => tx.commit().map_err(|err| DataShapeRegistryError::Io(err.to_string())),
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == ErrorCode::ConstraintViolation =>
                {
                    Err(DataShapeRegistryError::Conflict("schema already registered".to_string()))
                }
                Err(err) => Err(DataShapeRegistryError::Io(err.to_string())),
            }
        };
        drop(guard);
        result
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        schema_id: &DataShapeId,
        version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
        let mut guard = self.connection.lock().map_err(|_| {
            DataShapeRegistryError::Io("schema registry mutex poisoned".to_string())
        })?;
        let row = {
            let tx =
                guard.transaction().map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            let row = tx
                .query_row(
                    "SELECT schema_json, schema_hash, hash_algorithm, description, \
                     signing_key_id, signing_signature, signing_algorithm, created_at_json FROM \
                     data_shapes WHERE tenant_id = ?1 AND namespace_id = ?2 AND schema_id = ?3 \
                     AND version = ?4",
                    params![
                        tenant_id.to_string(),
                        namespace_id.to_string(),
                        schema_id.as_str(),
                        version.as_str()
                    ],
                    |row| {
                        let schema_json: Vec<u8> = row.get(0)?;
                        let schema_hash: String = row.get(1)?;
                        let hash_algorithm: String = row.get(2)?;
                        let description: Option<String> = row.get(3)?;
                        let signing_key_id: Option<String> = row.get(4)?;
                        let signing_signature: Option<String> = row.get(5)?;
                        let signing_algorithm: Option<String> = row.get(6)?;
                        let created_at_json: String = row.get(7)?;
                        Ok((
                            schema_json,
                            schema_hash,
                            hash_algorithm,
                            description,
                            signing_key_id,
                            signing_signature,
                            signing_algorithm,
                            created_at_json,
                        ))
                    },
                )
                .optional()
                .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            tx.commit().map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            row
        };
        drop(guard);
        let Some((
            schema_json,
            schema_hash,
            hash_algorithm,
            description,
            signing_key_id,
            signing_signature,
            signing_algorithm,
            created_at_json,
        )) = row
        else {
            return Ok(None);
        };
        let algorithm = parse_hash_algorithm(&hash_algorithm)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let expected = hash_bytes(algorithm, &schema_json);
        if expected.value != schema_hash {
            return Err(DataShapeRegistryError::Invalid("schema hash mismatch".to_string()));
        }
        let schema: serde_json::Value = serde_json::from_slice(&schema_json)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let created_at: Timestamp = serde_json::from_str(&created_at_json)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let signing = build_signing(signing_key_id, signing_signature, signing_algorithm);
        Ok(Some(DataShapeRecord {
            tenant_id: *tenant_id,
            namespace_id: *namespace_id,
            schema_id: schema_id.clone(),
            version: version.clone(),
            schema,
            description,
            created_at,
            signing,
        }))
    }

    fn list(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<DataShapePage, DataShapeRegistryError> {
        if limit == 0 {
            return Err(DataShapeRegistryError::Invalid(
                "schema list limit must be greater than zero".to_string(),
            ));
        }
        let limit = i64::try_from(limit)
            .map_err(|_| DataShapeRegistryError::Invalid("limit too large".to_string()))?;
        let cursor = cursor.map(|value| parse_registry_cursor(&value)).transpose()?;
        let mut guard = self.connection.lock().map_err(|_| {
            DataShapeRegistryError::Io("schema registry mutex poisoned".to_string())
        })?;
        let records = {
            let tx =
                guard.transaction().map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            let rows = query_schema_rows(&tx, *tenant_id, *namespace_id, cursor.as_ref(), limit)?;
            let records = rows
                .into_iter()
                .map(|row| build_schema_record(*tenant_id, *namespace_id, row))
                .collect::<Result<Vec<_>, _>>()?;
            tx.commit().map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            records
        };
        drop(guard);
        let next_token = match records.last() {
            Some(record) => {
                let cursor = RegistryCursor {
                    schema_id: record.schema_id.to_string(),
                    version: record.version.to_string(),
                };
                let token = serde_json::to_string(&cursor).map_err(|err| {
                    DataShapeRegistryError::Invalid(format!(
                        "failed to serialize registry cursor: {err}"
                    ))
                })?;
                Some(token)
            }
            None => None,
        };
        Ok(DataShapePage { items: records, next_token })
    }
}

impl SqliteRunStateStore {
    /// Loads run state for the provided run identifier.
    fn load_state(
        &self,
        tenant_id: TenantId,
        namespace_id: NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, SqliteStoreError> {
        let payload =
            fetch_run_state_payload(self.connection.as_ref(), tenant_id, namespace_id, run_id)?;
        let Some(payload) = payload else {
            return Ok(None);
        };
        let algorithm = parse_hash_algorithm(&payload.hash_algorithm)?;
        let expected = hash_bytes(algorithm, &payload.bytes);
        if expected.value != payload.hash_value {
            return Err(SqliteStoreError::Corrupt(format!(
                "hash mismatch for run {}",
                run_id.as_str()
            )));
        }
        let state: RunState = serde_json::from_slice(&payload.bytes)
            .map_err(|err| SqliteStoreError::Invalid(err.to_string()))?;
        if state.run_id.as_str() != run_id.as_str() {
            return Err(SqliteStoreError::Invalid(
                "run_id mismatch between key and payload".to_string(),
            ));
        }
        if state.tenant_id != tenant_id || state.namespace_id != namespace_id {
            return Err(SqliteStoreError::Invalid(
                "tenant/namespace mismatch between key and payload".to_string(),
            ));
        }
        Ok(Some(state))
    }

    /// Saves run state to the `SQLite` store.
    fn save_state(&self, state: &RunState) -> Result<(), SqliteStoreError> {
        let canonical_json = canonical_json_bytes(state)
            .map_err(|err| SqliteStoreError::Invalid(err.to_string()))?;
        if canonical_json.len() > MAX_STATE_BYTES {
            return Err(SqliteStoreError::TooLarge {
                max_bytes: MAX_STATE_BYTES,
                actual_bytes: canonical_json.len(),
            });
        }
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
                    "SELECT latest_version FROM runs WHERE tenant_id = ?1 AND namespace_id = ?2 \
                     AND run_id = ?3",
                    params![
                        state.tenant_id.to_string(),
                        state.namespace_id.to_string(),
                        state.run_id.as_str()
                    ],
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
                "INSERT INTO runs (tenant_id, namespace_id, run_id, latest_version) VALUES (?1, \
                 ?2, ?3, ?4) ON CONFLICT(tenant_id, namespace_id, run_id) DO UPDATE SET \
                 latest_version = excluded.latest_version",
                params![
                    state.tenant_id.to_string(),
                    state.namespace_id.to_string(),
                    state.run_id.as_str(),
                    next_version
                ],
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            tx.execute(
                "INSERT INTO run_state_versions (tenant_id, namespace_id, run_id, version, \
                 state_json, state_hash, hash_algorithm, saved_at) VALUES (?1, ?2, ?3, ?4, ?5, \
                 ?6, ?7, ?8)",
                params![
                    state.tenant_id.to_string(),
                    state.namespace_id.to_string(),
                    state.run_id.as_str(),
                    next_version,
                    canonical_json,
                    digest.value,
                    hash_algorithm_label(digest.algorithm),
                    saved_at
                ],
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            enforce_retention(
                &tx,
                state.tenant_id,
                state.namespace_id,
                state.run_id.as_str(),
                next_version,
                self.config.max_versions,
            )?;
            tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            drop(guard);
        }
        Ok(())
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

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
                    tenant_id TEXT NOT NULL,
                    namespace_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    latest_version INTEGER NOT NULL,
                    PRIMARY KEY (tenant_id, namespace_id, run_id)
                );
                CREATE TABLE IF NOT EXISTS run_state_versions (
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
                CREATE INDEX IF NOT EXISTS idx_run_state_versions_run_id
                    ON run_state_versions (tenant_id, namespace_id, run_id);
                CREATE TABLE IF NOT EXISTS data_shapes (
                    tenant_id TEXT NOT NULL,
                    namespace_id TEXT NOT NULL,
                    schema_id TEXT NOT NULL,
                    version TEXT NOT NULL,
                    schema_json BLOB NOT NULL,
                    schema_hash TEXT NOT NULL,
                    hash_algorithm TEXT NOT NULL,
                    description TEXT,
                    signing_key_id TEXT,
                    signing_signature TEXT,
                    signing_algorithm TEXT,
                    created_at_json TEXT NOT NULL,
                    PRIMARY KEY (tenant_id, namespace_id, schema_id, version)
                );
                CREATE INDEX IF NOT EXISTS idx_data_shapes_namespace
                    ON data_shapes (tenant_id, namespace_id, schema_id, version);",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        }
        Some(3) => {
            tx.execute_batch(
                "ALTER TABLE data_shapes ADD COLUMN signing_key_id TEXT;
                 ALTER TABLE data_shapes ADD COLUMN signing_signature TEXT;
                 ALTER TABLE data_shapes ADD COLUMN signing_algorithm TEXT;",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            tx.execute("UPDATE store_meta SET version = ?1", params![SCHEMA_VERSION])
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
    tenant_id: TenantId,
    namespace_id: NamespaceId,
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
            "DELETE FROM run_state_versions WHERE tenant_id = ?1 AND namespace_id = ?2 AND run_id \
             = ?3 AND version < ?4",
            params![tenant_id.to_string(), namespace_id.to_string(), run_id, min_version],
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

/// Raw payload for a stored run state.
#[derive(Debug)]
struct RunStatePayload {
    /// Stored JSON bytes for the run state.
    bytes: Vec<u8>,
    /// Stored hash value for the payload.
    hash_value: String,
    /// Stored hash algorithm label.
    hash_algorithm: String,
}

/// Fetches the latest run state payload for the provided run identifiers.
fn fetch_run_state_payload(
    connection: &Mutex<Connection>,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    run_id: &RunId,
) -> Result<Option<RunStatePayload>, SqliteStoreError> {
    let mut guard =
        connection.lock().map_err(|_| SqliteStoreError::Db("mutex poisoned".to_string()))?;
    let payload = {
        let tx = guard.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let latest_version: Option<i64> = tx
            .query_row(
                "SELECT latest_version FROM runs WHERE tenant_id = ?1 AND namespace_id = ?2 AND \
                 run_id = ?3",
                params![tenant_id.to_string(), namespace_id.to_string(), run_id.as_str()],
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
        let payload = if let Some(latest_version) = latest_version {
            let metadata = tx
                .query_row(
                    "SELECT length(state_json), state_hash, hash_algorithm FROM \
                     run_state_versions WHERE tenant_id = ?1 AND namespace_id = ?2 AND run_id = \
                     ?3 AND version = ?4",
                    params![
                        tenant_id.to_string(),
                        namespace_id.to_string(),
                        run_id.as_str(),
                        latest_version
                    ],
                    |row| {
                        let length: i64 = row.get(0)?;
                        let hash: String = row.get(1)?;
                        let algorithm: String = row.get(2)?;
                        Ok((length, hash, algorithm))
                    },
                )
                .optional()
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let (length, hash, algorithm) = metadata.ok_or_else(|| {
                SqliteStoreError::Corrupt(format!(
                    "missing run state version {latest_version} for run {}",
                    run_id.as_str()
                ))
            })?;
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
                    "SELECT state_json FROM run_state_versions WHERE tenant_id = ?1 AND \
                     namespace_id = ?2 AND run_id = ?3 AND version = ?4",
                    params![
                        tenant_id.to_string(),
                        namespace_id.to_string(),
                        run_id.as_str(),
                        latest_version
                    ],
                    |row| row.get(0),
                )
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            Some(RunStatePayload { bytes, hash_value: hash, hash_algorithm: algorithm })
        } else {
            None
        };
        tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        payload
    };
    drop(guard);
    Ok(payload)
}

/// Parses a pagination cursor payload.
fn parse_registry_cursor(cursor: &str) -> Result<RegistryCursor, DataShapeRegistryError> {
    serde_json::from_str(cursor)
        .map_err(|_| DataShapeRegistryError::Invalid("invalid cursor".to_string()))
}

/// Schema row data loaded from the registry.
#[derive(Debug)]
struct SchemaRow {
    /// Schema identifier string.
    schema_id: String,
    /// Schema version string.
    version: String,
    /// Canonical schema bytes.
    schema_json: Vec<u8>,
    /// Stored schema hash value.
    schema_hash: String,
    /// Stored hash algorithm label.
    hash_algorithm: String,
    /// Optional schema description.
    description: Option<String>,
    /// Optional signing key id.
    signing_key_id: Option<String>,
    /// Optional signing signature.
    signing_signature: Option<String>,
    /// Optional signing algorithm.
    signing_algorithm: Option<String>,
    /// JSON-encoded creation timestamp.
    created_at_json: String,
}

/// Maps a `SQLite` row into a schema row payload.
fn map_schema_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SchemaRow> {
    Ok(SchemaRow {
        schema_id: row.get(0)?,
        version: row.get(1)?,
        schema_json: row.get(2)?,
        schema_hash: row.get(3)?,
        hash_algorithm: row.get(4)?,
        description: row.get(5)?,
        signing_key_id: row.get(6)?,
        signing_signature: row.get(7)?,
        signing_algorithm: row.get(8)?,
        created_at_json: row.get(9)?,
    })
}

/// Maps `SQLite` errors to registry errors.
fn map_registry_error(err: &rusqlite::Error) -> DataShapeRegistryError {
    DataShapeRegistryError::Io(err.to_string())
}

/// Builds a `DataShapeSignature` when required fields are present and non-empty.
fn build_signing(
    key_id: Option<String>,
    signature: Option<String>,
    algorithm: Option<String>,
) -> Option<DataShapeSignature> {
    match (key_id, signature) {
        (Some(key_id), Some(signature))
            if !key_id.trim().is_empty() && !signature.trim().is_empty() =>
        {
            Some(DataShapeSignature { key_id, signature, algorithm })
        }
        _ => None,
    }
}

/// Queries schema rows for the provided tenant and namespace.
fn query_schema_rows(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    cursor: Option<&RegistryCursor>,
    limit: i64,
) -> Result<Vec<SchemaRow>, DataShapeRegistryError> {
    if let Some(cursor) = cursor {
        let mut stmt = tx
            .prepare(
                "SELECT schema_id, version, schema_json, schema_hash, hash_algorithm, \
                 description, signing_key_id, signing_signature, signing_algorithm, \
                 created_at_json FROM data_shapes WHERE tenant_id = ?1 AND namespace_id = ?2 AND \
                 (schema_id > ?3 OR (schema_id = ?3 AND version > ?4)) ORDER BY schema_id, \
                 version LIMIT ?5",
            )
            .map_err(|err| map_registry_error(&err))?;
        let rows = stmt
            .query_map(
                params![
                    tenant_id.to_string(),
                    namespace_id.to_string(),
                    cursor.schema_id.as_str(),
                    cursor.version.as_str(),
                    limit
                ],
                map_schema_row,
            )
            .map_err(|err| map_registry_error(&err))?;
        rows.map(|row| row.map_err(|err| map_registry_error(&err))).collect()
    } else {
        let mut stmt = tx
            .prepare(
                "SELECT schema_id, version, schema_json, schema_hash, hash_algorithm, \
                 description, signing_key_id, signing_signature, signing_algorithm, \
                 created_at_json FROM data_shapes WHERE tenant_id = ?1 AND namespace_id = ?2 \
                 ORDER BY schema_id, version LIMIT ?3",
            )
            .map_err(|err| map_registry_error(&err))?;
        let rows = stmt
            .query_map(
                params![tenant_id.to_string(), namespace_id.to_string(), limit],
                map_schema_row,
            )
            .map_err(|err| map_registry_error(&err))?;
        rows.map(|row| row.map_err(|err| map_registry_error(&err))).collect()
    }
}

/// Builds a validated schema record from stored row data.
fn build_schema_record(
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    row: SchemaRow,
) -> Result<DataShapeRecord, DataShapeRegistryError> {
    let SchemaRow {
        schema_id,
        version,
        schema_json,
        schema_hash,
        hash_algorithm,
        description,
        signing_key_id,
        signing_signature,
        signing_algorithm,
        created_at_json,
    } = row;
    let algorithm = parse_hash_algorithm(&hash_algorithm)
        .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
    let expected = hash_bytes(algorithm, &schema_json);
    if expected.value != schema_hash {
        return Err(DataShapeRegistryError::Invalid("schema hash mismatch".to_string()));
    }
    let schema: serde_json::Value = serde_json::from_slice(&schema_json)
        .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
    let created_at: Timestamp = serde_json::from_str(&created_at_json)
        .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
    let signing = build_signing(signing_key_id, signing_signature, signing_algorithm);
    Ok(DataShapeRecord {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::from(schema_id),
        version: DataShapeVersion::from(version),
        schema,
        description,
        created_at,
        signing,
    })
}
