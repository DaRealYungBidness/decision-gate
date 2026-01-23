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

use crate::core::RunId;
use crate::core::RunState;
use crate::interfaces::RunStateStore;
use crate::interfaces::StoreError;

// ============================================================================
// SECTION: In-Memory Store
// ============================================================================

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

impl RunStateStore for InMemoryRunStateStore {
    fn load(&self, run_id: &RunId) -> Result<Option<RunState>, StoreError> {
        let guard = self
            .runs
            .lock()
            .map_err(|_| StoreError::Store("run state store mutex poisoned".to_string()))?;
        Ok(guard.get(run_id.as_str()).cloned())
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        self.runs
            .lock()
            .map_err(|_| StoreError::Store("run state store mutex poisoned".to_string()))?
            .insert(state.run_id.as_str().to_string(), state.clone());
        Ok(())
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
    fn load(&self, run_id: &RunId) -> Result<Option<RunState>, StoreError> {
        self.inner.load(run_id)
    }

    fn save(&self, state: &RunState) -> Result<(), StoreError> {
        self.inner.save(state)
    }
}
