// decision-gate-core/src/runtime/store.rs
// ============================================================================
// Module: Decision Gate In-Memory Store
// Description: Simple in-memory run state store for tests and examples.
// Purpose: Provide a deterministic store implementation without external deps.
// Dependencies: crate::core, crate::interfaces
// ============================================================================

//! ## Overview
//! This module provides a simple in-memory implementation of [`RunStateStore`]
//! for tests and local demos. It is not intended for production use.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;
use serde::Serialize;

use crate::core::DataShapeId;
use crate::core::DataShapePage;
use crate::core::DataShapeRecord;
use crate::core::DataShapeVersion;
use crate::core::NamespaceId;
use crate::core::RunId;
use crate::core::RunState;
use crate::core::TenantId;
use crate::interfaces::DataShapeRegistry;
use crate::interfaces::DataShapeRegistryError;
use crate::interfaces::RunStateStore;
use crate::interfaces::StoreError;

// ============================================================================
// SECTION: In-Memory Store
// ============================================================================

/// Default max schema size for in-memory registry (bytes).
const DEFAULT_MAX_SCHEMA_BYTES: usize = 1024 * 1024;

/// Cursor payload for schema pagination.
#[derive(Debug, Serialize, Deserialize)]
struct RegistryCursor {
    /// Schema identifier for the cursor anchor.
    schema_id: String,
    /// Schema version for the cursor anchor.
    version: String,
}

/// In-memory run state store for tests and examples.
#[derive(Debug, Default, Clone)]
pub struct InMemoryRunStateStore {
    /// Run state map protected by a mutex.
    runs: Arc<Mutex<BTreeMap<String, RunState>>>,
}

impl InMemoryRunStateStore {
    /// Creates a new in-memory run state store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runs: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

/// In-memory data shape registry for tests and examples.
#[derive(Debug, Clone)]
pub struct InMemoryDataShapeRegistry {
    /// Registry map protected by a mutex.
    records: Arc<Mutex<BTreeMap<String, DataShapeRecord>>>,
    /// Maximum allowed schema size in bytes.
    max_schema_bytes: usize,
    /// Optional maximum number of records allowed.
    max_entries: Option<usize>,
}

impl Default for InMemoryDataShapeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDataShapeRegistry {
    /// Creates a new in-memory data shape registry with default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_SCHEMA_BYTES, None)
    }

    /// Creates a new in-memory data shape registry with explicit limits.
    #[must_use]
    pub fn with_limits(max_schema_bytes: usize, max_entries: Option<usize>) -> Self {
        Self {
            records: Arc::new(Mutex::new(BTreeMap::new())),
            max_schema_bytes,
            max_entries,
        }
    }
}

impl RunStateStore for InMemoryRunStateStore {
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        let guard = self
            .runs
            .lock()
            .map_err(|_| StoreError::Store("run state store mutex poisoned".to_string()))?;
        let key = run_key(tenant_id, namespace_id, run_id);
        Ok(guard.get(&key).cloned())
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        let key = run_key(&state.tenant_id, &state.namespace_id, &state.run_id);
        self.runs
            .lock()
            .map_err(|_| StoreError::Store("run state store mutex poisoned".to_string()))?
            .insert(key, state.clone());
        Ok(())
    }
}

impl DataShapeRegistry for InMemoryDataShapeRegistry {
    fn register(&self, record: DataShapeRecord) -> Result<(), DataShapeRegistryError> {
        let schema_bytes = serde_json::to_vec(&record.schema)
            .map_err(|err| DataShapeRegistryError::Invalid(err.to_string()))?;
        if schema_bytes.len() > self.max_schema_bytes {
            return Err(DataShapeRegistryError::Invalid(format!(
                "schema exceeds size limit: {} bytes (max {})",
                schema_bytes.len(),
                self.max_schema_bytes
            )));
        }
        let key =
            schema_key(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version);
        let mut guard = self.records.lock().map_err(|_| {
            DataShapeRegistryError::Io("schema registry mutex poisoned".to_string())
        })?;
        if guard.contains_key(&key) {
            return Err(DataShapeRegistryError::Conflict("schema already registered".to_string()));
        }
        if let Some(max_entries) = self.max_entries
            && guard.len() >= max_entries
        {
            return Err(DataShapeRegistryError::Invalid(
                "schema registry max entries exceeded".to_string(),
            ));
        }
        guard.insert(key, record);
        drop(guard);
        Ok(())
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        schema_id: &DataShapeId,
        version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
        let guard = self.records.lock().map_err(|_| {
            DataShapeRegistryError::Io("schema registry mutex poisoned".to_string())
        })?;
        let key = schema_key(tenant_id, namespace_id, schema_id, version);
        Ok(guard.get(&key).cloned())
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
        let mut records: Vec<DataShapeRecord> = {
            let guard = self.records.lock().map_err(|_| {
                DataShapeRegistryError::Io("schema registry mutex poisoned".to_string())
            })?;
            guard
                .values()
                .filter(|record| {
                    record.tenant_id == *tenant_id && record.namespace_id == *namespace_id
                })
                .cloned()
                .collect()
        };
        records.sort_by(|a, b| match a.schema_id.as_str().cmp(b.schema_id.as_str()) {
            std::cmp::Ordering::Equal => a.version.as_str().cmp(b.version.as_str()),
            other => other,
        });

        let start_index = if let Some(cursor) = cursor {
            let RegistryCursor {
                schema_id,
                version,
            } = serde_json::from_str(&cursor)
                .map_err(|_| DataShapeRegistryError::Invalid("invalid cursor".to_string()))?;
            records
                .iter()
                .position(|record| {
                    record.schema_id.as_str() == schema_id && record.version.as_str() == version
                })
                .map_or(0, |idx| idx + 1)
        } else {
            0
        };
        let page_items: Vec<DataShapeRecord> =
            records.into_iter().skip(start_index).take(limit).collect();
        let next_token = page_items.last().map(|record| {
            serde_json::to_string(&RegistryCursor {
                schema_id: record.schema_id.to_string(),
                version: record.version.to_string(),
            })
            .unwrap_or_default()
        });
        Ok(DataShapePage {
            items: page_items,
            next_token,
        })
    }
}

// ============================================================================
// SECTION: Shared Store Wrapper
// ============================================================================

/// Shared run state store backed by an `Arc` trait object.
#[derive(Clone)]
pub struct SharedRunStateStore {
    /// Inner store implementation.
    inner: Arc<dyn RunStateStore + Send + Sync>,
}

impl SharedRunStateStore {
    /// Wraps a run state store in a shared, clonable wrapper.
    #[must_use]
    pub fn from_store(store: impl RunStateStore + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }

    /// Wraps an existing shared store.
    #[must_use]
    pub const fn new(store: Arc<dyn RunStateStore + Send + Sync>) -> Self {
        Self {
            inner: store,
        }
    }
}

impl RunStateStore for SharedRunStateStore {
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError> {
        self.inner.load(tenant_id, namespace_id, run_id)
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        self.inner.save(state)
    }
}

/// Shared data shape registry backed by an `Arc` trait object.
#[derive(Clone)]
pub struct SharedDataShapeRegistry {
    /// Inner registry implementation.
    inner: Arc<dyn DataShapeRegistry + Send + Sync>,
}

impl SharedDataShapeRegistry {
    /// Wraps a data shape registry in a shared, clonable wrapper.
    #[must_use]
    pub fn from_registry(registry: impl DataShapeRegistry + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(registry),
        }
    }

    /// Wraps an existing shared registry.
    #[must_use]
    pub const fn new(registry: Arc<dyn DataShapeRegistry + Send + Sync>) -> Self {
        Self {
            inner: registry,
        }
    }
}

impl DataShapeRegistry for SharedDataShapeRegistry {
    fn register(&self, record: DataShapeRecord) -> Result<(), DataShapeRegistryError> {
        self.inner.register(record)
    }

    fn get(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        schema_id: &DataShapeId,
        version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError> {
        self.inner.get(tenant_id, namespace_id, schema_id, version)
    }

    fn list(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<DataShapePage, DataShapeRegistryError> {
        self.inner.list(tenant_id, namespace_id, cursor, limit)
    }
}

/// Builds a unique run key for the in-memory store.
fn run_key(tenant_id: &TenantId, namespace_id: &NamespaceId, run_id: &RunId) -> String {
    format!("{tenant_id}/{namespace_id}/{run_id}")
}

/// Builds a unique schema key for the in-memory registry.
fn schema_key(
    tenant_id: &TenantId,
    namespace_id: &NamespaceId,
    schema_id: &DataShapeId,
    version: &DataShapeVersion,
) -> String {
    format!("{tenant_id}/{namespace_id}/{schema_id}/{version}")
}
