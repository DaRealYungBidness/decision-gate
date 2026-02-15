// crates/decision-gate-contract/src/contract.rs
// ============================================================================
// Module: Contract Builder
// Description: Generator for Decision Gate contract artifacts.
// Purpose: Assemble deterministic contract outputs and write them to disk.
// Dependencies: decision-gate-config, decision-gate-core, serde, serde_jcs, serde_json, std
// ============================================================================

//! ## Overview
//! The contract builder assembles the canonical Decision Gate contract bundle
//! and writes it into `Docs/generated/decision-gate`. It enforces deterministic
//! output ordering and emits human-readable JSON with canonical key ordering,
//! hashing every artifact for integrity.
//! Security posture: outputs are consumed by external tooling; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use cap_primitives::fs::FollowSymlinks;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use cap_std::fs::OpenOptions;
use decision_gate_config as config;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use serde::Serialize;
use serde_jcs;
use serde_json;

use crate::ContractError;
use crate::authoring;
use crate::examples;
use crate::providers;
use crate::schemas;
use crate::tooling;
use crate::tooltips;
use crate::types::ContractArtifact;
use crate::types::ContractBundle;
use crate::types::ContractManifest;
use crate::types::ManifestArtifact;

// ============================================================================
// SECTION: Contract Builder
// ============================================================================

/// Builder for Decision Gate contract artifacts.
///
/// # Invariants
/// - `output_dir` is treated as a trusted root; artifact paths are validated as safe, relative
///   paths before writes occur.
/// - When used via [`ContractBuilder::build`], artifacts are deterministic and ordered by their
///   relative path.
#[derive(Debug, Clone)]
pub struct ContractBuilder {
    /// Output directory for generated artifacts.
    output_dir: PathBuf,
    /// Contract version identifier.
    contract_version: String,
    /// Hash algorithm used for artifact digests.
    hash_algorithm: HashAlgorithm,
}

impl ContractBuilder {
    /// Creates a new contract builder targeting the provided output directory.
    #[must_use]
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            contract_version: env!("CARGO_PKG_VERSION").to_string(),
            hash_algorithm: DEFAULT_HASH_ALGORITHM,
        }
    }

    /// Returns the default output directory for generated artifacts.
    #[must_use]
    pub fn default_output_dir() -> PathBuf {
        PathBuf::from("Docs/generated/decision-gate")
    }

    /// Builds the contract bundle without writing to disk.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when contract generation fails.
    pub fn build(&self) -> Result<ContractBundle, ContractError> {
        let tool_contracts = tooling::tool_contracts();
        let provider_contracts = providers::provider_contracts();
        let mut artifacts = vec![
            markdown_artifact("authoring.md", authoring::authoring_markdown()),
            markdown_artifact("glossary.md", tooltips::tooltips_glossary_markdown()),
            json_artifact("tooling.json", &tool_contracts)?,
            markdown_artifact("tooling.md", tooling::tooling_markdown(&tool_contracts)),
            json_artifact("tooltips.json", &tooltips::tooltips_manifest())?,
            json_artifact("providers.json", &provider_contracts)?,
            markdown_artifact("providers.md", providers::providers_markdown(&provider_contracts)),
            json_artifact("schemas/scenario.schema.json", &schemas::scenario_schema())?,
            json_artifact("schemas/config.schema.json", &config::config_schema())?,
            pretty_json_artifact("examples/scenario.json", &examples::scenario_example())?,
            text_artifact(
                "examples/scenario.ron",
                examples::scenario_example_ron()
                    .map_err(|err| ContractError::Serialization(err.to_string()))?,
                "text/plain",
            ),
            pretty_json_artifact("examples/run-config.json", &examples::run_config_example())?,
            text_artifact(
                "examples/decision-gate.toml",
                config::config_toml_example(),
                "application/toml",
            ),
        ];

        artifacts.sort_by(|lhs, rhs| lhs.path.cmp(&rhs.path));
        ensure_unique_paths(&artifacts)?;

        let manifest = build_manifest(&self.contract_version, self.hash_algorithm, &artifacts);

        Ok(ContractBundle {
            manifest,
            artifacts,
        })
    }

    /// Writes the contract bundle to the configured output directory.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when writing fails.
    pub fn write(&self) -> Result<ContractManifest, ContractError> {
        self.write_to(&self.output_dir)
    }

    /// Writes the contract bundle to the specified output directory.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when writing fails.
    pub fn write_to(&self, output_dir: &Path) -> Result<ContractManifest, ContractError> {
        let bundle = self.build()?;
        let output = open_output_dir(output_dir, true)?;
        for artifact in &bundle.artifacts {
            write_artifact(&output, artifact)?;
        }
        let manifest_bytes = serialize_json_pretty(&bundle.manifest)?;
        write_artifact_bytes(&output, Path::new("index.json"), &manifest_bytes)?;
        Ok(bundle.manifest)
    }

    /// Verifies the on-disk contract bundle matches the generated bundle.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when verification fails.
    pub fn verify_output(&self, output_dir: &Path) -> Result<(), ContractError> {
        let bundle = self.build()?;
        let output = open_output_dir(output_dir, false)?;
        let expected_files = expected_paths(&bundle);
        for artifact in &bundle.artifacts {
            let relative = validate_relative_path(&artifact.path)?;
            let bytes = read_expected_bytes(&output, &relative, artifact.bytes.len())?;
            if bytes != artifact.bytes {
                return Err(ContractError::Generation(format!(
                    "artifact mismatch: {}",
                    artifact.path
                )));
            }
        }
        let manifest_bytes = serialize_json_pretty(&bundle.manifest)?;
        let actual_manifest =
            read_expected_bytes(&output, Path::new("index.json"), manifest_bytes.len())?;
        if actual_manifest != manifest_bytes {
            return Err(ContractError::Generation(String::from("manifest mismatch: index.json")));
        }
        let actual_files = collect_output_files(&output)?;
        for path in actual_files {
            if !expected_files.contains(&path) {
                return Err(ContractError::Generation(format!("unexpected artifact: {path}")));
            }
        }
        Ok(())
    }
}

impl Default for ContractBuilder {
    fn default() -> Self {
        Self::new(Self::default_output_dir())
    }
}

// ============================================================================
// SECTION: Artifact Helpers
// ============================================================================

/// Builds a JSON artifact with deterministic, pretty-printed serialization.
fn json_artifact<T: Serialize>(path: &str, value: &T) -> Result<ContractArtifact, ContractError> {
    let bytes = serialize_json_pretty(value)?;
    Ok(ContractArtifact {
        path: path.to_string(),
        content_type: String::from("application/json"),
        bytes,
    })
}

/// Builds a JSON artifact with deterministic, pretty formatting for display.
fn pretty_json_artifact<T: Serialize>(
    path: &str,
    value: &T,
) -> Result<ContractArtifact, ContractError> {
    let bytes = serialize_json_pretty(value)?;
    Ok(ContractArtifact {
        path: path.to_string(),
        content_type: String::from("application/json"),
        bytes,
    })
}

/// Builds a markdown artifact from content.
fn markdown_artifact(path: &str, content: String) -> ContractArtifact {
    text_artifact(path, content, "text/markdown")
}

/// Builds a text artifact from content.
fn text_artifact(path: &str, content: String, content_type: &str) -> ContractArtifact {
    ContractArtifact {
        path: path.to_string(),
        content_type: content_type.to_string(),
        bytes: content.into_bytes(),
    }
}

/// Serializes a value into canonical JSON bytes for deterministic ordering.
fn serialize_json_canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, ContractError> {
    serde_jcs::to_vec(value).map_err(|err| ContractError::Serialization(err.to_string()))
}

/// Serializes a value into pretty JSON bytes with canonical key ordering.
fn serialize_json_pretty<T: Serialize>(value: &T) -> Result<Vec<u8>, ContractError> {
    let canonical = serialize_json_canonical(value)?;
    let canonical_value: serde_json::Value = serde_json::from_slice(&canonical)
        .map_err(|err| ContractError::Serialization(err.to_string()))?;
    let mut bytes = serde_json::to_vec_pretty(&canonical_value)
        .map_err(|err| ContractError::Serialization(err.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Builds the manifest from generated artifacts.
fn build_manifest(
    contract_version: &str,
    algorithm: HashAlgorithm,
    artifacts: &[ContractArtifact],
) -> ContractManifest {
    let mut entries = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let digest = hash_bytes(algorithm, &artifact.bytes);
        entries.push(ManifestArtifact {
            path: artifact.path.clone(),
            content_type: artifact.content_type.clone(),
            digest,
        });
    }
    ContractManifest {
        contract_version: contract_version.to_string(),
        hash_algorithm: algorithm,
        artifacts: entries,
    }
}

/// Ensures artifact paths are unique.
fn ensure_unique_paths(artifacts: &[ContractArtifact]) -> Result<(), ContractError> {
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        if !seen.insert(&artifact.path) {
            return Err(ContractError::Generation(format!(
                "duplicate artifact path: {}",
                artifact.path
            )));
        }
    }
    Ok(())
}

/// Opens the output directory as a capability handle.
///
/// # Errors
///
/// Returns [`ContractError`] when the path is invalid, unsafe, or inaccessible.
fn open_output_dir(output_dir: &Path, create_missing: bool) -> Result<Dir, ContractError> {
    if output_dir.as_os_str().is_empty() {
        return Err(ContractError::OutputPath(output_dir.to_path_buf()));
    }
    let normalized = normalize_output_dir(output_dir)?;
    let (anchor, components) = split_anchor_and_components(&normalized)?;
    if components.is_empty() {
        return Err(ContractError::OutputPath(normalized));
    }
    let mut current = Dir::open_ambient_dir(&anchor, ambient_authority())
        .map_err(|err| ContractError::Io(err.to_string()))?;
    for component in components {
        current = open_or_create_child_dir_nofollow(
            &current,
            Path::new(component.as_os_str()),
            create_missing,
        )
        .map_err(|err| map_open_error(&err, output_dir))?;
    }
    Ok(current)
}

/// Normalizes an output directory into an absolute path.
///
/// # Errors
///
/// Returns [`ContractError`] when the current directory cannot be resolved.
fn normalize_output_dir(output_dir: &Path) -> Result<PathBuf, ContractError> {
    if output_dir.is_absolute() {
        return Ok(output_dir.to_path_buf());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(output_dir))
        .map_err(|err| ContractError::Io(err.to_string()))
}

/// Splits an absolute path into an anchor root and normal child components.
///
/// # Errors
///
/// Returns [`ContractError`] when the path contains parent traversal components.
fn split_anchor_and_components(path: &Path) -> Result<(PathBuf, Vec<OsString>), ContractError> {
    let mut anchor = PathBuf::new();
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => anchor.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(value) => components.push(value.to_os_string()),
            Component::ParentDir => return Err(ContractError::OutputPath(path.to_path_buf())),
        }
    }
    if anchor.as_os_str().is_empty() {
        return Err(ContractError::OutputPath(path.to_path_buf()));
    }
    Ok((anchor, components))
}

/// Opens a child directory without following symlinks.
fn open_child_dir_nofollow(parent: &Dir, child: &Path) -> std::io::Result<Dir> {
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let file = parent.open_with(child, &options)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "path component is not a directory",
        ));
    }
    Ok(Dir::from_std_file(file.into_std()))
}

/// Opens or creates a child directory without following symlinks.
fn open_or_create_child_dir_nofollow(
    parent: &Dir,
    child: &Path,
    create_missing: bool,
) -> std::io::Result<Dir> {
    match open_child_dir_nofollow(parent, child) {
        Ok(dir) => Ok(dir),
        Err(err) if err.kind() == ErrorKind::NotFound && create_missing => {
            parent.create_dir(child)?;
            open_child_dir_nofollow(parent, child)
        }
        Err(err) => Err(err),
    }
}

/// Maps low-level open errors into contract-level path errors.
fn map_open_error(err: &std::io::Error, path: &Path) -> ContractError {
    if matches!(
        err.kind(),
        ErrorKind::NotFound
            | ErrorKind::InvalidInput
            | ErrorKind::PermissionDenied
            | ErrorKind::NotADirectory
            | ErrorKind::Unsupported
    ) {
        return ContractError::OutputPath(path.to_path_buf());
    }
    #[cfg(unix)]
    if err.raw_os_error() == Some(40) {
        return ContractError::OutputPath(path.to_path_buf());
    }
    #[cfg(windows)]
    if matches!(err.raw_os_error(), Some(681) | Some(1920)) {
        return ContractError::OutputPath(path.to_path_buf());
    }
    ContractError::Io(err.to_string())
}

/// Writes a single artifact to the output directory.
fn write_artifact(output_dir: &Dir, artifact: &ContractArtifact) -> Result<(), ContractError> {
    let relative = validate_relative_path(&artifact.path)?;
    write_artifact_bytes(output_dir, &relative, &artifact.bytes)
}

/// Writes bytes to a relative path using no-follow and atomic rename semantics.
fn write_artifact_bytes(
    output_dir: &Dir,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), ContractError> {
    let (parent_dir, file_name, file_path) = open_parent_dir(output_dir, relative, true)?;
    write_file_atomic(&parent_dir, Path::new(file_name.as_os_str()), &file_path, bytes)
}

/// Opens the parent directory for a relative artifact path.
fn open_parent_dir(
    output_dir: &Dir,
    relative: &Path,
    create_missing: bool,
) -> Result<(Dir, OsString, PathBuf), ContractError> {
    let mut current = output_dir.try_clone().map_err(|err| ContractError::Io(err.to_string()))?;
    let mut parent = PathBuf::new();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(ContractError::OutputPath(relative.to_path_buf()));
        };
        if components.peek().is_none() {
            let file_name = name.to_os_string();
            let file_path = if parent.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                parent.join(&file_name)
            };
            return Ok((current, file_name, file_path));
        }
        parent.push(name);
        current = open_or_create_child_dir_nofollow(&current, Path::new(name), create_missing)
            .map_err(|err| map_open_error(&err, relative))?;
    }
    Err(ContractError::OutputPath(relative.to_path_buf()))
}

/// Writes file bytes using a temporary sibling and atomic rename.
fn write_file_atomic(
    parent: &Dir,
    file_name: &Path,
    file_path: &Path,
    bytes: &[u8],
) -> Result<(), ContractError> {
    for attempt in 0 .. 64_u32 {
        let temp_name = temp_file_name(file_name, attempt)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        options._cap_fs_ext_follow(FollowSymlinks::No);
        match parent.open_with(&temp_name, &options) {
            Ok(mut temp_file) => {
                if let Err(err) = temp_file.write_all(bytes) {
                    let _ = parent.remove_file(&temp_name);
                    return Err(ContractError::Io(err.to_string()));
                }
                if let Err(err) = temp_file.sync_all() {
                    let _ = parent.remove_file(&temp_name);
                    return Err(ContractError::Io(err.to_string()));
                }
                if let Err(err) = parent.rename(&temp_name, parent, file_name) {
                    let _ = parent.remove_file(&temp_name);
                    return Err(ContractError::Io(err.to_string()));
                }
                return Ok(());
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => return Err(map_open_error(&err, file_path)),
        }
    }
    Err(ContractError::Generation("unable to allocate temporary output file".to_string()))
}

/// Builds a deterministic temporary file name for atomic writes.
fn temp_file_name(file_name: &Path, attempt: u32) -> Result<PathBuf, ContractError> {
    let Some(base_name) = file_name.file_name() else {
        return Err(ContractError::OutputPath(file_name.to_path_buf()));
    };
    let mut temp = OsString::from(".tmp-");
    temp.push(base_name);
    temp.push(format!(".{}.{}", std::process::id(), attempt));
    Ok(PathBuf::from(temp))
}

/// Validates that the artifact path is relative and safe.
fn validate_relative_path(path: &str) -> Result<PathBuf, ContractError> {
    if path.trim().is_empty() {
        return Err(ContractError::Generation(String::from("artifact path is empty")));
    }
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return Err(ContractError::Generation(format!("artifact path must be relative: {path}")));
    }
    for component in candidate.components() {
        if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err(ContractError::Generation(format!(
                "artifact path contains invalid component: {path}"
            )));
        }
    }
    Ok(candidate)
}

/// Reads a file and verifies its length matches the expected size.
fn read_expected_bytes(
    output_dir: &Dir,
    relative: &Path,
    expected_len: usize,
) -> Result<Vec<u8>, ContractError> {
    let (parent_dir, file_name, file_path) = open_parent_dir(output_dir, relative, false)?;
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let mut file = parent_dir
        .open_with(Path::new(file_name.as_os_str()), &options)
        .map_err(|err| map_open_error(&err, &file_path))?;
    let metadata = file.metadata().map_err(|err| ContractError::Io(err.to_string()))?;
    if !metadata.is_file() {
        return Err(ContractError::OutputPath(file_path));
    }
    let expected_len = u64::try_from(expected_len).map_err(|_| {
        ContractError::Generation(String::from("expected length exceeds addressable size"))
    })?;
    if metadata.len() != expected_len {
        return Err(ContractError::Generation(format!(
            "artifact size mismatch: {}",
            relative.display()
        )));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|err| ContractError::Io(err.to_string()))?;
    Ok(bytes)
}

/// Collects the expected output paths for verification.
fn expected_paths(bundle: &ContractBundle) -> BTreeSet<String> {
    let mut expected = BTreeSet::new();
    expected.insert(String::from("index.json"));
    for artifact in &bundle.artifacts {
        expected.insert(artifact.path.clone());
    }
    expected
}

/// Recursively collects file paths under the output directory.
fn collect_output_files(output_dir: &Dir) -> Result<BTreeSet<String>, ContractError> {
    let mut files = BTreeSet::new();
    collect_files_recursive(output_dir, Path::new(""), &mut files)?;
    Ok(files)
}

/// Recursively collects file paths relative to the root directory.
fn collect_files_recursive(
    current: &Dir,
    prefix: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), ContractError> {
    let entries = current.entries().map_err(|err| ContractError::Io(err.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|err| ContractError::Io(err.to_string()))?;
        let file_name = entry.file_name();
        let relative = if prefix.as_os_str().is_empty() {
            PathBuf::from(&file_name)
        } else {
            prefix.join(&file_name)
        };
        let file_type = entry.file_type().map_err(|err| ContractError::Io(err.to_string()))?;
        if file_type.is_symlink() {
            return Err(ContractError::OutputPath(relative));
        }
        if file_type.is_dir() {
            let directory = entry.open_dir().map_err(|err| ContractError::Io(err.to_string()))?;
            collect_files_recursive(&directory, &relative, files)?;
        } else if file_type.is_file() {
            let text =
                relative.to_str().ok_or_else(|| ContractError::OutputPath(relative.clone()))?;
            let normalized = text.replace('\\', "/");
            files.insert(normalized);
        }
    }
    Ok(())
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests;
