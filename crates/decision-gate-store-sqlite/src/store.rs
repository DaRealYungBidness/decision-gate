// crates/decision-gate-store-sqlite/src/store.rs
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
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::TrySendError;
use std::thread;
use std::time::Instant;
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
const SCHEMA_VERSION: i64 = 5;
/// Default busy timeout (ms).
const DEFAULT_BUSY_TIMEOUT_MS: u64 = 5_000;
/// Maximum length of a single path component.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;
/// Maximum run state snapshot size accepted by the store.
pub const MAX_STATE_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum schema payload size accepted by the registry.
/// Acts as a hard upper bound for configurable registry limits.
pub const MAX_SCHEMA_BYTES: usize = 1024 * 1024;
/// Millisecond bucket boundaries used for lightweight store perf snapshots.
const PERF_BUCKETS_MS: [u64; 10] = [1, 2, 5, 10, 20, 50, 100, 250, 500, 1_000];
/// Bucket boundaries used for writer queue depth histograms.
const WRITER_QUEUE_DEPTH_BUCKETS: [u64; 10] = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256];
/// Bucket boundaries used for writer batch size histograms.
const WRITER_BATCH_SIZE_BUCKETS: [u64; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
/// Microsecond bucket boundaries used for writer wait/commit histograms.
const WRITER_TIME_BUCKETS_US: [u64; 10] =
    [100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000];
/// Microsecond bucket boundaries used for read-pool lock wait histograms.
const READ_WAIT_TIME_BUCKETS_US: [u64; 10] =
    [100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000];

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
///
/// # Invariants
/// - Values map 1:1 to `SQLite` `journal_mode` pragma settings.
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
///
/// # Invariants
/// - Values map 1:1 to `SQLite` `synchronous` pragma settings.
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
///
/// # Invariants
/// - `path` must resolve to a file path (not a directory).
/// - `busy_timeout_ms` is interpreted as milliseconds.
/// - `max_versions`, when set, must be greater than zero.
/// - `schema_registry_max_schema_bytes`, when set, must be greater than zero and no more than
///   [`MAX_SCHEMA_BYTES`].
/// - `schema_registry_max_entries`, when set, must be greater than zero.
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
    /// Optional maximum schema payload size in bytes.
    #[serde(default)]
    pub schema_registry_max_schema_bytes: Option<usize>,
    /// Optional maximum number of schemas per tenant + namespace.
    #[serde(default)]
    pub schema_registry_max_entries: Option<usize>,
    /// Writer queue capacity.
    #[serde(default = "default_writer_queue_capacity")]
    pub writer_queue_capacity: usize,
    /// Maximum number of operations in a single writer batch.
    #[serde(default = "default_batch_max_ops")]
    pub batch_max_ops: usize,
    /// Maximum aggregate command bytes in a single writer batch.
    #[serde(default = "default_batch_max_bytes")]
    pub batch_max_bytes: usize,
    /// Maximum wait window for writer batching (milliseconds).
    #[serde(default = "default_batch_max_wait_ms")]
    pub batch_max_wait_ms: u64,
    /// Number of read-only connections used for read path isolation.
    #[serde(default = "default_read_pool_size")]
    pub read_pool_size: usize,
}

/// Returns the default busy timeout for `SQLite` connections.
const fn default_busy_timeout_ms() -> u64 {
    DEFAULT_BUSY_TIMEOUT_MS
}

/// Returns the default writer queue capacity.
const fn default_writer_queue_capacity() -> usize {
    1_024
}

/// Returns the default batch max operation count.
const fn default_batch_max_ops() -> usize {
    64
}

/// Returns the default batch max byte count.
const fn default_batch_max_bytes() -> usize {
    512 * 1024
}

/// Returns the default writer batch max wait window in milliseconds.
const fn default_batch_max_wait_ms() -> u64 {
    2
}

/// Returns the default read connection pool size.
const fn default_read_pool_size() -> usize {
    4
}

/// Validates schema registry limits in the store configuration.
fn validate_schema_registry_limits(config: &SqliteStoreConfig) -> Result<(), SqliteStoreError> {
    if let Some(max_bytes) = config.schema_registry_max_schema_bytes
        && (max_bytes == 0 || max_bytes > MAX_SCHEMA_BYTES)
    {
        return Err(SqliteStoreError::Invalid(format!(
            "schema_registry_max_schema_bytes out of range: {max_bytes} (max {MAX_SCHEMA_BYTES})"
        )));
    }
    if let Some(max_entries) = config.schema_registry_max_entries
        && max_entries == 0
    {
        return Err(SqliteStoreError::Invalid(
            "schema_registry_max_entries must be greater than zero".to_string(),
        ));
    }
    Ok(())
}

/// Validates runtime limits in the store configuration.
fn validate_runtime_limits(config: &SqliteStoreConfig) -> Result<(), SqliteStoreError> {
    if config.writer_queue_capacity == 0 {
        return Err(SqliteStoreError::Invalid(
            "writer_queue_capacity must be greater than zero".to_string(),
        ));
    }
    if config.batch_max_ops == 0 {
        return Err(SqliteStoreError::Invalid(
            "batch_max_ops must be greater than zero".to_string(),
        ));
    }
    if config.batch_max_bytes == 0 {
        return Err(SqliteStoreError::Invalid(
            "batch_max_bytes must be greater than zero".to_string(),
        ));
    }
    if config.batch_max_wait_ms == 0 {
        return Err(SqliteStoreError::Invalid(
            "batch_max_wait_ms must be greater than zero".to_string(),
        ));
    }
    if config.read_pool_size == 0 {
        return Err(SqliteStoreError::Invalid(
            "read_pool_size must be greater than zero".to_string(),
        ));
    }
    Ok(())
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// `SQLite` store errors.
///
/// # Invariants
/// - Error messages avoid embedding raw run state or schema payloads.
#[derive(Debug, Error, Clone)]
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
    /// Store is overloaded and the caller should retry.
    #[error("sqlite store overloaded: {message}")]
    Overloaded {
        /// Retryable overload message.
        message: String,
        /// Optional retry delay in milliseconds.
        retry_after_ms: Option<u64>,
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
            SqliteStoreError::Overloaded {
                message,
                retry_after_ms,
            } => Self::Overloaded {
                message,
                retry_after_ms,
            },
        }
    }
}

// ============================================================================
// SECTION: Store
// ============================================================================

/// `SQLite`-backed run state store with WAL support.
///
/// # Invariants
/// - Run state loads verify stored hashes before deserialization.
/// - `SQLite` connection access is serialized through a mutex.
#[derive(Clone)]
pub struct SqliteRunStateStore {
    /// Store configuration.
    config: SqliteStoreConfig,
    /// Shared writer connection guarded by a mutex.
    write_connection: Arc<Mutex<Connection>>,
    /// Read-only connection pool used for read path isolation under WAL.
    read_connections: Arc<Vec<Mutex<Connection>>>,
    /// Round-robin cursor for read connection selection.
    read_cursor: Arc<AtomicUsize>,
    /// Deterministic writer gateway for queueing durable mutations.
    writer_gateway: Arc<SqliteWriteGateway>,
    /// Lightweight operation stats used for local performance diagnostics.
    perf_stats: Arc<Mutex<SqlitePerfStats>>,
}

/// Summary metadata for a stored run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
    /// Latest stored version.
    pub latest_version: i64,
    /// Timestamp when the latest version was saved.
    pub saved_at: i64,
}

/// Summary metadata for a specific run state version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunVersionSummary {
    /// Stored version number.
    pub version: i64,
    /// Timestamp when the version was saved.
    pub saved_at: i64,
    /// Stored state hash.
    pub state_hash: String,
    /// Stored hash algorithm label.
    pub hash_algorithm: String,
    /// Stored payload length in bytes.
    pub state_bytes: usize,
}

/// Store-level operation counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SqliteStoreOpCounts {
    /// Run-state read operations (`load`).
    pub read: u64,
    /// Run-state write operations (`save`).
    pub write: u64,
    /// Registry register operations.
    pub register: u64,
    /// Registry list operations.
    pub list: u64,
}

/// Classified database error counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SqliteDbErrorCounts {
    /// Count of `busy` database errors.
    pub busy: u64,
    /// Count of `locked` database errors.
    pub locked: u64,
    /// Count of all other database errors.
    pub other: u64,
}

/// Snapshot of lightweight `SQLite` perf/contention stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlitePerfStatsSnapshot {
    /// Per-class operation counts.
    pub op_counts: SqliteStoreOpCounts,
    /// Operation latencies represented as `<= upper_bound` buckets plus overflow slot.
    pub latency_buckets_ms: Vec<u64>,
    /// Read-operation histogram counts (length = `latency_buckets_ms.len() + 1`).
    pub read_latency_histogram: Vec<u64>,
    /// Write-operation histogram counts (length = `latency_buckets_ms.len() + 1`).
    pub write_latency_histogram: Vec<u64>,
    /// Register-operation histogram counts (length = `latency_buckets_ms.len() + 1`).
    pub register_latency_histogram: Vec<u64>,
    /// List-operation histogram counts (length = `latency_buckets_ms.len() + 1`).
    pub list_latency_histogram: Vec<u64>,
    /// Cumulative read duration in milliseconds.
    pub read_total_duration_ms: u64,
    /// Cumulative write duration in milliseconds.
    pub write_total_duration_ms: u64,
    /// Cumulative register duration in milliseconds.
    pub register_total_duration_ms: u64,
    /// Cumulative list duration in milliseconds.
    pub list_total_duration_ms: u64,
    /// Read-pool lock wait bucket boundaries in microseconds.
    pub read_wait_buckets_us: Vec<u64>,
    /// Read-pool lock wait histogram counts.
    pub read_wait_histogram_us: Vec<u64>,
    /// Read-pool lock wait p50 estimate in microseconds.
    pub read_wait_p50_us: u64,
    /// Read-pool lock wait p95 estimate in microseconds.
    pub read_wait_p95_us: u64,
    /// Database error counters.
    pub db_errors: SqliteDbErrorCounts,
    /// Writer queue and batch diagnostics.
    pub writer: SqliteWriterDiagnosticsSnapshot,
}

/// Snapshot of writer queue/batch diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteWriterDiagnosticsSnapshot {
    /// Number of commands accepted by the writer queue.
    pub commands_enqueued: u64,
    /// Number of commands rejected due to queue saturation.
    pub commands_rejected: u64,
    /// Number of commands fully processed by the writer.
    pub commands_processed: u64,
    /// Number of successfully committed batches.
    pub commit_success_count: u64,
    /// Number of failed batch commits.
    pub commit_failure_count: u64,
    /// Queue depth bucket boundaries.
    pub queue_depth_buckets: Vec<u64>,
    /// Queue depth histogram counts (length = `queue_depth_buckets.len() + 1`).
    pub queue_depth_histogram: Vec<u64>,
    /// Queue depth p50 estimate from histogram.
    pub queue_depth_p50: u64,
    /// Queue depth p95 estimate from histogram.
    pub queue_depth_p95: u64,
    /// Batch-size bucket boundaries.
    pub batch_size_buckets: Vec<u64>,
    /// Batch-size histogram counts (length = `batch_size_buckets.len() + 1`).
    pub batch_size_histogram: Vec<u64>,
    /// Batch-size p50 estimate from histogram.
    pub batch_size_p50: u64,
    /// Batch-size p95 estimate from histogram.
    pub batch_size_p95: u64,
    /// Writer timing bucket boundaries in microseconds.
    pub timing_buckets_us: Vec<u64>,
    /// Batch wait histogram counts (length = `timing_buckets_us.len() + 1`).
    pub batch_wait_histogram_us: Vec<u64>,
    /// Batch wait p50 estimate in microseconds.
    pub batch_wait_p50_us: u64,
    /// Batch wait p95 estimate in microseconds.
    pub batch_wait_p95_us: u64,
    /// Batch commit histogram counts (length = `timing_buckets_us.len() + 1`).
    pub batch_commit_histogram_us: Vec<u64>,
    /// Batch commit p50 estimate in microseconds.
    pub batch_commit_p50_us: u64,
    /// Batch commit p95 estimate in microseconds.
    pub batch_commit_p95_us: u64,
}

/// Internal mutable perf counters before snapshot serialization.
#[derive(Debug, Default)]
struct SqlitePerfStats {
    /// Per-operation counters.
    op_counts: SqliteStoreOpCounts,
    /// Read-operation latency histogram.
    read_latency_histogram: [u64; PERF_BUCKETS_MS.len() + 1],
    /// Write-operation latency histogram.
    write_latency_histogram: [u64; PERF_BUCKETS_MS.len() + 1],
    /// Register-operation latency histogram.
    register_latency_histogram: [u64; PERF_BUCKETS_MS.len() + 1],
    /// List-operation latency histogram.
    list_latency_histogram: [u64; PERF_BUCKETS_MS.len() + 1],
    /// Cumulative read duration in milliseconds.
    read_total_duration_ms: u64,
    /// Cumulative write duration in milliseconds.
    write_total_duration_ms: u64,
    /// Cumulative register duration in milliseconds.
    register_total_duration_ms: u64,
    /// Cumulative list duration in milliseconds.
    list_total_duration_ms: u64,
    /// Read-pool lock wait histogram in microseconds.
    read_wait_histogram_us: [u64; READ_WAIT_TIME_BUCKETS_US.len() + 1],
    /// Classified database error counters.
    db_errors: SqliteDbErrorCounts,
    /// Writer queue and batch diagnostics.
    writer: SqliteWriterDiagnostics,
}

/// Performance operation class used for histogram/counter updates.
#[derive(Debug, Clone, Copy)]
enum SqlitePerfOp {
    /// Run-state read (`load`).
    Read,
    /// Run-state write (`save`).
    Write,
    /// Schema registration write.
    Register,
    /// Schema list read.
    List,
}

/// Internal mutable writer diagnostics before snapshot serialization.
#[derive(Debug)]
struct SqliteWriterDiagnostics {
    /// Number of commands accepted by queue submission.
    commands_enqueued: u64,
    /// Number of commands rejected due to queue backpressure or disconnect.
    commands_rejected: u64,
    /// Number of commands processed by writer batches.
    commands_processed: u64,
    /// Number of successful batch commits.
    commit_success_count: u64,
    /// Number of failed batch commits.
    commit_failure_count: u64,
    /// Queue-depth histogram captured at submit-time.
    queue_depth_histogram: [u64; WRITER_QUEUE_DEPTH_BUCKETS.len() + 1],
    /// Batch-size histogram captured at commit-time.
    batch_size_histogram: [u64; WRITER_BATCH_SIZE_BUCKETS.len() + 1],
    /// Batch wait-time histogram in microseconds.
    batch_wait_histogram_us: [u64; WRITER_TIME_BUCKETS_US.len() + 1],
    /// Batch commit-time histogram in microseconds.
    batch_commit_histogram_us: [u64; WRITER_TIME_BUCKETS_US.len() + 1],
}

impl Default for SqliteWriterDiagnostics {
    fn default() -> Self {
        Self {
            commands_enqueued: 0,
            commands_rejected: 0,
            commands_processed: 0,
            commit_success_count: 0,
            commit_failure_count: 0,
            queue_depth_histogram: [0; WRITER_QUEUE_DEPTH_BUCKETS.len() + 1],
            batch_size_histogram: [0; WRITER_BATCH_SIZE_BUCKETS.len() + 1],
            batch_wait_histogram_us: [0; WRITER_TIME_BUCKETS_US.len() + 1],
            batch_commit_histogram_us: [0; WRITER_TIME_BUCKETS_US.len() + 1],
        }
    }
}

/// Gateway for bounded writer-queue submissions.
struct SqliteWriteGateway {
    /// Synchronous channel sender into the writer runtime.
    sender: SyncSender<SqliteWriterCommand>,
    /// Approximate number of commands currently pending.
    pending_depth: Arc<AtomicUsize>,
    /// Monotonic sequence assigned to queued commands.
    sequence: AtomicU64,
    /// Shared perf counters updated by submit-side logic.
    perf_stats: Arc<Mutex<SqlitePerfStats>>,
    /// Suggested retry delay returned on overload responses.
    retry_after_ms: u64,
}

/// Command envelope queued to the writer runtime.
struct SqliteWriterCommand {
    /// Monotonic sequence for deterministic batch ordering.
    sequence: u64,
    /// Submit timestamp used to derive queue wait.
    enqueued_at: Instant,
    /// Approximate payload size used for batch byte limits.
    estimated_bytes: usize,
    /// Command payload and response channel.
    payload: SqliteWriterPayload,
}

/// Queue payload variants handled by the writer runtime.
enum SqliteWriterPayload {
    /// Persist a prepared run-state snapshot.
    Save {
        /// Prepared save request payload.
        request: PreparedSaveState,
        /// Result channel for the save operation.
        response: mpsc::Sender<Result<(), SqliteStoreError>>,
    },
    /// Persist a prepared schema registry record.
    Register {
        /// Prepared register request payload.
        request: PreparedRegisterRecord,
        /// Result channel for the register operation.
        response: mpsc::Sender<Result<(), DataShapeRegistryError>>,
    },
    /// Execute lightweight readiness probe on writer connection.
    Readiness {
        /// Result channel for readiness outcome.
        response: mpsc::Sender<Result<(), SqliteStoreError>>,
    },
}

/// Fully-prepared run-state save payload for writer execution.
#[derive(Debug, Clone)]
struct PreparedSaveState {
    /// Canonical run-state value to persist.
    state: RunState,
    /// Canonical JSON bytes for run-state.
    state_json: Vec<u8>,
    /// Canonical hash of `state_json`.
    state_hash: String,
    /// Hash algorithm used for `state_hash`.
    hash_algorithm: HashAlgorithm,
    /// Save timestamp in unix milliseconds.
    saved_at: i64,
}

/// Fully-prepared schema register payload for writer execution.
#[derive(Debug, Clone)]
struct PreparedRegisterRecord {
    /// Canonical schema record to persist.
    record: DataShapeRecord,
    /// Canonical JSON bytes for schema.
    schema_json: Vec<u8>,
    /// Canonical schema size in bytes.
    schema_size_bytes: i64,
    /// Canonical hash of `schema_json`.
    schema_hash: String,
    /// Hash algorithm used for `schema_hash`.
    hash_algorithm: HashAlgorithm,
    /// JSON-encoded creation timestamp.
    created_at_json: String,
    /// Optional signing key ID.
    signing_key_id: Option<String>,
    /// Optional signing signature bytes as text.
    signing_signature: Option<String>,
    /// Optional signing algorithm label.
    signing_algorithm: Option<String>,
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
        validate_schema_registry_limits(&config)?;
        validate_runtime_limits(&config)?;
        let mut write_connection = open_connection(&config)?;
        initialize_schema(&mut write_connection)?;
        let mut read_connections = Vec::with_capacity(config.read_pool_size);
        for _ in 0 .. config.read_pool_size {
            let mut read_connection = open_connection(&config)?;
            initialize_schema(&mut read_connection)?;
            read_connections.push(Mutex::new(read_connection));
        }
        let perf_stats = Arc::new(Mutex::new(SqlitePerfStats::default()));
        let pending_depth = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = mpsc::sync_channel(config.writer_queue_capacity);
        let retry_after_ms = config.batch_max_wait_ms;
        let write_connection = Arc::new(Mutex::new(write_connection));
        spawn_sqlite_writer_runtime(
            config.clone(),
            Arc::clone(&write_connection),
            Arc::clone(&perf_stats),
            Arc::clone(&pending_depth),
            receiver,
        )?;
        Ok(Self {
            config,
            write_connection,
            read_connections: Arc::new(read_connections),
            read_cursor: Arc::new(AtomicUsize::new(0)),
            writer_gateway: Arc::new(SqliteWriteGateway {
                sender,
                pending_depth,
                sequence: AtomicU64::new(1),
                perf_stats: Arc::clone(&perf_stats),
                retry_after_ms,
            }),
            perf_stats,
        })
    }

    /// Verifies the store can execute a simple SQL statement.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if the mutex is poisoned or the query fails.
    fn check_connection(&self) -> Result<(), SqliteStoreError> {
        let connection = self.read_connection();
        {
            let wait_started = Instant::now();
            let guard = connection
                .lock()
                .map_err(|_| SqliteStoreError::Io("sqlite read mutex poisoned".to_string()))?;
            let wait_us = u64::try_from(wait_started.elapsed().as_micros()).unwrap_or(u64::MAX);
            self.record_read_wait(wait_us);
            guard.execute("SELECT 1", []).map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        }
        self.writer_gateway.submit_readiness()
    }

    /// Returns the configured schema payload size limit for registry operations.
    #[must_use]
    const fn registry_max_schema_bytes(&self) -> usize {
        match self.config.schema_registry_max_schema_bytes {
            Some(limit) => limit,
            None => MAX_SCHEMA_BYTES,
        }
    }

    /// Returns a snapshot of lightweight operation and contention stats.
    #[must_use]
    pub fn perf_stats_snapshot(&self) -> SqlitePerfStatsSnapshot {
        let guard = self.perf_stats.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let writer = &guard.writer;
        SqlitePerfStatsSnapshot {
            op_counts: guard.op_counts.clone(),
            latency_buckets_ms: PERF_BUCKETS_MS.to_vec(),
            read_latency_histogram: guard.read_latency_histogram.to_vec(),
            write_latency_histogram: guard.write_latency_histogram.to_vec(),
            register_latency_histogram: guard.register_latency_histogram.to_vec(),
            list_latency_histogram: guard.list_latency_histogram.to_vec(),
            read_total_duration_ms: guard.read_total_duration_ms,
            write_total_duration_ms: guard.write_total_duration_ms,
            register_total_duration_ms: guard.register_total_duration_ms,
            list_total_duration_ms: guard.list_total_duration_ms,
            read_wait_buckets_us: READ_WAIT_TIME_BUCKETS_US.to_vec(),
            read_wait_histogram_us: guard.read_wait_histogram_us.to_vec(),
            read_wait_p50_us: histogram_percentile(
                &READ_WAIT_TIME_BUCKETS_US,
                &guard.read_wait_histogram_us,
                50,
            ),
            read_wait_p95_us: histogram_percentile(
                &READ_WAIT_TIME_BUCKETS_US,
                &guard.read_wait_histogram_us,
                95,
            ),
            db_errors: guard.db_errors.clone(),
            writer: SqliteWriterDiagnosticsSnapshot {
                commands_enqueued: writer.commands_enqueued,
                commands_rejected: writer.commands_rejected,
                commands_processed: writer.commands_processed,
                commit_success_count: writer.commit_success_count,
                commit_failure_count: writer.commit_failure_count,
                queue_depth_buckets: WRITER_QUEUE_DEPTH_BUCKETS.to_vec(),
                queue_depth_histogram: writer.queue_depth_histogram.to_vec(),
                queue_depth_p50: histogram_percentile(
                    &WRITER_QUEUE_DEPTH_BUCKETS,
                    &writer.queue_depth_histogram,
                    50,
                ),
                queue_depth_p95: histogram_percentile(
                    &WRITER_QUEUE_DEPTH_BUCKETS,
                    &writer.queue_depth_histogram,
                    95,
                ),
                batch_size_buckets: WRITER_BATCH_SIZE_BUCKETS.to_vec(),
                batch_size_histogram: writer.batch_size_histogram.to_vec(),
                batch_size_p50: histogram_percentile(
                    &WRITER_BATCH_SIZE_BUCKETS,
                    &writer.batch_size_histogram,
                    50,
                ),
                batch_size_p95: histogram_percentile(
                    &WRITER_BATCH_SIZE_BUCKETS,
                    &writer.batch_size_histogram,
                    95,
                ),
                timing_buckets_us: WRITER_TIME_BUCKETS_US.to_vec(),
                batch_wait_histogram_us: writer.batch_wait_histogram_us.to_vec(),
                batch_wait_p50_us: histogram_percentile(
                    &WRITER_TIME_BUCKETS_US,
                    &writer.batch_wait_histogram_us,
                    50,
                ),
                batch_wait_p95_us: histogram_percentile(
                    &WRITER_TIME_BUCKETS_US,
                    &writer.batch_wait_histogram_us,
                    95,
                ),
                batch_commit_histogram_us: writer.batch_commit_histogram_us.to_vec(),
                batch_commit_p50_us: histogram_percentile(
                    &WRITER_TIME_BUCKETS_US,
                    &writer.batch_commit_histogram_us,
                    50,
                ),
                batch_commit_p95_us: histogram_percentile(
                    &WRITER_TIME_BUCKETS_US,
                    &writer.batch_commit_histogram_us,
                    95,
                ),
            },
        }
    }

    /// Resets lightweight operation and contention stats to zero.
    pub fn reset_perf_stats(&self) {
        if let Ok(mut guard) = self.perf_stats.lock() {
            *guard = SqlitePerfStats::default();
        }
    }

    /// Records operation timing plus optional DB error classification.
    fn record_store_op(
        &self,
        op: SqlitePerfOp,
        elapsed: std::time::Duration,
        db_error: Option<&str>,
    ) {
        let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let bucket_index = histogram_bucket_index(elapsed_ms);
        let Ok(mut stats) = self.perf_stats.lock() else {
            return;
        };
        match op {
            SqlitePerfOp::Read => {
                stats.op_counts.read = stats.op_counts.read.saturating_add(1);
                stats.read_total_duration_ms =
                    stats.read_total_duration_ms.saturating_add(elapsed_ms);
                if let Some(slot) = stats.read_latency_histogram.get_mut(bucket_index) {
                    *slot = slot.saturating_add(1);
                }
            }
            SqlitePerfOp::Write => {
                stats.op_counts.write = stats.op_counts.write.saturating_add(1);
                stats.write_total_duration_ms =
                    stats.write_total_duration_ms.saturating_add(elapsed_ms);
                if let Some(slot) = stats.write_latency_histogram.get_mut(bucket_index) {
                    *slot = slot.saturating_add(1);
                }
            }
            SqlitePerfOp::Register => {
                stats.op_counts.register = stats.op_counts.register.saturating_add(1);
                stats.register_total_duration_ms =
                    stats.register_total_duration_ms.saturating_add(elapsed_ms);
                if let Some(slot) = stats.register_latency_histogram.get_mut(bucket_index) {
                    *slot = slot.saturating_add(1);
                }
            }
            SqlitePerfOp::List => {
                stats.op_counts.list = stats.op_counts.list.saturating_add(1);
                stats.list_total_duration_ms =
                    stats.list_total_duration_ms.saturating_add(elapsed_ms);
                if let Some(slot) = stats.list_latency_histogram.get_mut(bucket_index) {
                    *slot = slot.saturating_add(1);
                }
            }
        }
        if let Some(message) = db_error {
            match classify_db_error_message(message) {
                SqliteDbErrorKind::Busy => {
                    stats.db_errors.busy = stats.db_errors.busy.saturating_add(1);
                }
                SqliteDbErrorKind::Locked => {
                    stats.db_errors.locked = stats.db_errors.locked.saturating_add(1);
                }
                SqliteDbErrorKind::Other => {
                    stats.db_errors.other = stats.db_errors.other.saturating_add(1);
                }
            }
        }
    }

    /// Records read-pool lock wait in microseconds.
    fn record_read_wait(&self, wait_us: u64) {
        let bucket = histogram_bucket_index_from_bounds(&READ_WAIT_TIME_BUCKETS_US, wait_us);
        let Ok(mut stats) = self.perf_stats.lock() else {
            return;
        };
        if let Some(slot) = stats.read_wait_histogram_us.get_mut(bucket) {
            *slot = slot.saturating_add(1);
        }
    }

    /// Returns the next read connection using round-robin selection.
    fn read_connection(&self) -> &Mutex<Connection> {
        let len = self.read_connections.len();
        let index = self.read_cursor.fetch_add(1, Ordering::Relaxed) % len;
        &self.read_connections[index]
    }

    /// Builds a validated and hashed run-state save payload.
    fn prepare_save_state(state: &RunState) -> Result<PreparedSaveState, SqliteStoreError> {
        let state_json = canonical_json_bytes(state)
            .map_err(|err| SqliteStoreError::Invalid(err.to_string()))?;
        if state_json.len() > MAX_STATE_BYTES {
            return Err(SqliteStoreError::TooLarge {
                max_bytes: MAX_STATE_BYTES,
                actual_bytes: state_json.len(),
            });
        }
        let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &state_json);
        Ok(PreparedSaveState {
            state: state.clone(),
            state_json,
            state_hash: digest.value,
            hash_algorithm: digest.algorithm,
            saved_at: unix_millis(),
        })
    }

    /// Builds a validated and hashed schema registration payload.
    fn prepare_register_record(
        &self,
        record: DataShapeRecord,
    ) -> Result<PreparedRegisterRecord, DataShapeRegistryError> {
        let schema_json = canonical_json_bytes(&record.schema)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        ensure_schema_bytes_within_limit(schema_json.len(), self.registry_max_schema_bytes())?;
        let schema_size_bytes = i64::try_from(schema_json.len()).map_err(|_| {
            DataShapeRegistryError::Invalid("schema size exceeds platform limits".to_string())
        })?;
        let schema_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &schema_json);
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
        Ok(PreparedRegisterRecord {
            record,
            schema_json,
            schema_size_bytes,
            schema_hash: schema_hash.value,
            hash_algorithm: schema_hash.algorithm,
            created_at_json,
            signing_key_id,
            signing_signature,
            signing_algorithm,
        })
    }
}

impl RunStateStore for SqliteRunStateStore {
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        let started = Instant::now();
        let result = self.load_state(*tenant_id, *namespace_id, run_id);
        self.record_store_op(
            SqlitePerfOp::Read,
            started.elapsed(),
            result.as_ref().err().and_then(db_error_message_store),
        );
        result.map_err(StoreError::from)
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        let started = Instant::now();
        let result = Self::prepare_save_state(state)
            .and_then(|request| self.writer_gateway.submit_save(request));
        self.record_store_op(
            SqlitePerfOp::Write,
            started.elapsed(),
            result.as_ref().err().and_then(db_error_message_store),
        );
        result.map_err(StoreError::from)
    }

    fn readiness(&self) -> Result<(), StoreError> {
        self.check_connection().map_err(StoreError::from)
    }
}

impl DataShapeRegistry for SqliteRunStateStore {
    fn register(&self, record: DataShapeRecord) -> Result<(), DataShapeRegistryError> {
        let started = Instant::now();
        let result = self
            .prepare_register_record(record)
            .and_then(|request| self.writer_gateway.submit_register(request));
        self.record_store_op(
            SqlitePerfOp::Register,
            started.elapsed(),
            result.as_ref().err().and_then(db_error_message_registry),
        );
        result
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        schema_id: &DataShapeId,
        version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
        let started = Instant::now();
        let result = (|| -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
            let connection = self.read_connection();
            let wait_started = Instant::now();
            let mut guard = connection.lock().map_err(|_| {
                DataShapeRegistryError::Io("schema registry read mutex poisoned".to_string())
            })?;
            let wait_us = u64::try_from(wait_started.elapsed().as_micros()).unwrap_or(u64::MAX);
            self.record_read_wait(wait_us);
            let row = {
                let tx = guard
                    .transaction()
                    .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
                let row = query_schema_row_by_id(
                    &tx,
                    *tenant_id,
                    *namespace_id,
                    schema_id,
                    version,
                    self.registry_max_schema_bytes(),
                )?;
                tx.commit().map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
                row
            };
            drop(guard);
            let Some(row) = row else {
                return Ok(None);
            };
            let record = build_schema_record(*tenant_id, *namespace_id, row)?;
            Ok(Some(record))
        })();
        self.record_store_op(
            SqlitePerfOp::Read,
            started.elapsed(),
            result.as_ref().err().and_then(db_error_message_registry),
        );
        result
    }

    fn list(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<DataShapePage, DataShapeRegistryError> {
        let started = Instant::now();
        let result = (|| -> Result<DataShapePage, DataShapeRegistryError> {
            if limit == 0 {
                return Err(DataShapeRegistryError::Invalid(
                    "schema list limit must be greater than zero".to_string(),
                ));
            }
            let limit = i64::try_from(limit)
                .map_err(|_| DataShapeRegistryError::Invalid("limit too large".to_string()))?;
            let cursor = cursor.map(|value| parse_registry_cursor(&value)).transpose()?;
            let connection = self.read_connection();
            let wait_started = Instant::now();
            let mut guard = connection.lock().map_err(|_| {
                DataShapeRegistryError::Io("schema registry read mutex poisoned".to_string())
            })?;
            let wait_us = u64::try_from(wait_started.elapsed().as_micros()).unwrap_or(u64::MAX);
            self.record_read_wait(wait_us);
            let records = {
                let tx = guard
                    .transaction()
                    .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
                ensure_registry_schema_sizes(
                    &tx,
                    *tenant_id,
                    *namespace_id,
                    self.registry_max_schema_bytes(),
                )?;
                let rows =
                    query_schema_rows(&tx, *tenant_id, *namespace_id, cursor.as_ref(), limit)?;
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
            Ok(DataShapePage {
                items: records,
                next_token,
            })
        })();
        self.record_store_op(
            SqlitePerfOp::List,
            started.elapsed(),
            result.as_ref().err().and_then(db_error_message_registry),
        );
        result
    }

    fn readiness(&self) -> Result<(), DataShapeRegistryError> {
        self.check_connection().map_err(|err| DataShapeRegistryError::Io(err.to_string()))
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
            fetch_run_state_payload(self.read_connection(), tenant_id, namespace_id, run_id)?;
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

    /// Lists runs stored in the `SQLite` database (optionally filtered).
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if the database query fails or stored IDs
    /// cannot be parsed.
    pub fn list_runs(
        &self,
        tenant_id: Option<TenantId>,
        namespace_id: Option<NamespaceId>,
    ) -> Result<Vec<RunSummary>, SqliteStoreError> {
        let guard = self
            .read_connection()
            .lock()
            .map_err(|_| SqliteStoreError::Db("read mutex poisoned".to_string()))?;
        let mut stmt = guard
            .prepare(
                "SELECT runs.tenant_id, runs.namespace_id, runs.run_id, runs.latest_version, \
                 run_state_versions.saved_at
                 FROM runs
                 JOIN run_state_versions
                   ON runs.tenant_id = run_state_versions.tenant_id
                  AND runs.namespace_id = run_state_versions.namespace_id
                  AND runs.run_id = run_state_versions.run_id
                  AND runs.latest_version = run_state_versions.version",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let tenant: String = row.get(0)?;
                let namespace: String = row.get(1)?;
                let run_id: String = row.get(2)?;
                let latest_version: i64 = row.get(3)?;
                let saved_at: i64 = row.get(4)?;
                Ok((tenant, namespace, run_id, latest_version, saved_at))
            })
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let mut results = Vec::new();
        for row in rows {
            let (tenant_raw, namespace_raw, run_raw, latest_version, saved_at) =
                row.map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let tenant = parse_tenant_id_str(&tenant_raw)?;
            let namespace = parse_namespace_id_str(&namespace_raw)?;
            if let Some(expected) = tenant_id
                && tenant != expected
            {
                continue;
            }
            if let Some(expected) = namespace_id
                && namespace != expected
            {
                continue;
            }
            results.push(RunSummary {
                tenant_id: tenant,
                namespace_id: namespace,
                run_id: RunId::new(run_raw),
                latest_version,
                saved_at,
            });
        }
        drop(stmt);
        drop(guard);
        results.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        Ok(results)
    }

    /// Lists all stored versions for a run.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if the query fails or stored IDs cannot be
    /// parsed.
    pub fn list_run_versions(
        &self,
        tenant_id: TenantId,
        namespace_id: NamespaceId,
        run_id: &RunId,
    ) -> Result<Vec<RunVersionSummary>, SqliteStoreError> {
        let guard = self
            .read_connection()
            .lock()
            .map_err(|_| SqliteStoreError::Db("read mutex poisoned".to_string()))?;
        let mut stmt = guard
            .prepare(
                "SELECT version, saved_at, state_hash, hash_algorithm, length(state_json) FROM \
                 run_state_versions WHERE tenant_id = ?1 AND namespace_id = ?2 AND run_id = ?3 \
                 ORDER BY version DESC",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let rows = stmt
            .query_map(
                params![tenant_id.to_string(), namespace_id.to_string(), run_id.as_str()],
                |row| {
                    let version: i64 = row.get(0)?;
                    let saved_at: i64 = row.get(1)?;
                    let state_hash: String = row.get(2)?;
                    let hash_algorithm: String = row.get(3)?;
                    let length: i64 = row.get(4)?;
                    Ok((version, saved_at, state_hash, hash_algorithm, length))
                },
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let mut results = Vec::new();
        for row in rows {
            let (version, saved_at, state_hash, hash_algorithm, length) =
                row.map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let length = usize::try_from(length).map_err(|_| {
                SqliteStoreError::Invalid(format!(
                    "negative run state length for run {}",
                    run_id.as_str()
                ))
            })?;
            if length > MAX_STATE_BYTES {
                return Err(SqliteStoreError::TooLarge {
                    max_bytes: MAX_STATE_BYTES,
                    actual_bytes: length,
                });
            }
            results.push(RunVersionSummary {
                version,
                saved_at,
                state_hash,
                hash_algorithm,
                state_bytes: length,
            });
        }
        drop(stmt);
        drop(guard);
        Ok(results)
    }

    /// Loads a specific run state version.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if the version is invalid, the payload is
    /// corrupt, or the stored hash does not match the payload.
    pub fn load_version(
        &self,
        tenant_id: TenantId,
        namespace_id: NamespaceId,
        run_id: &RunId,
        version: i64,
    ) -> Result<Option<RunState>, SqliteStoreError> {
        if version < 1 {
            return Err(SqliteStoreError::Invalid("version must be >= 1".to_string()));
        }
        let payload = fetch_run_state_payload_version(
            self.read_connection(),
            tenant_id,
            namespace_id,
            run_id,
            version,
        )?;
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

    /// Prunes older run state versions, keeping the most recent `keep` entries.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if `keep` is less than 1 or if the database
    /// query fails.
    pub fn prune_versions(
        &self,
        tenant_id: TenantId,
        namespace_id: NamespaceId,
        run_id: &RunId,
        keep: u64,
    ) -> Result<u64, SqliteStoreError> {
        if keep == 0 {
            return Err(SqliteStoreError::Invalid("keep must be >= 1".to_string()));
        }
        let delete_count = {
            let mut guard = self
                .write_connection
                .lock()
                .map_err(|_| SqliteStoreError::Db("write mutex poisoned".to_string()))?;
            let tx = guard.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            let versions = {
                let mut stmt = tx
                    .prepare(
                        "SELECT version FROM run_state_versions WHERE tenant_id = ?1 AND \
                         namespace_id = ?2 AND run_id = ?3 ORDER BY version DESC",
                    )
                    .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
                let rows = stmt
                    .query_map(
                        params![tenant_id.to_string(), namespace_id.to_string(), run_id.as_str()],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
                let mut versions = Vec::new();
                for row in rows {
                    versions.push(row.map_err(|err| SqliteStoreError::Db(err.to_string()))?);
                }
                versions
            };
            let keep_usize = usize::try_from(keep).map_err(|_| {
                SqliteStoreError::Invalid(format!("keep value out of range: {keep}"))
            })?;
            let delete = versions.into_iter().skip(keep_usize).collect::<Vec<_>>();
            for version in &delete {
                tx.execute(
                    "DELETE FROM run_state_versions WHERE tenant_id = ?1 AND namespace_id = ?2 \
                     AND run_id = ?3 AND version = ?4",
                    params![
                        tenant_id.to_string(),
                        namespace_id.to_string(),
                        run_id.as_str(),
                        version
                    ],
                )
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            }
            tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            drop(guard);
            u64::try_from(delete.len()).map_err(|_| {
                SqliteStoreError::Invalid(format!(
                    "pruned version count exceeds u64: {}",
                    delete.len()
                ))
            })?
        };
        Ok(delete_count)
    }
}

/// Classification used when attributing `SQLite` DB error strings.
#[derive(Debug, Clone, Copy)]
enum SqliteDbErrorKind {
    /// Error text indicates busy timeout contention.
    Busy,
    /// Error text indicates lock contention.
    Locked,
    /// Any error not matching busy/locked classifiers.
    Other,
}

/// Returns latency histogram bucket index for millisecond duration.
const fn histogram_bucket_index(duration_ms: u64) -> usize {
    let mut index = 0usize;
    while index < PERF_BUCKETS_MS.len() {
        if duration_ms <= PERF_BUCKETS_MS[index] {
            return index;
        }
        index += 1;
    }
    PERF_BUCKETS_MS.len()
}

/// Classifies database error text into coarse contention categories.
fn classify_db_error_message(message: &str) -> SqliteDbErrorKind {
    let lower = message.to_ascii_lowercase();
    if lower.contains("busy") {
        SqliteDbErrorKind::Busy
    } else if lower.contains("locked") {
        SqliteDbErrorKind::Locked
    } else {
        SqliteDbErrorKind::Other
    }
}

/// Returns DB error message when a store error variant maps to DB.
const fn db_error_message_store(error: &SqliteStoreError) -> Option<&str> {
    match error {
        SqliteStoreError::Db(message) => Some(message.as_str()),
        _ => None,
    }
}

/// Returns DB error message when a registry error variant maps to DB.
const fn db_error_message_registry(error: &DataShapeRegistryError) -> Option<&str> {
    match error {
        DataShapeRegistryError::Io(message) => Some(message.as_str()),
        _ => None,
    }
}

/// Maps writer/store overload outcomes into registry overload semantics.
fn sqlite_store_to_registry_error(error: SqliteStoreError) -> DataShapeRegistryError {
    match error {
        SqliteStoreError::Overloaded {
            message,
            retry_after_ms,
        } => DataShapeRegistryError::Overloaded {
            message,
            retry_after_ms,
        },
        other => DataShapeRegistryError::Io(other.to_string()),
    }
}

impl SqliteWriteGateway {
    /// Submits a prepared run-state save command and waits for completion.
    fn submit_save(&self, request: PreparedSaveState) -> Result<(), SqliteStoreError> {
        let (response_tx, response_rx) = mpsc::channel();
        let command = SqliteWriterCommand {
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            enqueued_at: Instant::now(),
            estimated_bytes: request.state_json.len(),
            payload: SqliteWriterPayload::Save {
                request,
                response: response_tx,
            },
        };
        self.submit(command)?;
        response_rx.recv().map_err(|_| {
            SqliteStoreError::Io("sqlite writer response channel closed".to_string())
        })?
    }

    /// Submits a prepared schema register command and waits for completion.
    fn submit_register(
        &self,
        request: PreparedRegisterRecord,
    ) -> Result<(), DataShapeRegistryError> {
        let (response_tx, response_rx) = mpsc::channel();
        let command = SqliteWriterCommand {
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            enqueued_at: Instant::now(),
            estimated_bytes: request.schema_json.len(),
            payload: SqliteWriterPayload::Register {
                request,
                response: response_tx,
            },
        };
        self.submit(command).map_err(sqlite_store_to_registry_error)?;
        response_rx.recv().map_err(|_| {
            DataShapeRegistryError::Io("sqlite writer response channel closed".to_string())
        })?
    }

    /// Submits a writer readiness probe and waits for completion.
    fn submit_readiness(&self) -> Result<(), SqliteStoreError> {
        let (response_tx, response_rx) = mpsc::channel();
        let command = SqliteWriterCommand {
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            enqueued_at: Instant::now(),
            estimated_bytes: 1,
            payload: SqliteWriterPayload::Readiness {
                response: response_tx,
            },
        };
        self.submit(command)?;
        response_rx.recv().map_err(|_| {
            SqliteStoreError::Io("sqlite writer response channel closed".to_string())
        })?
    }

    /// Attempts enqueue into the bounded writer queue.
    fn submit(&self, command: SqliteWriterCommand) -> Result<(), SqliteStoreError> {
        let depth = self.pending_depth.fetch_add(1, Ordering::AcqRel).saturating_add(1);
        self.record_queue_depth(depth);
        match self.sender.try_send(command) {
            Ok(()) => {
                self.record_command_enqueued();
                Ok(())
            }
            Err(TrySendError::Full(_command)) => {
                self.pending_depth.fetch_sub(1, Ordering::AcqRel);
                self.record_command_rejected();
                Err(SqliteStoreError::Overloaded {
                    message: "sqlite writer queue full; retryable".to_string(),
                    retry_after_ms: Some(self.retry_after_ms),
                })
            }
            Err(TrySendError::Disconnected(_command)) => {
                self.pending_depth.fetch_sub(1, Ordering::AcqRel);
                self.record_command_rejected();
                Err(SqliteStoreError::Overloaded {
                    message: "sqlite writer runtime unavailable".to_string(),
                    retry_after_ms: Some(self.retry_after_ms),
                })
            }
        }
    }

    /// Records queue depth histogram at submission time.
    fn record_queue_depth(&self, depth: usize) {
        let depth_u64 = u64::try_from(depth).unwrap_or(u64::MAX);
        let bucket = histogram_bucket_index_from_bounds(&WRITER_QUEUE_DEPTH_BUCKETS, depth_u64);
        let Ok(mut stats) = self.perf_stats.lock() else {
            return;
        };
        if let Some(slot) = stats.writer.queue_depth_histogram.get_mut(bucket) {
            *slot = slot.saturating_add(1);
        }
    }

    /// Increments command-enqueued counter.
    fn record_command_enqueued(&self) {
        let Ok(mut stats) = self.perf_stats.lock() else {
            return;
        };
        stats.writer.commands_enqueued = stats.writer.commands_enqueued.saturating_add(1);
    }

    /// Increments command-rejected counter.
    fn record_command_rejected(&self) {
        let Ok(mut stats) = self.perf_stats.lock() else {
            return;
        };
        stats.writer.commands_rejected = stats.writer.commands_rejected.saturating_add(1);
    }
}

/// In-flight batch command with response channel and deferred result slot.
enum BatchCommand {
    /// Save batch command.
    Save {
        /// Prepared save payload.
        request: PreparedSaveState,
        /// Save response channel.
        response: mpsc::Sender<Result<(), SqliteStoreError>>,
        /// Deferred save result produced inside transaction.
        result: Option<Result<(), SqliteStoreError>>,
    },
    /// Register batch command.
    Register {
        /// Prepared register payload.
        request: PreparedRegisterRecord,
        /// Register response channel.
        response: mpsc::Sender<Result<(), DataShapeRegistryError>>,
        /// Deferred register result produced inside transaction.
        result: Option<Result<(), DataShapeRegistryError>>,
    },
    /// Readiness batch command.
    Readiness {
        /// Readiness response channel.
        response: mpsc::Sender<Result<(), SqliteStoreError>>,
        /// Deferred readiness result produced inside transaction.
        result: Option<Result<(), SqliteStoreError>>,
    },
}

/// Spawns the dedicated writer runtime thread.
fn spawn_sqlite_writer_runtime(
    config: SqliteStoreConfig,
    write_connection: Arc<Mutex<Connection>>,
    perf_stats: Arc<Mutex<SqlitePerfStats>>,
    pending_depth: Arc<AtomicUsize>,
    receiver: mpsc::Receiver<SqliteWriterCommand>,
) -> Result<(), SqliteStoreError> {
    thread::Builder::new()
        .name("dg-sqlite-writer".to_string())
        .spawn(move || {
            sqlite_writer_loop(&config, &write_connection, &perf_stats, &pending_depth, &receiver);
        })
        .map_err(|err| {
            SqliteStoreError::Io(format!("failed to spawn sqlite writer thread: {err}"))
        })?;
    Ok(())
}

/// Drains queued commands into deterministic micro-batches and commits them.
fn sqlite_writer_loop(
    config: &SqliteStoreConfig,
    write_connection: &Arc<Mutex<Connection>>,
    perf_stats: &Arc<Mutex<SqlitePerfStats>>,
    pending_depth: &Arc<AtomicUsize>,
    receiver: &mpsc::Receiver<SqliteWriterCommand>,
) {
    while let Ok(first) = receiver.recv() {
        let mut queued = vec![first];
        let mut queued_bytes = queued[0].estimated_bytes;
        let first_enqueued = queued[0].enqueued_at;
        let batch_deadline =
            first_enqueued + std::time::Duration::from_millis(config.batch_max_wait_ms);

        while queued.len() < config.batch_max_ops && queued_bytes < config.batch_max_bytes {
            let now = Instant::now();
            if now >= batch_deadline {
                break;
            }
            let timeout = batch_deadline.saturating_duration_since(now);
            match receiver.recv_timeout(timeout) {
                Ok(command) => {
                    queued_bytes = queued_bytes.saturating_add(command.estimated_bytes);
                    queued.push(command);
                    if queued_bytes >= config.batch_max_bytes {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }

        queued.sort_by_key(|command| command.sequence);
        let processed = queued.len();
        let batch_wait_us =
            u64::try_from(Instant::now().duration_since(first_enqueued).as_micros())
                .unwrap_or(u64::MAX);
        let commit_started = Instant::now();
        let commit_result = execute_writer_batch(config, write_connection, queued);
        let commit_elapsed_us =
            u64::try_from(commit_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let processed_u64 = u64::try_from(processed).unwrap_or(u64::MAX);
        let batch_size_u64 = u64::try_from(processed).unwrap_or(u64::MAX);
        let committed = commit_result.is_ok();

        if let Ok(mut stats) = perf_stats.lock() {
            stats.writer.commands_processed =
                stats.writer.commands_processed.saturating_add(processed_u64);
            if committed {
                stats.writer.commit_success_count =
                    stats.writer.commit_success_count.saturating_add(1);
            } else {
                stats.writer.commit_failure_count =
                    stats.writer.commit_failure_count.saturating_add(1);
            }
            let size_bucket =
                histogram_bucket_index_from_bounds(&WRITER_BATCH_SIZE_BUCKETS, batch_size_u64);
            if let Some(slot) = stats.writer.batch_size_histogram.get_mut(size_bucket) {
                *slot = slot.saturating_add(1);
            }
            let wait_bucket =
                histogram_bucket_index_from_bounds(&WRITER_TIME_BUCKETS_US, batch_wait_us);
            if let Some(slot) = stats.writer.batch_wait_histogram_us.get_mut(wait_bucket) {
                *slot = slot.saturating_add(1);
            }
            let commit_bucket =
                histogram_bucket_index_from_bounds(&WRITER_TIME_BUCKETS_US, commit_elapsed_us);
            if let Some(slot) = stats.writer.batch_commit_histogram_us.get_mut(commit_bucket) {
                *slot = slot.saturating_add(1);
            }
        }

        pending_depth.fetch_sub(processed, Ordering::AcqRel);
    }
}

/// Executes one deterministic writer batch in a single transaction.
fn execute_writer_batch(
    config: &SqliteStoreConfig,
    write_connection: &Arc<Mutex<Connection>>,
    commands: Vec<SqliteWriterCommand>,
) -> Result<(), SqliteStoreError> {
    let mut batch = Vec::with_capacity(commands.len());
    for command in commands {
        match command.payload {
            SqliteWriterPayload::Save {
                request,
                response,
            } => batch.push(BatchCommand::Save {
                request,
                response,
                result: None,
            }),
            SqliteWriterPayload::Register {
                request,
                response,
            } => {
                batch.push(BatchCommand::Register {
                    request,
                    response,
                    result: None,
                });
            }
            SqliteWriterPayload::Readiness {
                response,
            } => batch.push(BatchCommand::Readiness {
                response,
                result: None,
            }),
        }
    }

    let mut guard = write_connection
        .lock()
        .map_err(|_| SqliteStoreError::Db("sqlite write mutex poisoned".to_string()))?;
    let tx = guard.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;

    let mut fatal_error: Option<SqliteStoreError> = None;
    for command in &mut batch {
        match command {
            BatchCommand::Save {
                request,
                result,
                ..
            } => {
                let command_result = apply_prepared_save_in_tx(&tx, request, config.max_versions);
                if let Err(err) = &command_result
                    && is_fatal_store_error(err)
                {
                    fatal_error = Some(err.clone());
                    break;
                }
                *result = Some(command_result);
            }
            BatchCommand::Register {
                request,
                result,
                ..
            } => {
                let command_result =
                    apply_prepared_register_in_tx(&tx, request, config.schema_registry_max_entries);
                if let Err(err) = &command_result
                    && is_fatal_registry_error(err)
                {
                    fatal_error = Some(SqliteStoreError::Db(err.to_string()));
                    break;
                }
                *result = Some(command_result);
            }
            BatchCommand::Readiness {
                result, ..
            } => {
                let command_result = tx
                    .execute("SELECT 1", [])
                    .map(|_| ())
                    .map_err(|err| SqliteStoreError::Db(err.to_string()));
                if let Err(err) = &command_result
                    && is_fatal_store_error(err)
                {
                    fatal_error = Some(err.clone());
                    break;
                }
                *result = Some(command_result);
            }
        }
    }

    if let Some(error) = fatal_error {
        let _ = tx.rollback();
        send_batch_failure(batch, &error);
        return Err(error);
    }

    if let Err(err) = tx.commit() {
        let error = SqliteStoreError::Db(err.to_string());
        send_batch_failure(batch, &error);
        return Err(error);
    }
    drop(guard);
    send_batch_results(batch);
    Ok(())
}

/// Sends terminal failure to all batch command response channels.
fn send_batch_failure(batch: Vec<BatchCommand>, error: &SqliteStoreError) {
    let message = error.to_string();
    for command in batch {
        match command {
            BatchCommand::Register {
                response, ..
            } => {
                let _ = response.send(Err(DataShapeRegistryError::Io(message.clone())));
            }
            BatchCommand::Save {
                response, ..
            }
            | BatchCommand::Readiness {
                response, ..
            } => {
                let _ = response.send(Err(error.clone()));
            }
        }
    }
}

/// Sends per-command batch outcomes to response channels.
fn send_batch_results(batch: Vec<BatchCommand>) {
    for command in batch {
        match command {
            BatchCommand::Save {
                response,
                result,
                ..
            } => {
                let outcome = result.unwrap_or_else(|| {
                    Err(SqliteStoreError::Db("sqlite writer missing save outcome".to_string()))
                });
                let _ = response.send(outcome);
            }
            BatchCommand::Register {
                response,
                result,
                ..
            } => {
                let outcome = result.unwrap_or_else(|| {
                    Err(DataShapeRegistryError::Io(
                        "sqlite writer missing register outcome".to_string(),
                    ))
                });
                let _ = response.send(outcome);
            }
            BatchCommand::Readiness {
                response,
                result,
            } => {
                let outcome = result.unwrap_or_else(|| {
                    Err(SqliteStoreError::Db("sqlite writer missing readiness outcome".to_string()))
                });
                let _ = response.send(outcome);
            }
        }
    }
}

/// Returns true when store error should abort the entire writer batch.
const fn is_fatal_store_error(error: &SqliteStoreError) -> bool {
    matches!(
        error,
        SqliteStoreError::Io(_)
            | SqliteStoreError::Db(_)
            | SqliteStoreError::Corrupt(_)
            | SqliteStoreError::VersionMismatch(_)
    )
}

/// Returns true when registry error should abort the entire writer batch.
const fn is_fatal_registry_error(error: &DataShapeRegistryError) -> bool {
    matches!(error, DataShapeRegistryError::Io(_))
}

/// Applies a prepared run-state save inside an existing transaction.
fn apply_prepared_save_in_tx(
    tx: &rusqlite::Transaction<'_>,
    request: &PreparedSaveState,
    max_versions: Option<u64>,
) -> Result<(), SqliteStoreError> {
    let latest_version: Option<i64> = {
        let mut stmt = tx
            .prepare_cached(
                "SELECT latest_version FROM runs WHERE tenant_id = ?1 AND namespace_id = ?2 AND \
                 run_id = ?3",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        stmt.query_row(
            params![
                request.state.tenant_id.to_string(),
                request.state.namespace_id.to_string(),
                request.state.run_id.as_str()
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?
    };
    let next_version = match latest_version {
        None => 1,
        Some(value) => {
            if value < 1 {
                return Err(SqliteStoreError::Corrupt(format!(
                    "invalid latest_version for run {}",
                    request.state.run_id.as_str()
                )));
            }
            value.checked_add(1).ok_or_else(|| {
                SqliteStoreError::Corrupt(format!(
                    "run state version overflow for run {}",
                    request.state.run_id.as_str()
                ))
            })?
        }
    };
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO runs (tenant_id, namespace_id, run_id, latest_version) VALUES (?1, \
                 ?2, ?3, ?4) ON CONFLICT(tenant_id, namespace_id, run_id) DO UPDATE SET \
                 latest_version = excluded.latest_version",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        stmt.execute(params![
            request.state.tenant_id.to_string(),
            request.state.namespace_id.to_string(),
            request.state.run_id.as_str(),
            next_version
        ])
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    }
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO run_state_versions (tenant_id, namespace_id, run_id, version, \
                 state_json, state_hash, hash_algorithm, saved_at) VALUES (?1, ?2, ?3, ?4, ?5, \
                 ?6, ?7, ?8)",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        stmt.execute(params![
            request.state.tenant_id.to_string(),
            request.state.namespace_id.to_string(),
            request.state.run_id.as_str(),
            next_version,
            request.state_json.as_slice(),
            request.state_hash.as_str(),
            hash_algorithm_label(request.hash_algorithm),
            request.saved_at
        ])
        .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
    }
    enforce_retention(
        tx,
        request.state.tenant_id,
        request.state.namespace_id,
        request.state.run_id.as_str(),
        next_version,
        max_versions,
    )
}

/// Applies a prepared schema register inside an existing transaction.
fn apply_prepared_register_in_tx(
    tx: &rusqlite::Transaction<'_>,
    request: &PreparedRegisterRecord,
    max_entries: Option<usize>,
) -> Result<(), DataShapeRegistryError> {
    if let Some(max_entries) = max_entries {
        ensure_registry_entry_limit(
            tx,
            request.record.tenant_id,
            request.record.namespace_id,
            max_entries,
        )?;
    }
    let result = {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO data_shapes (
                    tenant_id, namespace_id, schema_id, version,
                    schema_json, schema_size_bytes, schema_hash, hash_algorithm, description,
                    signing_key_id, signing_signature, signing_algorithm,
                    created_at_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
        stmt.execute(params![
            request.record.tenant_id.to_string(),
            request.record.namespace_id.to_string(),
            request.record.schema_id.as_str(),
            request.record.version.as_str(),
            request.schema_json.as_slice(),
            request.schema_size_bytes,
            request.schema_hash.as_str(),
            hash_algorithm_label(request.hash_algorithm),
            request.record.description.as_deref(),
            request.signing_key_id.as_deref(),
            request.signing_signature.as_deref(),
            request.signing_algorithm.as_deref(),
            request.created_at_json.as_str(),
        ])
    };
    match result {
        Ok(_) => {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO registry_namespace_counters (tenant_id, namespace_id, \
                     entry_count)
                     VALUES (?1, ?2, 1)
                     ON CONFLICT(tenant_id, namespace_id)
                     DO UPDATE SET entry_count = entry_count + 1",
                )
                .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            stmt.execute(params![
                request.record.tenant_id.to_string(),
                request.record.namespace_id.to_string(),
            ])
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
            Ok(())
        }
        Err(rusqlite::Error::SqliteFailure(err, _))
            if err.code == ErrorCode::ConstraintViolation =>
        {
            Err(DataShapeRegistryError::Conflict("schema already registered".to_string()))
        }
        Err(err) => Err(DataShapeRegistryError::Io(err.to_string())),
    }
}

/// Returns bucket index for `value` against sorted histogram bounds.
fn histogram_bucket_index_from_bounds(bounds: &[u64], value: u64) -> usize {
    for (idx, upper_bound) in bounds.iter().enumerate() {
        if value <= *upper_bound {
            return idx;
        }
    }
    bounds.len()
}

/// Computes approximate percentile value from bucketed histogram counts.
fn histogram_percentile(bounds: &[u64], counts: &[u64], percentile: u32) -> u64 {
    if percentile == 0 || percentile > 100 || counts.is_empty() || bounds.is_empty() {
        return 0;
    }
    let total = counts.iter().fold(0_u64, |acc, value| acc.saturating_add(*value));
    if total == 0 {
        return 0;
    }
    let rank =
        total.saturating_mul(u64::from(percentile)).saturating_add(99).saturating_div(100).max(1);
    let mut running = 0_u64;
    for (idx, count) in counts.iter().enumerate() {
        running = running.saturating_add(*count);
        if running >= rank {
            return if idx < bounds.len() {
                bounds[idx]
            } else {
                bounds.last().copied().unwrap_or(0)
            };
        }
    }
    bounds.last().copied().unwrap_or(0)
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
    if path.as_os_str().is_empty() {
        return Err(SqliteStoreError::Invalid("store path must not be empty".to_string()));
    }
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
                    schema_size_bytes INTEGER NOT NULL,
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
                    ON data_shapes (tenant_id, namespace_id, schema_id, version);
                CREATE INDEX IF NOT EXISTS idx_data_shapes_size
                    ON data_shapes (tenant_id, namespace_id, schema_size_bytes);
                CREATE TABLE IF NOT EXISTS registry_namespace_counters (
                    tenant_id TEXT NOT NULL,
                    namespace_id TEXT NOT NULL,
                    entry_count INTEGER NOT NULL,
                    PRIMARY KEY (tenant_id, namespace_id)
                );",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        }
        Some(3) => {
            tx.execute_batch(
                "ALTER TABLE data_shapes ADD COLUMN signing_key_id TEXT;
                 ALTER TABLE data_shapes ADD COLUMN signing_signature TEXT;
                 ALTER TABLE data_shapes ADD COLUMN signing_algorithm TEXT;
                 ALTER TABLE data_shapes ADD COLUMN schema_size_bytes INTEGER NOT NULL DEFAULT 0;
                 UPDATE data_shapes SET schema_size_bytes = length(schema_json);
                 CREATE INDEX IF NOT EXISTS idx_data_shapes_size
                     ON data_shapes (tenant_id, namespace_id, schema_size_bytes);
                 CREATE TABLE IF NOT EXISTS registry_namespace_counters (
                     tenant_id TEXT NOT NULL,
                     namespace_id TEXT NOT NULL,
                     entry_count INTEGER NOT NULL,
                     PRIMARY KEY (tenant_id, namespace_id)
                 );
                 INSERT OR REPLACE INTO registry_namespace_counters (tenant_id, namespace_id, \
                 entry_count)
                 SELECT tenant_id, namespace_id, COUNT(1)
                 FROM data_shapes
                 GROUP BY tenant_id, namespace_id;",
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
            tx.execute("UPDATE store_meta SET version = ?1", params![SCHEMA_VERSION])
                .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        }
        Some(4) => {
            tx.execute_batch(
                "ALTER TABLE data_shapes ADD COLUMN schema_size_bytes INTEGER NOT NULL DEFAULT 0;
                 UPDATE data_shapes SET schema_size_bytes = length(schema_json);
                 CREATE INDEX IF NOT EXISTS idx_data_shapes_size
                     ON data_shapes (tenant_id, namespace_id, schema_size_bytes);
                 CREATE TABLE IF NOT EXISTS registry_namespace_counters (
                     tenant_id TEXT NOT NULL,
                     namespace_id TEXT NOT NULL,
                     entry_count INTEGER NOT NULL,
                     PRIMARY KEY (tenant_id, namespace_id)
                 );
                 INSERT OR REPLACE INTO registry_namespace_counters (tenant_id, namespace_id, \
                 entry_count)
                 SELECT tenant_id, namespace_id, COUNT(1)
                 FROM data_shapes
                 GROUP BY tenant_id, namespace_id;",
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
            Some(RunStatePayload {
                bytes,
                hash_value: hash,
                hash_algorithm: algorithm,
            })
        } else {
            None
        };
        tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        payload
    };
    drop(guard);
    Ok(payload)
}

/// Fetches a specific run state payload for the provided run identifiers.
fn fetch_run_state_payload_version(
    connection: &Mutex<Connection>,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    run_id: &RunId,
    version: i64,
) -> Result<Option<RunStatePayload>, SqliteStoreError> {
    let payload = {
        let mut guard =
            connection.lock().map_err(|_| SqliteStoreError::Db("mutex poisoned".to_string()))?;
        let tx = guard.transaction().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let metadata = tx
            .query_row(
                "SELECT length(state_json), state_hash, hash_algorithm FROM run_state_versions \
                 WHERE tenant_id = ?1 AND namespace_id = ?2 AND run_id = ?3 AND version = ?4",
                params![tenant_id.to_string(), namespace_id.to_string(), run_id.as_str(), version],
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
            return Ok(None);
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
                "SELECT state_json FROM run_state_versions WHERE tenant_id = ?1 AND namespace_id \
                 = ?2 AND run_id = ?3 AND version = ?4",
                params![tenant_id.to_string(), namespace_id.to_string(), run_id.as_str(), version],
                |row| row.get(0),
            )
            .map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        let payload = RunStatePayload {
            bytes,
            hash_value: hash,
            hash_algorithm: algorithm,
        };
        tx.commit().map_err(|err| SqliteStoreError::Db(err.to_string()))?;
        drop(guard);
        Ok(Some(payload))
    }?;
    Ok(payload)
}

/// Parses a tenant ID string stored in the database.
///
/// # Errors
///
/// Returns [`SqliteStoreError`] if the value is not a nonzero unsigned integer.
fn parse_tenant_id_str(value: &str) -> Result<TenantId, SqliteStoreError> {
    let raw: u64 = value
        .parse()
        .map_err(|_| SqliteStoreError::Invalid(format!("invalid tenant_id value: {value}")))?;
    TenantId::from_raw(raw)
        .ok_or_else(|| SqliteStoreError::Invalid(format!("tenant_id must be nonzero: {value}")))
}

/// Parses a namespace ID string stored in the database.
///
/// # Errors
///
/// Returns [`SqliteStoreError`] if the value is not a nonzero unsigned integer.
fn parse_namespace_id_str(value: &str) -> Result<NamespaceId, SqliteStoreError> {
    let raw: u64 = value
        .parse()
        .map_err(|_| SqliteStoreError::Invalid(format!("invalid namespace_id value: {value}")))?;
    NamespaceId::from_raw(raw)
        .ok_or_else(|| SqliteStoreError::Invalid(format!("namespace_id must be nonzero: {value}")))
}

/// Parses a pagination cursor payload.
fn parse_registry_cursor(cursor: &str) -> Result<RegistryCursor, DataShapeRegistryError> {
    serde_json::from_str(cursor)
        .map_err(|_| DataShapeRegistryError::Invalid("invalid cursor".to_string()))
}

/// Ensures schema payload sizes remain within configured limits.
fn ensure_schema_bytes_within_limit(
    actual_bytes: usize,
    max_schema_bytes: usize,
) -> Result<(), DataShapeRegistryError> {
    if actual_bytes > max_schema_bytes {
        return Err(DataShapeRegistryError::Invalid(format!(
            "schema exceeds size limit: {actual_bytes} bytes (max {max_schema_bytes})"
        )));
    }
    Ok(())
}

/// Parses a schema length returned from `SQLite` into a safe usize.
fn schema_length_to_usize(length: i64) -> Result<usize, DataShapeRegistryError> {
    usize::try_from(length)
        .map_err(|_| DataShapeRegistryError::Invalid("schema length is invalid".to_string()))
}

/// Checks for oversized schema payloads in the registry.
fn ensure_registry_schema_sizes(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    max_schema_bytes: usize,
) -> Result<(), DataShapeRegistryError> {
    let max_schema_bytes_i64 = i64::try_from(max_schema_bytes).map_err(|_| {
        DataShapeRegistryError::Invalid("schema size limit exceeds platform limits".to_string())
    })?;
    let oversized: Option<i64> = tx
        .query_row(
            "SELECT schema_size_bytes FROM data_shapes WHERE tenant_id = ?1 AND namespace_id = ?2 \
             AND schema_size_bytes > ?3 LIMIT 1",
            params![tenant_id.to_string(), namespace_id.to_string(), max_schema_bytes_i64],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
    if let Some(length) = oversized {
        let length_usize = schema_length_to_usize(length)?;
        ensure_schema_bytes_within_limit(length_usize, max_schema_bytes)?;
    }
    Ok(())
}

/// Ensures registry entry counts remain within configured limits.
fn ensure_registry_entry_limit(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    max_entries: usize,
) -> Result<(), DataShapeRegistryError> {
    let max_entries_i64 = i64::try_from(max_entries).map_err(|_| {
        DataShapeRegistryError::Invalid("schema entry limit exceeds platform limits".to_string())
    })?;
    let count = tx
        .query_row(
            "SELECT entry_count FROM registry_namespace_counters WHERE tenant_id = ?1 AND \
             namespace_id = ?2",
            params![tenant_id.to_string(), namespace_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
    let count = count.unwrap_or(0);
    if count >= max_entries_i64 {
        return Err(DataShapeRegistryError::Invalid(format!(
            "schema registry entry limit exceeded: {count} entries (max {max_entries})"
        )));
    }
    Ok(())
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
            Some(DataShapeSignature {
                key_id,
                signature,
                algorithm,
            })
        }
        _ => None,
    }
}

/// Queries a schema row for the provided tenant, namespace, and identifier.
fn query_schema_row_by_id(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: TenantId,
    namespace_id: NamespaceId,
    schema_id: &DataShapeId,
    version: &DataShapeVersion,
    max_schema_bytes: usize,
) -> Result<Option<SchemaRow>, DataShapeRegistryError> {
    let length: Option<i64> = tx
        .query_row(
            "SELECT schema_size_bytes FROM data_shapes WHERE tenant_id = ?1 AND namespace_id = ?2 \
             AND schema_id = ?3 AND version = ?4",
            params![
                tenant_id.to_string(),
                namespace_id.to_string(),
                schema_id.as_str(),
                version.as_str()
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| map_registry_error(&err))?;
    let Some(length) = length else {
        return Ok(None);
    };
    let length_usize = schema_length_to_usize(length)?;
    ensure_schema_bytes_within_limit(length_usize, max_schema_bytes)?;
    tx.query_row(
        "SELECT schema_id, version, schema_json, schema_hash, hash_algorithm, description, \
         signing_key_id, signing_signature, signing_algorithm, created_at_json FROM data_shapes \
         WHERE tenant_id = ?1 AND namespace_id = ?2 AND schema_id = ?3 AND version = ?4",
        params![
            tenant_id.to_string(),
            namespace_id.to_string(),
            schema_id.as_str(),
            version.as_str()
        ],
        map_schema_row,
    )
    .optional()
    .map_err(|err| map_registry_error(&err))
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

#[cfg(test)]
mod tests {
    use super::DataShapeRegistryError;
    use super::SqliteStoreError;
    use super::sqlite_store_to_registry_error;

    #[test]
    fn sqlite_registry_overloaded_path_preserves_retry_after() {
        let mapped = sqlite_store_to_registry_error(SqliteStoreError::Overloaded {
            message: "registry queue full".to_string(),
            retry_after_ms: Some(31),
        });
        assert!(matches!(
            mapped,
            DataShapeRegistryError::Overloaded {
                message,
                retry_after_ms: Some(31)
            } if message == "registry queue full"
        ));
    }
}
