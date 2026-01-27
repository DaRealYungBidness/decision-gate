// enterprise/decision-gate-enterprise/src/tenant_admin.rs
// ============================================================================
// Module: Tenant Administration
// Description: Tenant lifecycle scaffolding for managed deployments.
// Purpose: Provide admin primitives for tenant provisioning and API keys.
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use decision_gate_core::TenantId;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use rand::RngCore;
use thiserror::Error;

/// Tenant record.
#[derive(Debug, Clone)]
pub struct TenantRecord {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Creation timestamp (ms since epoch).
    pub created_at_ms: u128,
    /// Namespaces registered for the tenant.
    pub namespaces: BTreeSet<String>,
}

/// API key record (hashed).
#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    /// Hashed API key.
    pub key_hash: String,
    /// Creation timestamp (ms since epoch).
    pub created_at_ms: u128,
}

/// Tenant admin errors.
#[derive(Debug, Error)]
pub enum TenantAdminError {
    /// Tenant already exists.
    #[error("tenant already exists")]
    AlreadyExists,
    /// Tenant not found.
    #[error("tenant not found")]
    NotFound,
    /// Storage error.
    #[error("tenant admin storage error: {0}")]
    Storage(String),
}

/// Tenant administration interface.
pub trait TenantAdminStore: Send + Sync {
    /// Creates a tenant record.
    ///
    /// # Errors
    ///
    /// Returns [`TenantAdminError::AlreadyExists`] when the tenant already exists or
    /// [`TenantAdminError::Storage`] when the backing store fails.
    fn create_tenant(&self, tenant_id: TenantId) -> Result<TenantRecord, TenantAdminError>;
    /// Registers a namespace for the tenant.
    ///
    /// # Errors
    ///
    /// Returns [`TenantAdminError::NotFound`] when the tenant does not exist or
    /// [`TenantAdminError::Storage`] when the backing store fails.
    fn add_namespace(&self, tenant_id: &TenantId, namespace: &str) -> Result<(), TenantAdminError>;
    /// Issues an API key for the tenant.
    ///
    /// # Errors
    ///
    /// Returns [`TenantAdminError::NotFound`] when the tenant does not exist or
    /// [`TenantAdminError::Storage`] when the backing store fails.
    fn issue_api_key(&self, tenant_id: &TenantId) -> Result<String, TenantAdminError>;
    /// Lists tenants.
    ///
    /// # Errors
    ///
    /// Returns [`TenantAdminError::Storage`] when the backing store fails.
    fn list_tenants(&self) -> Result<Vec<TenantRecord>, TenantAdminError>;
}

/// In-memory tenant admin store (dev/test).
#[derive(Default)]
pub struct InMemoryTenantAdminStore {
    /// Stored tenant records keyed by tenant id.
    tenants: Mutex<BTreeMap<String, TenantRecord>>,
    /// Issued API keys keyed by tenant id.
    keys: Mutex<BTreeMap<String, Vec<ApiKeyRecord>>>,
}

impl InMemoryTenantAdminStore {
    /// Returns current time in milliseconds since epoch.
    fn now_ms() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
    }

    /// Hashes an API key for storage.
    fn hash_key(raw: &str) -> String {
        let digest = hash_bytes(HashAlgorithm::Sha256, raw.as_bytes());
        digest.value
    }

    /// Generates a new API key.
    fn generate_key() -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }
}

impl TenantAdminStore for InMemoryTenantAdminStore {
    fn create_tenant(&self, tenant_id: TenantId) -> Result<TenantRecord, TenantAdminError> {
        let record = TenantRecord {
            tenant_id: tenant_id.clone(),
            created_at_ms: Self::now_ms(),
            namespaces: BTreeSet::new(),
        };
        {
            let mut tenants = self
                .tenants
                .lock()
                .map_err(|_| TenantAdminError::Storage("tenant lock poisoned".to_string()))?;
            if tenants.contains_key(tenant_id.as_str()) {
                return Err(TenantAdminError::AlreadyExists);
            }
            tenants.insert(tenant_id.as_str().to_string(), record.clone());
        }
        Ok(record)
    }

    fn add_namespace(&self, tenant_id: &TenantId, namespace: &str) -> Result<(), TenantAdminError> {
        let mut tenants = self
            .tenants
            .lock()
            .map_err(|_| TenantAdminError::Storage("tenant lock poisoned".to_string()))?;
        let Some(record) = tenants.get_mut(tenant_id.as_str()) else {
            return Err(TenantAdminError::NotFound);
        };
        record.namespaces.insert(namespace.to_string());
        drop(tenants);
        Ok(())
    }

    fn issue_api_key(&self, tenant_id: &TenantId) -> Result<String, TenantAdminError> {
        let raw = Self::generate_key();
        let hash = Self::hash_key(&raw);
        {
            let mut keys = self
                .keys
                .lock()
                .map_err(|_| TenantAdminError::Storage("key lock poisoned".to_string()))?;
            keys.entry(tenant_id.as_str().to_string()).or_default().push(ApiKeyRecord {
                key_hash: hash,
                created_at_ms: Self::now_ms(),
            });
        }
        Ok(raw)
    }

    fn list_tenants(&self) -> Result<Vec<TenantRecord>, TenantAdminError> {
        let tenants = self
            .tenants
            .lock()
            .map_err(|_| TenantAdminError::Storage("tenant lock poisoned".to_string()))?;
        Ok(tenants.values().cloned().collect())
    }
}
