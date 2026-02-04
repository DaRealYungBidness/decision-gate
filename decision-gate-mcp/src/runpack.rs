// decision-gate-mcp/src/runpack.rs
// ============================================================================
// Module: MCP Runpack IO
// Description: File-backed artifact sink/reader for runpack export/verify.
// Purpose: Support runpack export and offline verification over the filesystem.
// Dependencies: decision-gate-core
// ============================================================================

//! ## Overview
//! This module provides filesystem-backed artifact sinks/readers with strict
//! path validation. Security posture: runpack paths are untrusted and must be
//! validated; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use decision_gate_core::Artifact;
use decision_gate_core::ArtifactError;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactRef;
use decision_gate_core::ArtifactSink;
use decision_gate_core::RunpackManifest;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum length of a single path component to prevent path abuse.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length for runpack storage.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;

// ============================================================================
// SECTION: File Artifact Sink
// ============================================================================

/// File-backed artifact sink for runpack export.
///
/// # Invariants
/// - All artifact paths are resolved under the validated root.
pub struct FileArtifactSink {
    /// Root directory for artifact storage.
    root: PathBuf,
    /// Manifest output path.
    manifest_path: PathBuf,
}

impl FileArtifactSink {
    /// Creates a new file artifact sink rooted at the given directory.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError`] when the root path is invalid.
    pub fn new(root: PathBuf, manifest_name: &str) -> Result<Self, ArtifactError> {
        validate_path(&root)?;
        let manifest_relative = PathBuf::from(manifest_name);
        if manifest_relative.file_name().is_none() {
            return Err(ArtifactError::Sink("manifest name missing filename".to_string()));
        }
        ensure_relative_path(&manifest_relative)?;
        let manifest_path = root.join(&manifest_relative);
        validate_path(&manifest_path)?;
        Ok(Self {
            root,
            manifest_path,
        })
    }
}

impl ArtifactSink for FileArtifactSink {
    fn write(&mut self, artifact: &Artifact) -> Result<ArtifactRef, ArtifactError> {
        let candidate = PathBuf::from(&artifact.path);
        ensure_relative_path(&candidate)?;
        let joined = self.root.join(&candidate);
        if let Some(parent) = joined.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                ArtifactError::Sink("unable to create artifact directory".to_string())
            })?;
        }
        let path = resolve_path(&self.root, &artifact.path)?;
        fs::write(&path, &artifact.bytes)
            .map_err(|_| ArtifactError::Sink("unable to write artifact".to_string()))?;
        Ok(ArtifactRef {
            uri: path.to_string_lossy().to_string(),
        })
    }

    fn finalize(&mut self, manifest: &RunpackManifest) -> Result<ArtifactRef, ArtifactError> {
        let bytes =
            serde_jcs::to_vec(manifest).map_err(|err| ArtifactError::Sink(err.to_string()))?;
        if let Some(parent) = self.manifest_path.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                ArtifactError::Sink("unable to create manifest directory".to_string())
            })?;
        }
        fs::write(&self.manifest_path, bytes)
            .map_err(|_| ArtifactError::Sink("unable to write manifest".to_string()))?;
        Ok(ArtifactRef {
            uri: self.manifest_path.to_string_lossy().to_string(),
        })
    }
}

// ============================================================================
// SECTION: File Artifact Reader
// ============================================================================

/// File-backed artifact reader for runpack verification.
///
/// # Invariants
/// - All artifact paths are resolved under the validated root.
pub struct FileArtifactReader {
    /// Root directory for artifact reads.
    root: PathBuf,
}

impl FileArtifactReader {
    /// Creates a new file artifact reader rooted at the given directory.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError`] when the root path is invalid.
    pub fn new(root: PathBuf) -> Result<Self, ArtifactError> {
        validate_path(&root)?;
        Ok(Self {
            root,
        })
    }
}

impl ArtifactReader for FileArtifactReader {
    fn read_with_limit(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, ArtifactError> {
        let resolved = resolve_path(&self.root, path)?;
        let metadata = fs::metadata(&resolved)
            .map_err(|_| ArtifactError::Sink("unable to read artifact metadata".to_string()))?;
        let actual_bytes = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        let max_bytes_u64 = u64::try_from(max_bytes).unwrap_or(u64::MAX);
        if metadata.len() > max_bytes_u64 {
            return Err(ArtifactError::TooLarge {
                path: path.to_string(),
                max_bytes,
                actual_bytes,
            });
        }
        let bytes = fs::read(&resolved)
            .map_err(|_| ArtifactError::Sink("unable to read artifact".to_string()))?;
        if bytes.len() > max_bytes {
            return Err(ArtifactError::TooLarge {
                path: path.to_string(),
                max_bytes,
                actual_bytes: bytes.len(),
            });
        }
        Ok(bytes)
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Resolves and validates an artifact path relative to a runpack root.
fn resolve_path(root: &Path, relative: &str) -> Result<PathBuf, ArtifactError> {
    let candidate = PathBuf::from(relative);
    ensure_relative_path(&candidate)?;
    let root = root
        .canonicalize()
        .map_err(|_| ArtifactError::Sink("unable to resolve runpack root".to_string()))?;
    let joined = root.join(&candidate);
    let parent = joined
        .parent()
        .ok_or_else(|| ArtifactError::Sink("artifact path missing parent".to_string()))?;
    let parent = parent
        .canonicalize()
        .map_err(|_| ArtifactError::Sink("unable to resolve artifact path".to_string()))?;
    if !parent.starts_with(&root) {
        return Err(ArtifactError::Sink("artifact path escapes runpack root".to_string()));
    }
    let file_name = candidate
        .file_name()
        .ok_or_else(|| ArtifactError::Sink("artifact path missing filename".to_string()))?;
    Ok(parent.join(file_name))
}

/// Validates a runpack path against length constraints.
fn validate_path(path: &Path) -> Result<(), ArtifactError> {
    let text = path.to_string_lossy();
    if text.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(ArtifactError::Sink("runpack path exceeds limit".to_string()));
    }
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if value.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(ArtifactError::Sink("runpack path component too long".to_string()));
        }
    }
    Ok(())
}

/// Ensure a path is relative and does not escape the runpack root.
fn ensure_relative_path(candidate: &Path) -> Result<(), ArtifactError> {
    if candidate.is_absolute() {
        return Err(ArtifactError::Sink("absolute artifact path not allowed".to_string()));
    }
    for component in candidate.components() {
        match component {
            Component::ParentDir => {
                return Err(ArtifactError::Sink("artifact path escapes runpack root".to_string()));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(ArtifactError::Sink("absolute artifact path not allowed".to_string()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}
