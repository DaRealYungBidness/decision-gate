// enterprise/decision-gate-store-enterprise/src/runpack_store.rs
// ============================================================================
// Module: Runpack Store
// Description: Runpack storage abstraction for managed deployments.
// Purpose: Provide pluggable runpack storage backends (filesystem, S3, etc.).
// ============================================================================

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::TenantId;
use thiserror::Error;

/// Maximum length of a single path segment.
const MAX_SEGMENT_LENGTH: usize = 255;
/// Maximum total path length for runpack storage.
#[cfg(feature = "s3")]
const MAX_TOTAL_PATH_LENGTH: usize = 4096;

/// Runpack identifier tuple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpackKey {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
}

/// Runpack storage errors.
#[derive(Debug, Error)]
pub enum RunpackStoreError {
    /// I/O error.
    #[error("runpack store io error: {0}")]
    Io(String),
    /// Invalid identifier error.
    #[error("runpack store invalid key: {0}")]
    Invalid(String),
}

/// Runpack storage backend.
pub trait RunpackStore: Send + Sync {
    /// Stores a runpack directory for the given key.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackStoreError`] when validation or storage fails.
    fn put_dir(&self, key: &RunpackKey, source_dir: &Path) -> Result<(), RunpackStoreError>;

    /// Retrieves a runpack directory for the given key.
    ///
    /// # Errors
    ///
    /// Returns [`RunpackStoreError`] when the runpack is missing or cannot be read.
    fn get_dir(&self, key: &RunpackKey, dest_dir: &Path) -> Result<(), RunpackStoreError>;
}

/// Filesystem-backed runpack store (dev/early deployments).
pub struct FilesystemRunpackStore {
    /// Root directory for runpack storage.
    root: PathBuf,
}

impl FilesystemRunpackStore {
    /// Creates a new filesystem runpack store rooted at the provided path.
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self {
            root,
        }
    }

    /// Validates a single path segment for storage safety.
    fn validate_segment(value: &str) -> Result<(), RunpackStoreError> {
        validate_segment(value)
    }

    /// Builds the on-disk path for a runpack key.
    fn key_path(&self, key: &RunpackKey) -> Result<PathBuf, RunpackStoreError> {
        Self::validate_segment(key.tenant_id.as_str())?;
        Self::validate_segment(key.namespace_id.as_str())?;
        Self::validate_segment(key.run_id.as_str())?;
        Ok(self
            .root
            .join(key.tenant_id.as_str())
            .join(key.namespace_id.as_str())
            .join(key.run_id.as_str()))
    }

    /// Recursively copies a directory into the destination, rejecting symlinks.
    fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), RunpackStoreError> {
        fs::create_dir_all(dst).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        for entry in fs::read_dir(src).map_err(|err| RunpackStoreError::Io(err.to_string()))? {
            let entry = entry.map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            let path = entry.path();
            let file_type =
                entry.file_type().map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            if file_type.is_symlink() {
                return Err(RunpackStoreError::Invalid(
                    "runpack directories must not contain symlinks".to_string(),
                ));
            }
            let file_name = entry.file_name();
            let dest_path = dst.join(file_name);
            if file_type.is_dir() {
                Self::copy_dir_recursive(&path, &dest_path)?;
            } else {
                fs::copy(&path, &dest_path)
                    .map_err(|err| RunpackStoreError::Io(err.to_string()))?;
            }
        }
        Ok(())
    }
}

impl RunpackStore for FilesystemRunpackStore {
    fn put_dir(&self, key: &RunpackKey, source_dir: &Path) -> Result<(), RunpackStoreError> {
        let dest = self.key_path(key)?;
        if dest.exists() {
            fs::remove_dir_all(&dest).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        }
        Self::copy_dir_recursive(source_dir, &dest)
    }

    fn get_dir(&self, key: &RunpackKey, dest_dir: &Path) -> Result<(), RunpackStoreError> {
        let source = self.key_path(key)?;
        if !source.exists() {
            return Err(RunpackStoreError::Io("runpack not found".to_string()));
        }
        if dest_dir.exists() {
            fs::remove_dir_all(dest_dir).map_err(|err| RunpackStoreError::Io(err.to_string()))?;
        }
        Self::copy_dir_recursive(&source, dest_dir)
    }
}

/// Validates a single path segment for runpack storage.
pub(crate) fn validate_segment(value: &str) -> Result<(), RunpackStoreError> {
    if value.is_empty() || value == "." || value == ".." {
        return Err(RunpackStoreError::Invalid("segment is invalid".to_string()));
    }
    if value.len() > MAX_SEGMENT_LENGTH {
        return Err(RunpackStoreError::Invalid("segment exceeds length limit".to_string()));
    }
    if value.contains(['/', '\\']) {
        return Err(RunpackStoreError::Invalid("segment contains invalid characters".to_string()));
    }
    Ok(())
}

#[cfg(feature = "s3")]
/// Validates a relative path for runpack archive extraction.
pub(crate) fn validate_relative_path(path: &Path) -> Result<(), RunpackStoreError> {
    let text = path.to_string_lossy();
    if text.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(RunpackStoreError::Invalid("path exceeds length limit".to_string()));
    }
    for component in path.components() {
        match component {
            std::path::Component::Normal(os_str) => {
                let value = os_str.to_string_lossy();
                if value.len() > MAX_SEGMENT_LENGTH {
                    return Err(RunpackStoreError::Invalid(
                        "path segment exceeds length limit".to_string(),
                    ));
                }
                validate_segment(&value)?;
            }
            _ => {
                return Err(RunpackStoreError::Invalid(
                    "path must be relative without traversal".to_string(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
