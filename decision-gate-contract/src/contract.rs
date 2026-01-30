// decision-gate-contract/src/contract.rs
// ============================================================================
// Module: Contract Builder
// Description: Generator for Decision Gate contract artifacts.
// Purpose: Assemble deterministic contract outputs and write them to disk.
// Dependencies: decision-gate-core, serde_jcs, std
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
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

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
        ensure_output_dir(output_dir)?;
        for artifact in &bundle.artifacts {
            write_artifact(output_dir, artifact)?;
        }
        let manifest_path = output_dir.join("index.json");
        let manifest_bytes = serialize_json_pretty(&bundle.manifest)?;
        fs::write(&manifest_path, &manifest_bytes)
            .map_err(|err| ContractError::Io(err.to_string()))?;
        Ok(bundle.manifest)
    }

    /// Verifies the on-disk contract bundle matches the generated bundle.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when verification fails.
    pub fn verify_output(&self, output_dir: &Path) -> Result<(), ContractError> {
        let bundle = self.build()?;
        ensure_existing_output_dir(output_dir)?;
        let expected_files = expected_paths(&bundle);
        for artifact in &bundle.artifacts {
            let path = output_dir.join(&artifact.path);
            let bytes = fs::read(&path).map_err(|err| ContractError::Io(err.to_string()))?;
            if bytes != artifact.bytes {
                return Err(ContractError::Generation(format!(
                    "artifact mismatch: {}",
                    artifact.path
                )));
            }
        }
        let manifest_bytes = serialize_json_pretty(&bundle.manifest)?;
        let manifest_path = output_dir.join("index.json");
        let actual_manifest =
            fs::read(&manifest_path).map_err(|err| ContractError::Io(err.to_string()))?;
        if actual_manifest != manifest_bytes {
            return Err(ContractError::Generation(String::from("manifest mismatch: index.json")));
        }
        let actual_files = collect_output_files(output_dir)?;
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

/// Ensures the output directory exists (creating it if necessary).
fn ensure_output_dir(output_dir: &Path) -> Result<(), ContractError> {
    if output_dir.as_os_str().is_empty() {
        return Err(ContractError::OutputPath(output_dir.to_path_buf()));
    }
    if output_dir.exists() {
        if !output_dir.is_dir() {
            return Err(ContractError::OutputPath(output_dir.to_path_buf()));
        }
        return Ok(());
    }
    fs::create_dir_all(output_dir).map_err(|err| ContractError::Io(err.to_string()))
}

/// Ensures the output directory exists and is a directory.
fn ensure_existing_output_dir(output_dir: &Path) -> Result<(), ContractError> {
    if !output_dir.is_dir() {
        return Err(ContractError::OutputPath(output_dir.to_path_buf()));
    }
    Ok(())
}

/// Writes a single artifact to the output directory.
fn write_artifact(output_dir: &Path, artifact: &ContractArtifact) -> Result<(), ContractError> {
    let relative = validate_relative_path(&artifact.path)?;
    let target = output_dir.join(&relative);
    let parent = target.parent().ok_or_else(|| ContractError::OutputPath(target.clone()))?;
    fs::create_dir_all(parent).map_err(|err| ContractError::Io(err.to_string()))?;
    fs::write(&target, &artifact.bytes).map_err(|err| ContractError::Io(err.to_string()))
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
fn collect_output_files(output_dir: &Path) -> Result<BTreeSet<String>, ContractError> {
    let mut files = BTreeSet::new();
    collect_files_recursive(output_dir, output_dir, &mut files)?;
    Ok(files)
}

/// Recursively collects file paths relative to the root directory.
fn collect_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), ContractError> {
    let entries = fs::read_dir(current).map_err(|err| ContractError::Io(err.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|err| ContractError::Io(err.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, files)?;
        } else if path.is_file() {
            let relative =
                path.strip_prefix(root).map_err(|_| ContractError::OutputPath(path.clone()))?;
            let text = relative
                .to_str()
                .ok_or_else(|| ContractError::OutputPath(relative.to_path_buf()))?;
            let normalized = text.replace('\\', "/");
            files.insert(normalized);
        }
    }
    Ok(())
}
