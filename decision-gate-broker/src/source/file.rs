// decision-gate-broker/src/source/file.rs
// ============================================================================
// Module: Decision Gate File Source
// Description: File-backed source for external payload resolution.
// Purpose: Read payload bytes from local files.
// Dependencies: decision-gate-core, std, url
// ============================================================================

//! ## Overview
//! [`FileSource`] resolves `file://` URIs into payload bytes. A root directory can
//! be configured to fail closed on path traversal.
//! Security posture: treats file paths as untrusted input; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::ErrorKind;
use std::io::Read;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use cap_primitives::fs::FollowSymlinks;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use cap_std::fs::OpenOptions;
use decision_gate_core::ContentRef;
use url::Url;

use crate::source::Source;
use crate::source::SourceError;
use crate::source::SourcePayload;
use crate::source::enforce_max_bytes;
use crate::source::max_source_bytes_u64;

// ============================================================================
// SECTION: File Source
// ============================================================================

/// File-backed payload source.
#[derive(Debug, Clone)]
pub struct FileSource {
    /// Optional root directory for path traversal protection.
    root: Option<PathBuf>,
}

impl FileSource {
    /// Creates a file source rooted at the provided directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
        }
    }

    /// Creates a file source with no root restrictions.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            root: None,
        }
    }

    /// Resolves a file URI into a local path.
    /// Resolves a file URI into a local path.
    fn resolve_path(uri: &str) -> Result<PathBuf, SourceError> {
        let url = Url::parse(uri).map_err(|err| SourceError::InvalidUri(err.to_string()))?;
        if url.scheme() != "file" {
            return Err(SourceError::UnsupportedScheme(url.scheme().to_string()));
        }
        let path = url
            .to_file_path()
            .map_err(|()| SourceError::InvalidUri("failed to map file url to path".to_string()))?;
        Ok(path)
    }

    /// Reads bytes from disk while enforcing the maximum source size.
    /// Reads bytes while enforcing the maximum source size.
    fn read_with_limit<R: Read>(file: R) -> Result<Vec<u8>, SourceError> {
        let max_bytes = max_source_bytes_u64()?;
        let limit = max_bytes.checked_add(1).ok_or(SourceError::LimitOverflow {
            limit: crate::source::MAX_SOURCE_BYTES,
        })?;
        let mut limited = file.take(limit);
        let mut bytes = Vec::new();
        limited.read_to_end(&mut bytes).map_err(|err| SourceError::Io(err.to_string()))?;
        enforce_max_bytes(bytes.len())?;
        Ok(bytes)
    }

    /// Normalizes a root path into an absolute path.
    fn normalize_root_path(root: &Path) -> Result<PathBuf, SourceError> {
        if root.is_absolute() {
            return Ok(root.to_path_buf());
        }
        std::env::current_dir()
            .map(|cwd| cwd.join(root))
            .map_err(|err| SourceError::Io(err.to_string()))
    }

    /// Returns a safe, relative path from the configured root.
    fn relative_from_root(root: &Path, path: &Path) -> Result<PathBuf, SourceError> {
        let relative = path.strip_prefix(root).map_err(|_| {
            SourceError::InvalidUri("file path escapes configured root".to_string())
        })?;
        for component in relative.components() {
            match component {
                Component::Normal(_) => {}
                _ => {
                    return Err(SourceError::InvalidUri(
                        "file path escapes configured root".to_string(),
                    ));
                }
            }
        }
        if relative.as_os_str().is_empty() {
            return Err(SourceError::InvalidUri("file path refers to root directory".to_string()));
        }
        Ok(relative.to_path_buf())
    }

    /// Opens a file within the root using capability-based APIs.
    fn open_rooted_file(root: &Path, relative: &Path) -> Result<cap_std::fs::File, SourceError> {
        let dir =
            Dir::open_ambient_dir(root, ambient_authority()).map_err(|err| map_open_error(&err))?;
        let mut options = OpenOptions::new();
        options.read(true);
        options._cap_fs_ext_follow(FollowSymlinks::No);
        dir.open_with(relative, &options).map_err(|err| map_open_error(&err))
    }

    /// Returns true when the path is a directory (without following symlinks).
    fn path_is_directory(path: &Path) -> bool {
        std::fs::symlink_metadata(path).map(|metadata| metadata.is_dir()).unwrap_or(false)
    }

    /// Ensures the opened handle is a regular file.
    fn ensure_regular_file(file: &cap_std::fs::File) -> Result<(), SourceError> {
        let metadata = file.metadata().map_err(|err| SourceError::Io(err.to_string()))?;
        if metadata.is_dir() {
            return Err(SourceError::InvalidUri("file path refers to directory".to_string()));
        }
        Ok(())
    }
}

impl Source for FileSource {
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError> {
        let path = Self::resolve_path(&content_ref.uri)?;
        let file = if let Some(root) = &self.root {
            let root = Self::normalize_root_path(root)?;
            let relative = Self::relative_from_root(&root, &path)?;
            let full_path = root.join(&relative);
            match Self::open_rooted_file(&root, &relative) {
                Ok(file) => file,
                Err(err) => {
                    if Self::path_is_directory(&full_path) {
                        return Err(SourceError::InvalidUri(
                            "file path refers to directory".to_string(),
                        ));
                    }
                    return Err(err);
                }
            }
        } else {
            match cap_std::fs::File::open_ambient(&path, ambient_authority())
                .map_err(|err| map_open_error(&err))
            {
                Ok(file) => file,
                Err(err) => {
                    if Self::path_is_directory(&path) {
                        return Err(SourceError::InvalidUri(
                            "file path refers to directory".to_string(),
                        ));
                    }
                    return Err(err);
                }
            }
        };
        Self::ensure_regular_file(&file)?;
        let bytes = Self::read_with_limit(file)?;
        Ok(SourcePayload {
            bytes,
            content_type: None,
        })
    }
}

/// Maps IO errors into source errors with policy context.
fn map_open_error(err: &std::io::Error) -> SourceError {
    if err.kind() == ErrorKind::NotFound {
        return SourceError::NotFound(err.to_string());
    }
    if err.kind() == ErrorKind::InvalidInput || err.kind() == ErrorKind::PermissionDenied {
        return SourceError::InvalidUri(err.to_string());
    }
    SourceError::Io(err.to_string())
}
