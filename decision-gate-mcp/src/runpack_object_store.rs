// decision-gate-mcp/src/runpack_object_store.rs
// ============================================================================
// Module: Runpack Object Storage
// Description: Object-store artifact sink/reader for runpack export and verify.
// Purpose: Persist runpacks in durable object storage with strict validation.
// Dependencies: decision-gate-core, aws-sdk-s3, tokio
// ============================================================================

//! ## Overview
//! This module provides object-store-backed [`ArtifactSink`] and
//! [`ArtifactReader`] implementations for runpack export and verification.
//! Object keys are derived from tenant/namespace/scenario/run identifiers and
//! the scenario spec hash. Security posture: storage is untrusted; all keys and
//! payload sizes are validated; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::future::Future;
use std::path::Component;
use std::path::Path;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;

use aws_config::BehaviorVersion;
use aws_config::Region;
use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use decision_gate_core::Artifact;
use decision_gate_core::ArtifactError;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactRef;
use decision_gate_core::ArtifactSink;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::HashDigest;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunpackManifest;
use decision_gate_core::ScenarioId;
use decision_gate_core::TenantId;
use decision_gate_core::runtime::runpack::MAX_RUNPACK_ARTIFACT_BYTES;
use tokio::io::AsyncReadExt;
use tokio::runtime::Handle;
use tokio::runtime::Runtime;
use tokio::runtime::RuntimeFlavor;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum length of a single key segment.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total key length.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;

// ============================================================================
// SECTION: Runtime Helpers
// ============================================================================

/// Blocks on an object-store future using a compatible runtime.
fn block_on_with_runtime<F, T>(runtime: &Runtime, future: F) -> Result<T, ObjectStoreError>
where
    F: Future<Output = Result<T, ObjectStoreError>> + Send + 'static,
    T: Send + 'static,
{
    if let Ok(handle) = Handle::try_current() {
        if matches!(handle.runtime_flavor(), RuntimeFlavor::MultiThread) {
            return tokio::task::block_in_place(|| handle.block_on(future));
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = Runtime::new()
                .map_err(|err| ObjectStoreError::Io(err.to_string()))
                .and_then(|runtime| runtime.block_on(future));
            let _ = tx.send(result);
        });
        return rx
            .recv()
            .unwrap_or_else(|_| Err(ObjectStoreError::Io("object store thread join failed".to_string())));
    }

    runtime.block_on(future)
}

use crate::config::ObjectStoreConfig;
use crate::config::ObjectStoreProvider;

// ============================================================================
// SECTION: Runpack Key
// ============================================================================

/// Runpack object-store key derivation inputs.
#[derive(Debug, Clone)]
pub struct RunpackObjectKey {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Run identifier.
    pub run_id: RunId,
    /// Scenario specification hash.
    pub spec_hash: HashDigest,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Object-store errors for runpack storage.
#[derive(Debug, thiserror::Error)]
pub enum ObjectStoreError {
    /// Invalid configuration or key input.
    #[error("object store invalid: {0}")]
    Invalid(String),
    /// Backend I/O failure.
    #[error("object store io error: {0}")]
    Io(String),
    /// Backend returned an error.
    #[error("object store backend error: {0}")]
    Backend(String),
    /// Object exceeds size limits.
    #[error("object too large: {path} ({actual_bytes} > {max_bytes})")]
    TooLarge {
        /// Object path.
        path: String,
        /// Maximum allowed bytes.
        max_bytes: usize,
        /// Actual size in bytes.
        actual_bytes: usize,
    },
}

// ============================================================================
// SECTION: Object Store Client
// ============================================================================

/// Minimal object-store client abstraction.
pub(crate) trait ObjectStoreClient: Send + Sync {
    /// Writes a single object to storage.
    fn put(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError>;
    /// Reads a single object from storage with a size limit.
    fn get(&self, key: &str, max_bytes: usize) -> Result<Vec<u8>, ObjectStoreError>;
}

/// S3-backed object-store client.
struct S3ObjectStoreClient {
    /// Underlying S3 client.
    client: Client,
    /// Bucket name.
    bucket: String,
    /// Prefix for object keys.
    prefix: String,
    /// Tokio runtime for blocking S3 operations.
    runtime: Option<Arc<Runtime>>,
}

impl Drop for S3ObjectStoreClient {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let _ = std::thread::spawn(move || drop(runtime));
        }
    }
}

impl S3ObjectStoreClient {
    /// Builds a new S3-backed object store client.
    fn new(config: &ObjectStoreConfig) -> Result<Self, ObjectStoreError> {
        config.validate().map_err(|err| ObjectStoreError::Invalid(err.to_string()))?;
        let prefix = normalize_prefix(config.prefix.as_deref().unwrap_or(""))?;
        let runtime = Runtime::new().map_err(|err| ObjectStoreError::Io(err.to_string()))?;
        let region = config.region.clone();
        let endpoint = config.endpoint.clone();
        let shared_config = block_on_with_runtime(&runtime, async {
            let mut loader = aws_config::defaults(BehaviorVersion::latest());
            if let Some(region) = region {
                loader = loader.region(Region::new(region));
            }
            if let Some(endpoint) = endpoint {
                loader = loader.endpoint_url(endpoint);
            }
            Ok(loader.load().await)
        })?;
        let mut s3_builder = aws_sdk_s3::config::Builder::from(&shared_config);
        if config.force_path_style {
            s3_builder = s3_builder.force_path_style(true);
        }
        let client = Client::from_conf(s3_builder.build());
        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix,
            runtime: Some(Arc::new(runtime)),
        })
    }

    /// Applies the configured prefix to a key.
    fn prefixed_key(&self, key: &str) -> String {
        if self.prefix.is_empty() { key.to_string() } else { format!("{}{}", self.prefix, key) }
    }

    /// Returns the runtime or an error if shutdown.
    fn runtime(&self) -> Result<&Runtime, ObjectStoreError> {
        self.runtime
            .as_ref()
            .map(AsRef::as_ref)
            .ok_or_else(|| ObjectStoreError::Io("object store runtime closed".to_string()))
    }
}

impl ObjectStoreClient for S3ObjectStoreClient {
    fn put(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        let bucket = self.bucket.clone();
        let key = self.prefixed_key(key);
        let client = self.client.clone();
        let content_type = content_type.map(str::to_string);
        block_on_with_runtime(self.runtime()?, async move {
            let body = ByteStream::from(bytes);
            let mut request = client.put_object().bucket(bucket).key(key).body(body);
            if let Some(content_type) = content_type {
                request = request.content_type(content_type);
            }
            request.send().await.map_err(|err| ObjectStoreError::Backend(err.to_string()))?;
            Ok(())
        })
    }

    fn get(&self, key: &str, max_bytes: usize) -> Result<Vec<u8>, ObjectStoreError> {
        let bucket = self.bucket.clone();
        let key = self.prefixed_key(key);
        let client = self.client.clone();
        block_on_with_runtime(self.runtime()?, async move {
            let output = client
                .get_object()
                .bucket(bucket)
                .key(key.clone())
                .send()
                .await
                .map_err(|err| ObjectStoreError::Backend(err.to_string()))?;
            if let Some(length) = output.content_length() {
                let actual_bytes = usize::try_from(length).unwrap_or(usize::MAX);
                if actual_bytes > max_bytes {
                    return Err(ObjectStoreError::TooLarge {
                        path: key.clone(),
                        max_bytes,
                        actual_bytes,
                    });
                }
            }
            let mut reader = output.body.into_async_read();
            let mut buffer = Vec::new();
            let mut total_bytes = 0usize;
            let mut chunk = [0u8; 8192];
            loop {
                let read = reader
                    .read(&mut chunk)
                    .await
                    .map_err(|err| ObjectStoreError::Io(err.to_string()))?;
                if read == 0 {
                    break;
                }
                total_bytes = total_bytes
                    .checked_add(read)
                    .ok_or_else(|| ObjectStoreError::Io("object size overflow".to_string()))?;
                if total_bytes > max_bytes {
                    return Err(ObjectStoreError::TooLarge {
                        path: key.clone(),
                        max_bytes,
                        actual_bytes: total_bytes,
                    });
                }
                buffer.extend_from_slice(&chunk[.. read]);
            }
            Ok(buffer)
        })
    }
}

// ============================================================================
// SECTION: Backend Wrapper
// ============================================================================

/// Object-store backend for runpack export and verification.
pub struct ObjectStoreRunpackBackend {
    /// Object-store client implementation.
    client: Arc<dyn ObjectStoreClient>,
    /// Bucket name used for storage.
    bucket: String,
    /// Storage URI scheme (e.g., s3).
    scheme: &'static str,
    /// Root prefix prepended to all keys.
    root_prefix: String,
}

impl ObjectStoreRunpackBackend {
    /// Creates a backend from object-store configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectStoreError`] when configuration or initialization fails.
    pub fn new(config: &ObjectStoreConfig) -> Result<Self, ObjectStoreError> {
        config.validate().map_err(|err| ObjectStoreError::Invalid(err.to_string()))?;
        let root_prefix = normalize_prefix(config.prefix.as_deref().unwrap_or(""))?;
        let (client, scheme) = match config.provider {
            ObjectStoreProvider::S3 => {
                (Arc::new(S3ObjectStoreClient::new(config)?) as Arc<_>, "s3")
            }
        };
        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            scheme,
            root_prefix,
        })
    }

    /// Creates a backend from a custom object-store client (tests only).
    #[cfg(test)]
    pub(crate) fn from_client(bucket: &str, client: Arc<dyn ObjectStoreClient>) -> Self {
        Self {
            client,
            bucket: bucket.to_string(),
            scheme: "memory",
            root_prefix: String::new(),
        }
    }

    /// Returns the storage URI for a runpack root.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectStoreError`] when the runpack key is invalid.
    pub fn storage_uri(&self, key: &RunpackObjectKey) -> Result<String, ObjectStoreError> {
        let prefix = format!("{}{}", self.root_prefix, runpack_prefix(key)?);
        Ok(format!("{}://{}/{}", self.scheme, self.bucket, prefix))
    }

    /// Creates a sink for runpack export.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectStoreError`] when the runpack key or manifest name is invalid.
    pub fn sink(
        &self,
        key: &RunpackObjectKey,
        manifest_name: &str,
    ) -> Result<ObjectStoreArtifactSink, ObjectStoreError> {
        let prefix = runpack_prefix(key)?;
        ObjectStoreArtifactSink::new(
            self.client.clone(),
            self.root_prefix.clone(),
            prefix,
            manifest_name,
        )
    }

    /// Creates a reader for runpack verification.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectStoreError`] when the runpack key is invalid.
    pub fn reader(
        &self,
        key: &RunpackObjectKey,
    ) -> Result<ObjectStoreArtifactReader, ObjectStoreError> {
        let prefix = runpack_prefix(key)?;
        Ok(ObjectStoreArtifactReader {
            client: self.client.clone(),
            root_prefix: self.root_prefix.clone(),
            prefix,
        })
    }
}

// ============================================================================
// SECTION: Artifact Sink/Reader
// ============================================================================

/// Object-store-backed artifact sink.
pub struct ObjectStoreArtifactSink {
    /// Object-store client.
    client: Arc<dyn ObjectStoreClient>,
    /// Root prefix prepended to keys.
    root_prefix: String,
    /// Runpack-specific prefix.
    prefix: String,
    /// Manifest file name.
    manifest_name: String,
}

impl ObjectStoreArtifactSink {
    /// Creates a new object-store artifact sink.
    fn new(
        client: Arc<dyn ObjectStoreClient>,
        root_prefix: String,
        prefix: String,
        manifest_name: &str,
    ) -> Result<Self, ObjectStoreError> {
        validate_relative_path(manifest_name)?;
        Ok(Self {
            client,
            root_prefix,
            prefix,
            manifest_name: manifest_name.to_string(),
        })
    }

    /// Returns the object key for a runpack-relative path.
    fn object_key(&self, relative: &str) -> Result<String, ObjectStoreError> {
        validate_relative_path(relative)?;
        let candidate = format!("{}{}", self.prefix, relative);
        let full = format!("{}{}", self.root_prefix, candidate);
        if full.len() > MAX_TOTAL_PATH_LENGTH {
            return Err(ObjectStoreError::Invalid("object key exceeds length limit".to_string()));
        }
        Ok(candidate)
    }
}

impl ArtifactSink for ObjectStoreArtifactSink {
    fn write(&mut self, artifact: &Artifact) -> Result<ArtifactRef, ArtifactError> {
        let path = artifact.path.as_str();
        if artifact.bytes.len() > MAX_RUNPACK_ARTIFACT_BYTES {
            return Err(ArtifactError::TooLarge {
                path: path.to_string(),
                max_bytes: MAX_RUNPACK_ARTIFACT_BYTES,
                actual_bytes: artifact.bytes.len(),
            });
        }
        let key = self.object_key(path).map_err(|err| ArtifactError::Sink(err.to_string()))?;
        // ArtifactSink accepts references; clone bytes to hand ownership to the object store
        // client.
        self.client
            .put(&key, artifact.bytes.clone(), artifact.content_type.as_deref())
            .map_err(|err| ArtifactError::Sink(err.to_string()))?;
        Ok(ArtifactRef {
            uri: key,
        })
    }

    fn finalize(&mut self, manifest: &RunpackManifest) -> Result<ArtifactRef, ArtifactError> {
        let bytes =
            serde_jcs::to_vec(manifest).map_err(|err| ArtifactError::Sink(err.to_string()))?;
        if bytes.len() > MAX_RUNPACK_ARTIFACT_BYTES {
            return Err(ArtifactError::TooLarge {
                path: self.manifest_name.clone(),
                max_bytes: MAX_RUNPACK_ARTIFACT_BYTES,
                actual_bytes: bytes.len(),
            });
        }
        let key = self
            .object_key(&self.manifest_name)
            .map_err(|err| ArtifactError::Sink(err.to_string()))?;
        self.client
            .put(&key, bytes, Some("application/json"))
            .map_err(|err| ArtifactError::Sink(err.to_string()))?;
        Ok(ArtifactRef {
            uri: key,
        })
    }
}

/// Object-store-backed artifact reader.
pub struct ObjectStoreArtifactReader {
    /// Object-store client.
    client: Arc<dyn ObjectStoreClient>,
    /// Root prefix prepended to keys.
    root_prefix: String,
    /// Runpack-specific prefix.
    prefix: String,
}

impl ObjectStoreArtifactReader {
    /// Returns the object key for a runpack-relative path.
    fn object_key(&self, relative: &str) -> Result<String, ObjectStoreError> {
        validate_relative_path(relative)?;
        let candidate = format!("{}{}", self.prefix, relative);
        let full = format!("{}{}", self.root_prefix, candidate);
        if full.len() > MAX_TOTAL_PATH_LENGTH {
            return Err(ObjectStoreError::Invalid("object key exceeds length limit".to_string()));
        }
        Ok(candidate)
    }
}

impl ArtifactReader for ObjectStoreArtifactReader {
    fn read_with_limit(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, ArtifactError> {
        let key = self.object_key(path).map_err(|err| ArtifactError::Sink(err.to_string()))?;
        match self.client.get(&key, max_bytes) {
            Ok(bytes) => Ok(bytes),
            Err(ObjectStoreError::TooLarge {
                max_bytes,
                actual_bytes,
                ..
            }) => Err(ArtifactError::TooLarge {
                path: path.to_string(),
                max_bytes,
                actual_bytes,
            }),
            Err(err) => Err(ArtifactError::Sink(err.to_string())),
        }
    }
}

// ============================================================================
// SECTION: Key Derivation Helpers
// ============================================================================

/// Builds the runpack key prefix for object storage.
fn runpack_prefix(key: &RunpackObjectKey) -> Result<String, ObjectStoreError> {
    let tenant = key.tenant_id.get().to_string();
    let namespace = key.namespace_id.get().to_string();
    let scenario = key.scenario_id.as_str();
    let run_id = key.run_id.as_str();
    let hash_algorithm = hash_algorithm_label(key.spec_hash.algorithm);
    let hash_value = key.spec_hash.value.as_str();
    for segment in [
        "tenant",
        tenant.as_str(),
        "namespace",
        namespace.as_str(),
        "scenario",
        scenario,
        "run",
        run_id,
        "spec",
        hash_algorithm,
        hash_value,
    ] {
        validate_segment(segment)?;
    }
    let prefix = format!(
        "tenant/{tenant}/namespace/{namespace}/scenario/{scenario}/run/{run_id}/spec/\
         {hash_algorithm}/{hash_value}/"
    );
    if prefix.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ObjectStoreError::Invalid("runpack prefix exceeds length limit".to_string()));
    }
    Ok(prefix)
}

/// Returns the canonical label for a hash algorithm.
const fn hash_algorithm_label(algorithm: HashAlgorithm) -> &'static str {
    match algorithm {
        HashAlgorithm::Sha256 => "sha256",
    }
}

/// Normalizes a root prefix string for object storage.
fn normalize_prefix(raw: &str) -> Result<String, ObjectStoreError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.starts_with('/') {
        return Err(ObjectStoreError::Invalid(
            "prefix must be relative (no leading slash)".to_string(),
        ));
    }
    let normalized = trimmed.strip_suffix('/').unwrap_or(trimmed);
    validate_relative_path(normalized)?;
    Ok(format!("{normalized}/"))
}

/// Validates a runpack-relative path string.
fn validate_relative_path(path: &str) -> Result<(), ObjectStoreError> {
    if path.is_empty() {
        return Err(ObjectStoreError::Invalid("path must be set".to_string()));
    }
    if path.contains('\\') {
        return Err(ObjectStoreError::Invalid("path must not contain backslashes".to_string()));
    }
    if path.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ObjectStoreError::Invalid("path exceeds length limit".to_string()));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(ObjectStoreError::Invalid("path must be relative".to_string()));
    }
    for component in candidate.components() {
        match component {
            Component::Normal(value) => {
                let segment = value.to_string_lossy();
                if segment.len() > MAX_PATH_COMPONENT_LENGTH {
                    return Err(ObjectStoreError::Invalid(
                        "path segment exceeds length limit".to_string(),
                    ));
                }
                validate_segment(&segment)?;
            }
            _ => {
                return Err(ObjectStoreError::Invalid(
                    "path must be relative without traversal".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Validates a single path segment.
fn validate_segment(value: &str) -> Result<(), ObjectStoreError> {
    if value.is_empty() || value == "." || value == ".." {
        return Err(ObjectStoreError::Invalid("segment is invalid".to_string()));
    }
    if value.len() > MAX_PATH_COMPONENT_LENGTH {
        return Err(ObjectStoreError::Invalid("segment exceeds length limit".to_string()));
    }
    if value.contains(['/', '\\']) {
        return Err(ObjectStoreError::Invalid("segment contains invalid characters".to_string()));
    }
    Ok(())
}

// ============================================================================
// SECTION: In-Memory Test Client
// ============================================================================

#[cfg(test)]
struct InMemoryObjectStore {
    objects: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

#[cfg(test)]
impl InMemoryObjectStore {
    fn new() -> Self {
        Self {
            objects: Mutex::new(std::collections::BTreeMap::new()),
        }
    }
}

#[cfg(test)]
impl ObjectStoreClient for InMemoryObjectStore {
    fn put(
        &self,
        key: &str,
        bytes: Vec<u8>,
        _content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        self.objects
            .lock()
            .map_err(|_| ObjectStoreError::Io("object store lock poisoned".to_string()))?
            .insert(key.to_string(), bytes);
        Ok(())
    }

    fn get(&self, key: &str, max_bytes: usize) -> Result<Vec<u8>, ObjectStoreError> {
        let bytes = self
            .objects
            .lock()
            .map_err(|_| ObjectStoreError::Io("object store lock poisoned".to_string()))?
            .get(key)
            .ok_or_else(|| ObjectStoreError::Io("object not found".to_string()))?
            .clone();
        if bytes.len() > max_bytes {
            return Err(ObjectStoreError::TooLarge {
                path: key.to_string(),
                max_bytes,
                actual_bytes: bytes.len(),
            });
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests;
