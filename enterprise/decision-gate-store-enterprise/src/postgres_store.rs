// enterprise/decision-gate-store-enterprise/src/postgres_store.rs
// ============================================================================
// Module: Postgres Store
// Description: Postgres-backed run state and schema registry storage.
// Purpose: Provide durable multi-tenant storage for managed deployments.
// ============================================================================

use std::sync::Arc;
use std::time::Duration;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapePage;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeRegistryError;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::StoreError;
use decision_gate_core::TenantId;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::canonical_json_bytes_with_limit;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use postgres::NoTls;
use postgres::error::SqlState;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

/// Maximum schema payload size accepted by the registry.
const MAX_SCHEMA_BYTES: usize = 1024 * 1024;
/// Maximum run state snapshot size accepted by the store.
const MAX_STATE_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Postgres store configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostgresStoreConfig {
    /// Postgres connection string.
    pub connection: String,
    /// Maximum pool size.
    pub max_connections: u32,
    /// Connect timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Statement timeout in milliseconds.
    pub statement_timeout_ms: u64,
}

impl Default for PostgresStoreConfig {
    fn default() -> Self {
        Self {
            connection: "postgres://decision_gate:decision_gate@localhost/decision_gate"
                .to_string(),
            max_connections: 16,
            connect_timeout_ms: 5_000,
            statement_timeout_ms: 30_000,
        }
    }
}

/// Postgres store errors.
#[derive(Debug, Error)]
pub enum PostgresStoreError {
    /// Postgres error.
    #[error("postgres store error: {0}")]
    Postgres(String),
    /// Invalid data error.
    #[error("postgres store invalid data: {0}")]
    Invalid(String),
}

/// Postgres-backed store implementing run state and schema registry.
pub struct PostgresStore {
    /// Connection pool for Postgres access.
    pool: Option<Pool<PostgresConnectionManager<NoTls>>>,
}

impl Drop for PostgresStore {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            let _ = std::thread::spawn(move || drop(pool));
        }
    }
}

impl PostgresStore {
    /// Creates a new Postgres store and runs migrations.
    ///
    /// # Errors
    ///
    /// Returns [`PostgresStoreError`] when initialization fails.
    pub fn new(config: &PostgresStoreConfig) -> Result<Self, PostgresStoreError> {
        let mut pg_config = config
            .connection
            .parse::<postgres::Config>()
            .map_err(|err| PostgresStoreError::Postgres(err.to_string()))?;
        pg_config.connect_timeout(Duration::from_millis(config.connect_timeout_ms));
        let options = format!("-c statement_timeout={}", config.statement_timeout_ms);
        pg_config.options(&options);
        let manager = PostgresConnectionManager::new(pg_config, NoTls);
        let pool = Pool::builder()
            .max_size(config.max_connections)
            .build(manager)
            .map_err(|err| PostgresStoreError::Postgres(err.to_string()))?;
        let store = Self {
            pool: Some(pool),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Ensures schema and indices exist for Postgres storage.
    fn migrate(&self) -> Result<(), PostgresStoreError> {
        let mut conn = self
            .pool
            .as_ref()
            .ok_or_else(|| PostgresStoreError::Postgres("postgres store closed".to_string()))?
            .get()
            .map_err(|err| PostgresStoreError::Postgres(err.to_string()))?;
        conn.batch_execute(
            "CREATE TABLE IF NOT EXISTS runs (tenant_id TEXT NOT NULL,namespace_id TEXT NOT \
             NULL,run_id TEXT NOT NULL,latest_version BIGINT NOT NULL,PRIMARY KEY (tenant_id, \
             namespace_id, run_id));CREATE TABLE IF NOT EXISTS run_state_versions (tenant_id TEXT \
             NOT NULL,namespace_id TEXT NOT NULL,run_id TEXT NOT NULL,version BIGINT NOT \
             NULL,state_json TEXT NOT NULL,state_hash TEXT NOT NULL,hash_algorithm TEXT NOT \
             NULL,saved_at BIGINT NOT NULL,PRIMARY KEY (tenant_id, namespace_id, run_id, \
             version));CREATE INDEX IF NOT EXISTS idx_run_state_versions_lookup ON \
             run_state_versions (tenant_id, namespace_id, run_id, version);CREATE TABLE IF NOT \
             EXISTS data_shapes (tenant_id TEXT NOT NULL,namespace_id TEXT NOT NULL,schema_id \
             TEXT NOT NULL,version TEXT NOT NULL,schema_json TEXT NOT NULL,schema_hash TEXT NOT \
             NULL,hash_algorithm TEXT NOT NULL,description TEXT,created_at_json TEXT NOT \
             NULL,signing_key_id TEXT,signing_signature TEXT,signing_algorithm TEXT,PRIMARY KEY \
             (tenant_id, namespace_id, schema_id, version));CREATE INDEX IF NOT EXISTS \
             idx_data_shapes_lookup ON data_shapes (tenant_id, namespace_id, schema_id, \
             version);CREATE INDEX IF NOT EXISTS idx_data_shapes_list ON data_shapes (tenant_id, \
             namespace_id, schema_id, version);",
        )
        .map_err(|err| PostgresStoreError::Postgres(err.to_string()))?;
        Ok(())
    }

    /// Returns the current time in milliseconds since epoch.
    fn now_ms() -> i64 {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        i64::try_from(ms).unwrap_or(i64::MAX)
    }

    /// Encodes a registry cursor from schema id and version.
    pub(crate) fn encode_cursor(schema_id: &str, version: &str) -> String {
        serde_json::to_string(&RegistryCursor {
            schema_id: schema_id.to_string(),
            version: version.to_string(),
        })
        .unwrap_or_default()
    }

    /// Decodes a registry cursor string.
    pub(crate) fn decode_cursor(cursor: &str) -> Result<RegistryCursor, DataShapeRegistryError> {
        serde_json::from_str(cursor)
            .map_err(|_| DataShapeRegistryError::Invalid("invalid registry cursor".to_string()))
    }
}

impl RunStateStore for PostgresStore {
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        let mut conn = self
            .pool
            .as_ref()
            .ok_or_else(|| StoreError::Io("postgres store closed".to_string()))?
            .get()
            .map_err(|err| StoreError::Io(err.to_string()))?;
        let latest: Option<i64> = conn
            .query_opt(
                "SELECT latest_version FROM runs WHERE tenant_id = $1 AND namespace_id = $2 AND \
                 run_id = $3",
                &[&tenant_id.as_str(), &namespace_id.as_str(), &run_id.as_str()],
            )
            .map_err(|err| StoreError::Io(err.to_string()))?
            .map(|row| row.get(0));
        let Some(version) = latest else {
            return Ok(None);
        };
        let row = conn
            .query_opt(
                "SELECT state_json, state_hash, hash_algorithm FROM run_state_versions WHERE \
                 tenant_id = $1 AND namespace_id = $2 AND run_id = $3 AND version = $4",
                &[&tenant_id.as_str(), &namespace_id.as_str(), &run_id.as_str(), &version],
            )
            .map_err(|err| StoreError::Io(err.to_string()))?
            .ok_or_else(|| StoreError::Corrupt("missing run state payload".to_string()))?;
        let state_json: String = row.get(0);
        let hash_value: String = row.get(1);
        let hash_algorithm: String = row.get(2);
        let algorithm = match hash_algorithm.as_str() {
            "sha256" => decision_gate_core::hashing::HashAlgorithm::Sha256,
            _ => return Err(StoreError::Corrupt("unknown hash algorithm".to_string())),
        };
        let expected = hash_bytes(algorithm, state_json.as_bytes());
        if expected.value != hash_value {
            return Err(StoreError::Corrupt("hash mismatch for run state".to_string()));
        }
        let state: RunState = serde_json::from_str(&state_json)
            .map_err(|err| StoreError::Invalid(err.to_string()))?;
        if state.run_id.as_str() != run_id.as_str() {
            return Err(StoreError::Invalid("run_id mismatch".to_string()));
        }
        if state.tenant_id.as_str() != tenant_id.as_str()
            || state.namespace_id.as_str() != namespace_id.as_str()
        {
            return Err(StoreError::Invalid("tenant/namespace mismatch".to_string()));
        }
        Ok(Some(state))
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        let canonical_json = canonical_json_bytes_with_limit(state, MAX_STATE_BYTES)
            .map_err(|err| StoreError::Invalid(err.to_string()))?;
        let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &canonical_json);
        let state_json = String::from_utf8(canonical_json)
            .map_err(|err| StoreError::Invalid(err.to_string()))?;
        let mut conn = self
            .pool
            .as_ref()
            .ok_or_else(|| StoreError::Io("postgres store closed".to_string()))?
            .get()
            .map_err(|err| StoreError::Io(err.to_string()))?;
        let mut tx = conn.transaction().map_err(|err| StoreError::Io(err.to_string()))?;
        let row = tx
            .query_one(
                "INSERT INTO runs (tenant_id, namespace_id, run_id, latest_version) VALUES ($1, \
                 $2, $3, 1) ON CONFLICT (tenant_id, namespace_id, run_id) DO UPDATE SET \
                 latest_version = runs.latest_version + 1 RETURNING latest_version",
                &[&state.tenant_id.as_str(), &state.namespace_id.as_str(), &state.run_id.as_str()],
            )
            .map_err(|err| StoreError::Io(err.to_string()))?;
        let next_version: i64 = row.get(0);
        tx.execute(
            "INSERT INTO run_state_versions (tenant_id, namespace_id, run_id, version, \
             state_json, state_hash, hash_algorithm, saved_at) VALUES ($1, $2, $3, $4, $5, $6, \
             $7, $8)",
            &[
                &state.tenant_id.as_str(),
                &state.namespace_id.as_str(),
                &state.run_id.as_str(),
                &next_version,
                &state_json,
                &digest.value,
                &hash_algorithm_label(digest.algorithm),
                &Self::now_ms(),
            ],
        )
        .map_err(|err| StoreError::Io(err.to_string()))?;
        tx.commit().map_err(|err| StoreError::Io(err.to_string()))?;
        Ok(())
    }
}

impl DataShapeRegistry for PostgresStore {
    fn register(&self, record: DataShapeRecord) -> Result<(), DataShapeRegistryError> {
        let schema_bytes = canonical_json_bytes_with_limit(&record.schema, MAX_SCHEMA_BYTES)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let digest = hash_bytes(DEFAULT_HASH_ALGORITHM, &schema_bytes);
        let schema_json = String::from_utf8(schema_bytes)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let signing = record.signing.clone();
        let mut conn = self
            .pool
            .as_ref()
            .ok_or_else(|| DataShapeRegistryError::Io("postgres store closed".to_string()))?
            .get()
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
        let result = conn.execute(
            "INSERT INTO data_shapes (tenant_id, namespace_id, schema_id, version, schema_json, \
             schema_hash, hash_algorithm, description, created_at_json, signing_key_id, \
             signing_signature, signing_algorithm) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
             $10, $11, $12)",
            &[
                &record.tenant_id.as_str(),
                &record.namespace_id.as_str(),
                &record.schema_id.as_str(),
                &record.version.as_str(),
                &schema_json,
                &digest.value,
                &hash_algorithm_label(digest.algorithm),
                &record.description,
                &serde_json::to_string(&record.created_at).unwrap_or_default(),
                &signing.as_ref().map(|s| s.key_id.as_str()),
                &signing.as_ref().map(|s| s.signature.as_str()),
                &signing.as_ref().and_then(|s| s.algorithm.as_deref()),
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.code() == Some(&SqlState::UNIQUE_VIOLATION) {
                    Err(DataShapeRegistryError::Conflict("schema already exists".to_string()))
                } else {
                    Err(DataShapeRegistryError::Io(err.to_string()))
                }
            }
        }
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        schema_id: &DataShapeId,
        version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
        let mut conn = self
            .pool
            .as_ref()
            .ok_or_else(|| DataShapeRegistryError::Io("postgres store closed".to_string()))?
            .get()
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
        let row = conn
            .query_opt(
                "SELECT schema_json, schema_hash, hash_algorithm, description, created_at_json, \
                 signing_key_id, signing_signature, signing_algorithm FROM data_shapes WHERE \
                 tenant_id = $1 AND namespace_id = $2 AND schema_id = $3 AND version = $4",
                &[
                    &tenant_id.as_str(),
                    &namespace_id.as_str(),
                    &schema_id.as_str(),
                    &version.as_str(),
                ],
            )
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let schema_json: String = row.get(0);
        let hash_value: String = row.get(1);
        let hash_algorithm: String = row.get(2);
        let description: Option<String> = row.get(3);
        let created_at_json: String = row.get(4);
        let signing_key_id: Option<String> = row.get(5);
        let signing_signature: Option<String> = row.get(6);
        let signing_algorithm: Option<String> = row.get(7);
        let algorithm = match hash_algorithm.as_str() {
            "sha256" => decision_gate_core::hashing::HashAlgorithm::Sha256,
            _ => return Err(DataShapeRegistryError::Invalid("unknown hash algorithm".to_string())),
        };
        let expected = hash_bytes(algorithm, schema_json.as_bytes());
        if expected.value != hash_value {
            return Err(DataShapeRegistryError::Invalid("schema hash mismatch".to_string()));
        }
        let schema: serde_json::Value = serde_json::from_str(&schema_json)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let created_at = serde_json::from_str(&created_at_json)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        let signing = match (signing_key_id, signing_signature) {
            (Some(key_id), Some(signature)) => Some(decision_gate_core::DataShapeSignature {
                key_id,
                signature,
                algorithm: signing_algorithm,
            }),
            _ => None,
        };
        Ok(Some(DataShapeRecord {
            tenant_id: tenant_id.clone(),
            namespace_id: namespace_id.clone(),
            schema_id: schema_id.clone(),
            version: version.clone(),
            schema,
            description,
            created_at,
            signing,
        }))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Registry pagination and validation are kept together for auditability."
    )]
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
        let limit_usize = usize::try_from(limit)
            .map_err(|_| DataShapeRegistryError::Invalid("limit too large".to_string()))?;
        let fetch_limit = limit.saturating_add(1);
        let (cursor_schema, cursor_version) = match cursor {
            None => (None, None),
            Some(value) => {
                let parsed = Self::decode_cursor(&value)?;
                (Some(parsed.schema_id), Some(parsed.version))
            }
        };
        let mut conn = self
            .pool
            .as_ref()
            .ok_or_else(|| DataShapeRegistryError::Io("postgres store closed".to_string()))?
            .get()
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?;
        let rows = if let (Some(schema_id), Some(version)) = (cursor_schema, cursor_version) {
            conn.query(
                "SELECT schema_id, version, schema_json, schema_hash, hash_algorithm, \
                 description, created_at_json, signing_key_id, signing_signature, \
                 signing_algorithm FROM data_shapes WHERE tenant_id = $1 AND namespace_id = $2 \
                 AND (schema_id, version) > ($3, $4) ORDER BY schema_id, version LIMIT $5",
                &[
                    &tenant_id.as_str(),
                    &namespace_id.as_str(),
                    &schema_id.as_str(),
                    &version.as_str(),
                    &fetch_limit,
                ],
            )
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?
        } else {
            conn.query(
                "SELECT schema_id, version, schema_json, schema_hash, hash_algorithm, \
                 description, created_at_json, signing_key_id, signing_signature, \
                 signing_algorithm FROM data_shapes WHERE tenant_id = $1 AND namespace_id = $2 \
                 ORDER BY schema_id, version LIMIT $3",
                &[&tenant_id.as_str(), &namespace_id.as_str(), &fetch_limit],
            )
            .map_err(|err| DataShapeRegistryError::Io(err.to_string()))?
        };
        let mut records = Vec::new();
        for row in rows {
            let schema_id: String = row.get(0);
            let version: String = row.get(1);
            let schema_json: String = row.get(2);
            let hash_value: String = row.get(3);
            let hash_algorithm: String = row.get(4);
            let description: Option<String> = row.get(5);
            let created_at_json: String = row.get(6);
            let signing_key_id: Option<String> = row.get(7);
            let signing_signature: Option<String> = row.get(8);
            let signing_algorithm: Option<String> = row.get(9);
            let algorithm = match hash_algorithm.as_str() {
                "sha256" => decision_gate_core::hashing::HashAlgorithm::Sha256,
                _ => {
                    return Err(DataShapeRegistryError::Invalid(
                        "unknown hash algorithm".to_string(),
                    ));
                }
            };
            let expected = hash_bytes(algorithm, schema_json.as_bytes());
            if expected.value != hash_value {
                return Err(DataShapeRegistryError::Invalid("schema hash mismatch".to_string()));
            }
            let schema: serde_json::Value = serde_json::from_str(&schema_json)
                .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
            let created_at = serde_json::from_str(&created_at_json)
                .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
            let signing = match (signing_key_id, signing_signature) {
                (Some(key_id), Some(signature)) => Some(decision_gate_core::DataShapeSignature {
                    key_id,
                    signature,
                    algorithm: signing_algorithm,
                }),
                _ => None,
            };
            records.push(DataShapeRecord {
                tenant_id: tenant_id.clone(),
                namespace_id: namespace_id.clone(),
                schema_id: DataShapeId::new(&schema_id),
                version: DataShapeVersion::new(&version),
                schema,
                description,
                created_at,
                signing,
            });
        }
        let has_more = if records.len() > limit_usize {
            records.truncate(limit_usize);
            true
        } else {
            false
        };
        let next_token = if has_more {
            records.last().map(|record| {
                Self::encode_cursor(record.schema_id.as_str(), record.version.as_str())
            })
        } else {
            None
        };
        Ok(DataShapePage {
            items: records,
            next_token,
        })
    }
}

/// Cursor payload for registry pagination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RegistryCursor {
    /// Schema identifier for pagination.
    pub(crate) schema_id: String,
    /// Schema version for pagination.
    pub(crate) version: String,
}

/// Maps hash algorithm enums to persisted labels.
pub(crate) const fn hash_algorithm_label(
    algorithm: decision_gate_core::hashing::HashAlgorithm,
) -> &'static str {
    match algorithm {
        decision_gate_core::hashing::HashAlgorithm::Sha256 => "sha256",
    }
}

/// Builds shared run state + schema registry wrappers for Postgres.
///
/// # Errors
///
/// Returns [`PostgresStoreError`] when initialization fails.
pub fn shared_postgres_store(
    config: &PostgresStoreConfig,
) -> Result<(SharedRunStateStore, SharedDataShapeRegistry), PostgresStoreError> {
    let store = Arc::new(PostgresStore::new(config)?);
    let run_state_store = SharedRunStateStore::new(store.clone());
    let schema_registry = SharedDataShapeRegistry::new(store);
    Ok((run_state_store, schema_registry))
}

#[cfg(test)]
mod tests {
    use decision_gate_core::DataShapeRegistryError;
    use decision_gate_core::hashing::HashAlgorithm;

    use super::PostgresStore;
    use super::hash_algorithm_label;

    #[test]
    fn postgres_store_cursor_roundtrip() {
        let cursor = PostgresStore::encode_cursor("schema-1", "1.2.3");
        let decoded = PostgresStore::decode_cursor(&cursor).expect("decode cursor");
        assert_eq!(decoded.schema_id, "schema-1");
        assert_eq!(decoded.version, "1.2.3");
    }

    #[test]
    fn postgres_store_decode_cursor_rejects_invalid() {
        let result = PostgresStore::decode_cursor("not-json");
        assert!(matches!(result, Err(DataShapeRegistryError::Invalid(_))));
    }

    #[test]
    fn postgres_store_hash_algorithm_label_matches() {
        assert_eq!(hash_algorithm_label(HashAlgorithm::Sha256), "sha256");
    }
}
