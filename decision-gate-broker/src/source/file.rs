// decision-gate-broker/src/source/file.rs
// ============================================================================
// Module: Decision Gate File Source
// Description: File-backed source for external payload resolution.
// Purpose: Read payload bytes from local files.
// Dependencies: std, url
// ============================================================================

//! ## Overview
//! `FileSource` resolves `file://` URIs into payload bytes. A root directory can
//! be configured to fail closed on path traversal.
//! Security posture: treats file paths as untrusted input; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::ErrorKind;
use std::io::Read;
use std::path::PathBuf;

use decision_gate_core::ContentRef;
use url::Url;

use crate::source::Source;
use crate::source::SourceError;
use crate::source::SourcePayload;
use crate::source::enforce_max_bytes;

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
    fn resolve_path(&self, uri: &str) -> Result<PathBuf, SourceError> {
        let url = Url::parse(uri).map_err(|err| SourceError::InvalidUri(err.to_string()))?;
        if url.scheme() != "file" {
            return Err(SourceError::UnsupportedScheme(url.scheme().to_string()));
        }
        let path = url
            .to_file_path()
            .map_err(|()| SourceError::InvalidUri("failed to map file url to path".to_string()))?;

        if let Some(root) = &self.root {
            let root =
                std::fs::canonicalize(root).map_err(|err| SourceError::Io(err.to_string()))?;
            let resolved = std::fs::canonicalize(&path).map_err(|err| {
                if err.kind() == ErrorKind::NotFound {
                    SourceError::NotFound(err.to_string())
                } else {
                    SourceError::Io(err.to_string())
                }
            })?;
            if !resolved.starts_with(&root) {
                return Err(SourceError::InvalidUri(
                    "file path escapes configured root".to_string(),
                ));
            }
        }

        Ok(path)
    }

    fn read_with_limit(&self, path: &PathBuf) -> Result<Vec<u8>, SourceError> {
        let file = std::fs::File::open(path).map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                SourceError::NotFound(err.to_string())
            } else {
                SourceError::Io(err.to_string())
            }
        })?;
        let mut limited = file.take((crate::source::MAX_SOURCE_BYTES + 1) as u64);
        let mut bytes = Vec::new();
        limited.read_to_end(&mut bytes).map_err(|err| SourceError::Io(err.to_string()))?;
        enforce_max_bytes(bytes.len())?;
        Ok(bytes)
    }
}

impl Source for FileSource {
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError> {
        let path = self.resolve_path(&content_ref.uri)?;
        let bytes = self.read_with_limit(&path)?;
        Ok(SourcePayload {
            bytes,
            content_type: None,
        })
    }
}
