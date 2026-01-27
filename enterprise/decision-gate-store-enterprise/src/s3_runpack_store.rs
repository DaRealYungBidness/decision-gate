// enterprise/decision-gate-store-enterprise/src/s3_runpack_store.rs
// ============================================================================
// Module: S3 Runpack Store
// Description: S3-backed runpack storage for managed deployments.
// Purpose: Store runpacks in object storage with tenant isolation and integrity.
// ============================================================================

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use aws_config::BehaviorVersion;
use aws_config::Region;
use aws_sdk_s3::Client;
use aws_sdk_s3::types::ServerSideEncryption;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::HashDigest;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use tar::Archive;
use tar::Builder;
use tar::EntryType;
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;

use crate::runpack_store::RunpackKey;
use crate::runpack_store::RunpackStore;
use crate::runpack_store::RunpackStoreError;
use crate::runpack_store::validate_relative_path;
use crate::runpack_store::validate_segment;

/// Default archive filename used for runpack uploads.
const DEFAULT_ARCHIVE_NAME: &str = "runpack.tar";

/// Server-side encryption options for S3.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum S3ServerSideEncryption {
    /// AES256 (SSE-S3).
    Aes256,
    /// KMS-managed encryption (SSE-KMS).
    AwsKms,
}

impl S3ServerSideEncryption {
    /// Converts to the AWS SDK encryption enum.
    const fn as_sdk(self) -> ServerSideEncryption {
        match self {
            Self::Aes256 => ServerSideEncryption::Aes256,
            Self::AwsKms => ServerSideEncryption::AwsKms,
        }
    }
}

/// Configuration for S3-backed runpack storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3RunpackStoreConfig {
    /// Bucket name.
    pub bucket: String,
    /// AWS region (optional; falls back to environment configuration).
    #[serde(default)]
    pub region: Option<String>,
    /// Optional prefix inside the bucket.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Custom endpoint URL (for S3-compatible stores).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Force path-style addressing (for S3-compatible stores).
    #[serde(default)]
    pub force_path_style: bool,
    /// Server-side encryption mode.
    #[serde(default)]
    pub server_side_encryption: Option<S3ServerSideEncryption>,
    /// Optional KMS key id for SSE-KMS.
    #[serde(default)]
    pub kms_key_id: Option<String>,
    /// Optional maximum archive size in bytes.
    #[serde(default)]
    pub max_archive_bytes: Option<u64>,
}

/// S3-backed runpack store.
pub struct S3RunpackStore {
    /// S3 client handle.
    client: Client,
    /// Bucket name for runpack storage.
    bucket: String,
    /// Normalized prefix for object keys.
    prefix: String,
    /// Store configuration.
    config: S3RunpackStoreConfig,
    /// Tokio runtime for blocking S3 calls.
    runtime: Option<Arc<Runtime>>,
}

impl Drop for S3RunpackStore {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let _ = std::thread::spawn(move || drop(runtime));
        }
    }
}

impl S3RunpackStore {
    /// Creates a new S3 runpack store.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackStoreError`] when initialization fails.
    pub fn new(config: S3RunpackStoreConfig) -> Result<Self, RunpackStoreError> {
        if config.bucket.trim().is_empty() {
            return Err(RunpackStoreError::Invalid("bucket must be set".to_string()));
        }
        if matches!(config.server_side_encryption, Some(S3ServerSideEncryption::AwsKms))
            && config.kms_key_id.is_none()
        {
            return Err(RunpackStoreError::Invalid(
                "kms_key_id is required for aws_kms encryption".to_string(),
            ));
        }
        let prefix = normalize_prefix(config.prefix.as_deref())?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let shared_config = runtime.block_on(async {
            let mut loader = aws_config::defaults(BehaviorVersion::latest());
            if let Some(region) = &config.region {
                loader = loader.region(Region::new(region.clone()));
            }
            if let Some(endpoint) = &config.endpoint {
                loader = loader.endpoint_url(endpoint);
            }
            loader.load().await
        });
        let mut s3_builder = aws_sdk_s3::config::Builder::from(&shared_config);
        if config.force_path_style {
            s3_builder = s3_builder.force_path_style(true);
        }
        let client = Client::from_conf(s3_builder.build());
        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix,
            config,
            runtime: Some(Arc::new(runtime)),
        })
    }

    /// Builds the S3 object key for a runpack.
    fn object_key(&self, key: &RunpackKey) -> Result<String, RunpackStoreError> {
        validate_segment(key.tenant_id.as_str())?;
        validate_segment(key.namespace_id.as_str())?;
        validate_segment(key.run_id.as_str())?;
        let mut path = String::new();
        path.push_str(&self.prefix);
        path.push_str(key.tenant_id.as_str());
        path.push('/');
        path.push_str(key.namespace_id.as_str());
        path.push('/');
        path.push_str(key.run_id.as_str());
        path.push('/');
        path.push_str(DEFAULT_ARCHIVE_NAME);
        Ok(path)
    }

    /// Returns the object URI for a stored runpack.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackStoreError`] when the key is invalid.
    pub fn object_uri(&self, key: &RunpackKey) -> Result<String, RunpackStoreError> {
        let object_key = self.object_key(key)?;
        Ok(format!("s3://{}/{}", self.bucket, object_key))
    }

    /// Builds a tar archive for the runpack directory.
    fn build_archive(&self, source_dir: &Path) -> Result<NamedTempFile, RunpackStoreError> {
        if !source_dir.is_dir() {
            return Err(RunpackStoreError::Invalid("source must be a directory".to_string()));
        }
        let archive = tempfile::Builder::new()
            .prefix("dg-runpack-")
            .suffix(".tar")
            .tempfile()
            .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let file = archive.reopen().map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let mut builder = Builder::new(file);
        let mut total_bytes = 0u64;
        append_dir_recursive(
            &mut builder,
            source_dir,
            source_dir,
            &mut total_bytes,
            self.config.max_archive_bytes,
        )?;
        builder.finish().map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        Ok(archive)
    }

    /// Ensures the archive size is within configured limits.
    fn verify_archive_size(&self, path: &Path) -> Result<(), RunpackStoreError> {
        let Some(limit) = self.config.max_archive_bytes else {
            return Ok(());
        };
        let metadata = fs::metadata(path).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        if metadata.len() > limit {
            return Err(RunpackStoreError::Invalid("archive exceeds size limit".to_string()));
        }
        Ok(())
    }

    /// Computes the SHA-256 digest of a file.
    fn compute_sha256(path: &Path) -> Result<HashDigest, RunpackStoreError> {
        let mut file =
            fs::File::open(path).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let read =
                file.read(&mut buffer).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[.. read]);
        }
        let digest = hasher.finalize();
        Ok(HashDigest::new(HashAlgorithm::Sha256, &digest))
    }

    /// Extracts the archive into the destination directory.
    fn extract_archive(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
    ) -> Result<(), RunpackStoreError> {
        let file =
            fs::File::open(archive_path).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let mut archive = Archive::new(file);
        let mut total_bytes = 0u64;
        for entry in archive.entries().map_err(|err| RunpackStoreError::Io(err.to_string()))? {
            let mut entry = entry.map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            let entry_type = entry.header().entry_type();
            match entry_type {
                EntryType::Directory | EntryType::Regular => {}
                _ => {
                    return Err(RunpackStoreError::Invalid(
                        "runpack archives may not contain special entries".to_string(),
                    ));
                }
            }
            let path = entry.path().map_err(|err| RunpackStoreError::Invalid(err.to_string()))?;
            validate_relative_path(&path)?;
            let entry_size = entry.size();
            total_bytes = total_bytes
                .checked_add(entry_size)
                .ok_or_else(|| RunpackStoreError::Invalid("archive size overflow".to_string()))?;
            if let Some(limit) = self.config.max_archive_bytes
                && total_bytes > limit
            {
                return Err(RunpackStoreError::Invalid("archive exceeds size limit".to_string()));
            }
            let dest_path = dest_dir.join(&path);
            if entry_type == EntryType::Directory {
                fs::create_dir_all(&dest_path)
                    .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
                }
                entry.unpack(&dest_path).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            }
        }
        Ok(())
    }
}

impl RunpackStore for S3RunpackStore {
    fn put_dir(&self, key: &RunpackKey, source_dir: &Path) -> Result<(), RunpackStoreError> {
        let archive = self.build_archive(source_dir)?;
        let archive_path = archive.path().to_path_buf();
        self.verify_archive_size(&archive_path)?;
        let digest = Self::compute_sha256(&archive_path)?;
        let object_key = self.object_key(key)?;
        let bucket = self.bucket.clone();
        let client = self.client.clone();
        let sse = self.config.server_side_encryption;
        let kms_key_id = self.config.kms_key_id.clone();
        self.runtime
            .as_ref()
            .ok_or_else(|| RunpackStoreError::Io("runpack store closed".to_string()))?
            .block_on(async {
                let body = aws_sdk_s3::primitives::ByteStream::from_path(&archive_path)
                    .await
                    .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
                let mut metadata = HashMap::new();
                metadata.insert("sha256".to_string(), digest.value.clone());
                let mut request = client
                    .put_object()
                    .bucket(bucket)
                    .key(object_key)
                    .body(body)
                    .set_metadata(Some(metadata))
                    .content_type("application/x-tar");
                if let Some(mode) = sse {
                    request = request.server_side_encryption(mode.as_sdk());
                }
                if let Some(key_id) = kms_key_id {
                    request = request.ssekms_key_id(key_id);
                }
                request.send().await.map_err(|err| RunpackStoreError::Io(err.to_string()))?;
                Ok(())
            })
    }

    fn get_dir(&self, key: &RunpackKey, dest_dir: &Path) -> Result<(), RunpackStoreError> {
        let object_key = self.object_key(key)?;
        let bucket = self.bucket.clone();
        let client = self.client.clone();
        let temp = tempfile::Builder::new()
            .prefix("dg-runpack-download-")
            .suffix(".tar")
            .tempfile()
            .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let temp_path = temp.path().to_path_buf();
        let metadata = self
            .runtime
            .as_ref()
            .ok_or_else(|| RunpackStoreError::Io("runpack store closed".to_string()))?
            .block_on(async {
                let output = client
                    .get_object()
                    .bucket(bucket)
                    .key(object_key)
                    .send()
                    .await
                    .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
                let metadata = output.metadata().cloned();
                let mut file = tokio::fs::File::from_std(
                    temp.reopen().map_err(|err| RunpackStoreError::Io(err.to_string()))?,
                );
                let mut body = output.body.into_async_read();
                tokio::io::copy(&mut body, &mut file)
                    .await
                    .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
                file.flush().await.map_err(|err| RunpackStoreError::Io(err.to_string()))?;
                Ok::<_, RunpackStoreError>(metadata)
            })?;
        self.verify_archive_size(&temp_path)?;
        let digest = Self::compute_sha256(&temp_path)?;
        if let Some(meta) = metadata
            && let Some(expected) = meta.get("sha256")
            && expected != &digest.value
        {
            return Err(RunpackStoreError::Invalid("runpack hash mismatch".to_string()));
        }
        if dest_dir.exists() {
            fs::remove_dir_all(dest_dir).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        }
        fs::create_dir_all(dest_dir).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        self.extract_archive(&temp_path, dest_dir)
    }
}

/// Normalizes an optional prefix into a safe S3 path prefix.
pub(crate) fn normalize_prefix(prefix: Option<&str>) -> Result<String, RunpackStoreError> {
    let Some(prefix) = prefix else {
        return Ok(String::new());
    };
    let trimmed = prefix.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    for segment in trimmed.split('/') {
        validate_segment(segment)?;
    }
    Ok(format!("{trimmed}/"))
}

/// Recursively appends directory contents to a tar builder.
fn append_dir_recursive(
    builder: &mut Builder<fs::File>,
    root: &Path,
    path: &Path,
    total_bytes: &mut u64,
    max_bytes: Option<u64>,
) -> Result<(), RunpackStoreError> {
    for entry in fs::read_dir(path).map_err(|err| RunpackStoreError::Io(err.to_string()))? {
        let entry = entry.map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        let file_type = entry.file_type().map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        if file_type.is_symlink() {
            return Err(RunpackStoreError::Invalid(
                "runpack directories must not contain symlinks".to_string(),
            ));
        }
        let entry_path = entry.path();
        let relative = entry_path
            .strip_prefix(root)
            .map_err(|_| RunpackStoreError::Invalid("runpack path invalid".to_string()))?;
        validate_relative_path(relative)?;
        if file_type.is_dir() {
            builder
                .append_dir(relative, &entry_path)
                .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            append_dir_recursive(builder, root, &entry_path, total_bytes, max_bytes)?;
        } else if file_type.is_file() {
            let metadata =
                entry.metadata().map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            *total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| RunpackStoreError::Invalid("archive size overflow".to_string()))?;
            if let Some(limit) = max_bytes
                && *total_bytes > limit
            {
                return Err(RunpackStoreError::Invalid("archive exceeds size limit".to_string()));
            }
            builder
                .append_path_with_name(&entry_path, relative)
                .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        } else {
            return Err(RunpackStoreError::Invalid(
                "runpack directories must contain only files and directories".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "s3"))]
mod tests {
    use super::normalize_prefix;

    #[test]
    fn normalize_prefix_none_is_empty() {
        let normalized = normalize_prefix(None).expect("normalize");
        assert_eq!(normalized, "");
    }

    #[test]
    fn normalize_prefix_trims_and_appends_slash() {
        let normalized = normalize_prefix(Some("/runs/prefix/")).expect("normalize");
        assert_eq!(normalized, "runs/prefix/");
    }

    #[test]
    fn normalize_prefix_empty_or_root_is_empty() {
        let normalized = normalize_prefix(Some("///")).expect("normalize");
        assert_eq!(normalized, "");
        let normalized = normalize_prefix(Some("")).expect("normalize");
        assert_eq!(normalized, "");
    }

    #[test]
    fn normalize_prefix_rejects_invalid_segments() {
        assert!(normalize_prefix(Some("bad/../prefix")).is_err());
        assert!(normalize_prefix(Some("bad//prefix")).is_err());
        assert!(normalize_prefix(Some("bad\\prefix")).is_err());
    }
}
