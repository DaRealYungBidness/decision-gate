// enterprise/decision-gate-enterprise/src/runpack_storage.rs
// ============================================================================
// Module: Enterprise Runpack Storage
// Description: Adapters for MCP runpack storage backends.
// Purpose: Bridge MCP runpack storage trait to enterprise storage crates.
// ============================================================================

//! Runpack storage adapters for enterprise deployments.

use std::sync::Arc;

use decision_gate_mcp::RunpackStorage;
use decision_gate_mcp::RunpackStorageError;
use decision_gate_mcp::RunpackStorageKey;
use decision_gate_store_enterprise::runpack_store::RunpackKey;
use decision_gate_store_enterprise::runpack_store::RunpackStore;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStore;

/// S3-backed runpack storage adapter for MCP.
pub struct S3RunpackStorage {
    /// Inner S3-backed store implementation.
    inner: Arc<S3RunpackStore>,
}

impl S3RunpackStorage {
    /// Wraps an S3 runpack store for MCP usage.
    #[must_use]
    pub fn new(store: S3RunpackStore) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }

    /// Maps enterprise store errors into MCP storage errors.
    fn map_error(
        err: &decision_gate_store_enterprise::runpack_store::RunpackStoreError,
    ) -> RunpackStorageError {
        RunpackStorageError::Backend(err.to_string())
    }
}

impl RunpackStorage for S3RunpackStorage {
    fn store_runpack(
        &self,
        key: &RunpackStorageKey,
        source_dir: &std::path::Path,
    ) -> Result<Option<String>, RunpackStorageError> {
        let store_key = RunpackKey {
            tenant_id: key.tenant_id.clone(),
            namespace_id: key.namespace_id.clone(),
            run_id: key.run_id.clone(),
        };
        self.inner.put_dir(&store_key, source_dir).map_err(|err| Self::map_error(&err))?;
        let uri = self.inner.object_uri(&store_key).map_err(|err| Self::map_error(&err))?;
        Ok(Some(uri))
    }
}
