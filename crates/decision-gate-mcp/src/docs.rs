// crates/decision-gate-mcp/src/docs.rs
// ============================================================================
// Module: MCP Documentation Registry
// Description: Embedded documentation registry and search helpers for MCP tools
// Purpose: Serve documentation sections to MCP callers without runtime I/O
// Dependencies: serde, decision-gate-config
// ============================================================================

//! ## Overview
//!
//! Provides a deterministic documentation catalog for MCP callers. Default
//! documents are embedded at compile time; optional extra docs may be loaded
//! from local paths during server startup. Search uses heading-first lexical
//! matching with role-aware tie-breaking and stable ordering.
//! Security posture: docs input is untrusted; enforce size/path limits; see
//! `Docs/security/threat_model.md`.
//!
//! ## Layer Responsibilities
//!
//! - Maintain a catalog of documentation blobs for MCP callers.
//! - Convert Markdown into addressable sections keyed by headings.
//! - Perform deterministic search with bounded results.
//!
//! ## Invariants
//!
//! - Default docs are embedded with `include_str!`; no runtime network I/O.
//! - Search ordering is deterministic across identical inputs.
//! - Section boundaries are `##` / `###` headings; intro uses document title.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::config::DocsConfig;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Default maximum number of sections returned when no limit is provided.
const DEFAULT_MAX_SECTIONS: u32 = 3;
/// Hard cap on sections returned to keep responses bounded.
const ABSOLUTE_MAX_SECTIONS: u32 = 10;
/// MIME type used for embedded Markdown resources.
const DOC_MIME_TYPE: &str = "text/markdown";
/// Prefix for Decision Gate docs resource URIs.
pub const RESOURCE_URI_PREFIX: &str = "decision-gate://docs/";

/// Embedded Evidence Flow + Execution Model guide.
pub const EVIDENCE_FLOW_AND_EXECUTION_MODEL: &str =
    include_str!("../../../Docs/guides/evidence_flow_and_execution_model.md");
/// Embedded Security Guide.
pub const SECURITY_GUIDE: &str = include_str!("../../../Docs/guides/security_guide.md");
/// Embedded Tooling Summary (generated).
pub const TOOLING_SUMMARY: &str = include_str!("../../../Docs/generated/decision-gate/tooling.md");
/// Embedded Authoring Formats (generated).
pub const AUTHORING_FORMATS: &str =
    include_str!("../../../Docs/generated/decision-gate/authoring.md");
/// Embedded Condition Authoring guide.
pub const CONDITION_AUTHORING: &str = include_str!("../../../Docs/guides/condition_authoring.md");
/// Embedded RET Logic guide.
pub const RET_LOGIC: &str = include_str!("../../../Docs/guides/ret_logic.md");
/// Embedded LLM-Native Playbook.
pub const LLM_NATIVE_PLAYBOOK: &str = include_str!("../../../Docs/guides/llm_native_playbook.md");
/// Embedded Built-in Providers (generated).
pub const PROVIDERS_SUMMARY: &str =
    include_str!("../../../Docs/generated/decision-gate/providers.md");
/// Embedded Provider Protocol guide.
pub const PROVIDER_PROTOCOL: &str = include_str!("../../../Docs/guides/provider_protocol.md");
/// Embedded Provider Schema Authoring guide.
pub const PROVIDER_SCHEMA_AUTHORING: &str =
    include_str!("../../../Docs/guides/provider_schema_authoring.md");
/// Embedded Provider Development guide.
pub const PROVIDER_DEVELOPMENT: &str = include_str!("../../../Docs/guides/provider_development.md");
/// Embedded JSON Evidence Playbook.
pub const JSON_EVIDENCE_PLAYBOOK: &str =
    include_str!("../../../Docs/guides/json_evidence_playbook.md");
/// Embedded Runpack Architecture reference.
pub const RUNPACK_ARCHITECTURE: &str =
    include_str!("../../../Docs/architecture/decision_gate_runpack_architecture.md");

/// Preferred role ordering for overview responses.
const OVERVIEW_ORDER: &[DocRole] =
    &[DocRole::Reasoning, DocRole::Decision, DocRole::Ontology, DocRole::Pattern];

// ============================================================================
// SECTION: Types
// ============================================================================

/// High-level role for a canonical document.
///
/// # Invariants
/// - Variants are stable for ordering and serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DocRole {
    /// Conceptual layer.
    Reasoning,
    /// Tool selection and decision layer.
    Decision,
    /// Schema and ontology layer.
    Ontology,
    /// Pattern / recipe layer.
    Pattern,
}

impl DocRole {
    /// Stable ordering used for overview mode and deterministic tie-breaking.
    #[must_use]
    pub const fn order(self) -> usize {
        match self {
            Self::Reasoning => 0,
            Self::Decision => 1,
            Self::Ontology => 2,
            Self::Pattern => 3,
        }
    }
}

/// Registry entry describing a document.
///
/// # Invariants
/// - `id` values are unique within a [`DocsCatalog`].
/// - `resource_uri` values use the [`RESOURCE_URI_PREFIX`].
#[derive(Debug, Clone)]
pub struct DocEntry {
    /// Stable identifier for the document.
    pub id: String,
    /// Human-readable title for listings.
    pub title: String,
    /// Raw Markdown body.
    pub body: String,
    /// Canonical role for search weighting and display.
    pub role: DocRole,
    /// Resource URI for MCP resource listing.
    pub resource_uri: String,
    /// Short description for MCP resource listing.
    pub resource_description: String,
}

/// Searchable slice of a document corresponding to a Markdown section.
///
/// # Invariants
/// - Values are derived from a [`DocEntry`] and are treated as read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocSection {
    /// Document identifier.
    pub doc_id: String,
    /// Document title.
    pub doc_title: String,
    /// Document role.
    pub doc_role: DocRole,
    /// Section heading text.
    pub heading: String,
    /// Section body content (raw Markdown).
    pub content: String,
}

/// Search result section with rank applied.
///
/// # Invariants
/// - `rank` is zero-based within the returned set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchSection {
    /// Rank (0-based) within the returned set.
    pub rank: usize,
    /// Document identifier.
    pub doc_id: String,
    /// Document title.
    pub doc_title: String,
    /// Document role.
    pub doc_role: DocRole,
    /// Section heading text.
    pub heading: String,
    /// Section body content (raw Markdown).
    pub content: String,
}

/// Unique document coverage details in a result set.
///
/// # Invariants
/// - Each entry refers to a unique document identifier in the response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocCoverage {
    /// Document identifier.
    pub doc_id: String,
    /// Document title.
    pub doc_title: String,
    /// Document role.
    pub doc_role: DocRole,
}

/// Structured search response surfaced by docs search.
///
/// # Invariants
/// - `sections` are ordered by deterministic ranking rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    /// Ranked sections matching the query.
    pub sections: Vec<SearchSection>,
    /// Set of documents represented in `sections`.
    pub docs_covered: Vec<DocCoverage>,
    /// Role-aware follow-up suggestions.
    pub suggested_followups: Vec<String>,
}

/// Request payload for documentation search.
///
/// # Invariants
/// - `max_sections` is clamped to server limits before use.
#[derive(Debug, Clone, Deserialize)]
pub struct DocsSearchRequest {
    /// Natural-language query for section retrieval.
    pub query: String,
    /// Maximum number of sections to return (defaults to 3, capped at 10).
    #[serde(default)]
    pub max_sections: Option<u32>,
}

/// Errors raised when loading the docs catalog.
///
/// # Invariants
/// - Variants are stable for catalog error classification.
#[derive(Debug, thiserror::Error)]
pub enum DocsCatalogError {
    /// IO error while reading docs from disk.
    #[error("docs catalog io error: {0}")]
    Io(String),
    /// Invalid configuration or catalog limits.
    #[error("docs catalog invalid: {0}")]
    Invalid(String),
}

/// Runtime documentation catalog.
///
/// # Invariants
/// - `docs` entries have unique identifiers.
/// - `max_sections` is clamped to [`ABSOLUTE_MAX_SECTIONS`].
#[derive(Debug, Clone)]
pub struct DocsCatalog {
    /// Embedded and ingested documentation entries.
    docs: Vec<DocEntry>,
    /// Maximum sections returned by search.
    max_sections: u32,
    /// Non-fatal warnings encountered during ingestion.
    warnings: Vec<String>,
}

impl DocsCatalog {
    /// Builds a catalog from configuration, including embedded docs.
    ///
    /// # Errors
    ///
    /// Returns [`DocsCatalogError`] on missing paths or invalid doc ingestion.
    pub fn from_config(config: &DocsConfig) -> Result<Self, DocsCatalogError> {
        let max_sections = config.max_sections.min(ABSOLUTE_MAX_SECTIONS);
        if !config.enabled {
            return Ok(Self {
                docs: Vec::new(),
                max_sections,
                warnings: Vec::new(),
            });
        }

        let mut warnings = Vec::new();
        let mut docs = if config.include_default_docs { default_docs() } else { Vec::new() };

        if docs.iter().any(|doc| doc.body.len() > config.max_doc_bytes) {
            return Err(DocsCatalogError::Invalid(
                "docs.max_doc_bytes too small for default corpus".to_string(),
            ));
        }
        let mut total_bytes = docs.iter().map(|doc| doc.body.len()).sum::<usize>();
        if docs.len() > config.max_docs {
            return Err(DocsCatalogError::Invalid(
                "docs.max_docs too small for default corpus".to_string(),
            ));
        }
        if total_bytes > config.max_total_bytes {
            return Err(DocsCatalogError::Invalid(
                "docs.max_total_bytes too small for default corpus".to_string(),
            ));
        }

        let remaining_docs = config.max_docs.saturating_sub(docs.len());
        let remaining_bytes = config.max_total_bytes.saturating_sub(total_bytes);
        let extra_docs = load_extra_docs(config, &mut warnings, remaining_docs, remaining_bytes)?;
        for doc in extra_docs {
            if docs.len() >= config.max_docs {
                warnings.push("docs catalog max_docs reached; skipping extra doc".to_string());
                break;
            }
            if doc.body.len() > config.max_doc_bytes {
                warnings.push(format!("docs entry '{}' exceeds max_doc_bytes; skipping", doc.id));
                continue;
            }
            if total_bytes + doc.body.len() > config.max_total_bytes {
                warnings.push(format!("docs entry '{}' exceeds max_total_bytes; skipping", doc.id));
                continue;
            }
            total_bytes += doc.body.len();
            docs.push(doc);
        }

        Ok(Self {
            docs,
            max_sections,
            warnings,
        })
    }

    /// Builds a catalog from pre-loaded entries.
    #[must_use]
    pub fn from_entries(entries: Vec<DocEntry>, max_sections: u32) -> Self {
        let max_sections = max_sections.clamp(1, ABSOLUTE_MAX_SECTIONS);
        Self {
            docs: entries,
            max_sections,
            warnings: Vec::new(),
        }
    }

    /// Returns warnings emitted during catalog construction.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Returns the docs catalog.
    #[must_use]
    pub fn docs(&self) -> &[DocEntry] {
        &self.docs
    }

    /// Returns true when docs search is enabled for this catalog.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Searches documentation sections using a heading/body heuristic.
    #[must_use]
    pub fn search(&self, request: &DocsSearchRequest) -> SearchResult {
        let limit =
            request.max_sections.unwrap_or(DEFAULT_MAX_SECTIONS).clamp(1, self.max_sections);
        let normalized_query = request.query.trim();
        if normalized_query.is_empty() {
            return overview_result(self.docs(), limit);
        }

        search_sections(self.docs(), normalized_query, limit)
    }
}

// ============================================================================
// SECTION: Default Catalog
// ============================================================================

#[derive(Debug, Clone, Copy)]
/// Static doc seed used for embedded defaults.
struct DocSeed {
    /// Stable identifier for the document.
    id: &'static str,
    /// Display title for the document.
    title: &'static str,
    /// Raw Markdown body.
    body: &'static str,
    /// Role used for deterministic ranking.
    role: DocRole,
    /// Resource URI exposed via MCP resources.
    resource_uri: &'static str,
    /// Short resource description.
    resource_description: &'static str,
}

/// Embedded default docs catalog.
const DEFAULT_DOCS: [DocSeed; 13] = [
    DocSeed {
        id: "evidence_flow_and_execution_model",
        title: "Evidence Flow + Execution Model",
        body: EVIDENCE_FLOW_AND_EXECUTION_MODEL,
        role: DocRole::Reasoning,
        resource_uri: "decision-gate://docs/evidence-flow",
        resource_description: "Evidence flow, trust lanes, and runtime evaluation semantics.",
    },
    DocSeed {
        id: "security_guide",
        title: "Security Guide",
        body: SECURITY_GUIDE,
        role: DocRole::Reasoning,
        resource_uri: "decision-gate://docs/security",
        resource_description: "Security posture, disclosure policy, and fail-closed defaults.",
    },
    DocSeed {
        id: "tooling_summary",
        title: "Decision Gate MCP Tools",
        body: TOOLING_SUMMARY,
        role: DocRole::Decision,
        resource_uri: "decision-gate://docs/tooling",
        resource_description: "Tool surface summary and usage notes.",
    },
    DocSeed {
        id: "authoring_formats",
        title: "Decision Gate Authoring Formats",
        body: AUTHORING_FORMATS,
        role: DocRole::Ontology,
        resource_uri: "decision-gate://docs/authoring",
        resource_description: "Canonical JSON and authoring format rules.",
    },
    DocSeed {
        id: "condition_authoring",
        title: "Condition Authoring Cookbook",
        body: CONDITION_AUTHORING,
        role: DocRole::Ontology,
        resource_uri: "decision-gate://docs/conditions",
        resource_description: "Comparator semantics and tri-state rules.",
    },
    DocSeed {
        id: "ret_logic",
        title: "Requirement Evaluation Trees (RET)",
        body: RET_LOGIC,
        role: DocRole::Ontology,
        resource_uri: "decision-gate://docs/ret-logic",
        resource_description: "RET structure, operators, and evaluation rules.",
    },
    DocSeed {
        id: "llm_native_playbook",
        title: "LLM-Native Playbook",
        body: LLM_NATIVE_PLAYBOOK,
        role: DocRole::Decision,
        resource_uri: "decision-gate://docs/llm-playbook",
        resource_description: "LLM-first workflows and MCP call sequences.",
    },
    DocSeed {
        id: "providers_summary",
        title: "Decision Gate Built-in Providers",
        body: PROVIDERS_SUMMARY,
        role: DocRole::Ontology,
        resource_uri: "decision-gate://docs/providers",
        resource_description: "Built-in provider checks and schema details.",
    },
    DocSeed {
        id: "provider_protocol",
        title: "Evidence Provider Protocol",
        body: PROVIDER_PROTOCOL,
        role: DocRole::Pattern,
        resource_uri: "decision-gate://docs/provider-protocol",
        resource_description: "MCP evidence_query protocol contract.",
    },
    DocSeed {
        id: "provider_schema_authoring",
        title: "Provider Schema Authoring Guide",
        body: PROVIDER_SCHEMA_AUTHORING,
        role: DocRole::Pattern,
        resource_uri: "decision-gate://docs/provider-schema",
        resource_description: "How to author provider schemas for custom checks.",
    },
    DocSeed {
        id: "provider_development",
        title: "Provider Development Guide",
        body: PROVIDER_DEVELOPMENT,
        role: DocRole::Pattern,
        resource_uri: "decision-gate://docs/provider-development",
        resource_description: "Implementing MCP evidence providers.",
    },
    DocSeed {
        id: "json_evidence_playbook",
        title: "JSON Evidence Playbook",
        body: JSON_EVIDENCE_PLAYBOOK,
        role: DocRole::Pattern,
        resource_uri: "decision-gate://docs/json-evidence",
        resource_description: "JSON evidence workflows and JSONPath semantics.",
    },
    DocSeed {
        id: "runpack_architecture",
        title: "Decision Gate Runpack Architecture",
        body: RUNPACK_ARCHITECTURE,
        role: DocRole::Reasoning,
        resource_uri: "decision-gate://docs/runpack-architecture",
        resource_description: "Runpack export/verify semantics and integrity guarantees.",
    },
];

/// Builds the default embedded docs catalog.
fn default_docs() -> Vec<DocEntry> {
    DEFAULT_DOCS
        .iter()
        .map(|seed| DocEntry {
            id: seed.id.to_string(),
            title: seed.title.to_string(),
            body: seed.body.to_string(),
            role: seed.role,
            resource_uri: seed.resource_uri.to_string(),
            resource_description: seed.resource_description.to_string(),
        })
        .collect()
}

// ============================================================================
// SECTION: Extra Docs Ingestion
// ============================================================================

/// Loads additional docs from configured paths.
fn load_extra_docs(
    config: &DocsConfig,
    warnings: &mut Vec<String>,
    max_docs: usize,
    max_total_bytes: usize,
) -> Result<Vec<DocEntry>, DocsCatalogError> {
    if config.extra_paths.is_empty() || max_docs == 0 || max_total_bytes == 0 {
        return Ok(Vec::new());
    }
    let mut docs = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut visited_dirs: HashSet<PathBuf> = HashSet::new();
    let mut budget = IngestionBudget::new(max_docs, max_total_bytes);
    let max_doc_bytes = config.max_doc_bytes;

    for entry in &config.extra_paths {
        if budget.docs_full() {
            warnings.push("docs catalog max_docs reached; skipping extra doc".to_string());
            break;
        }
        if budget.bytes_full() {
            warnings.push("docs catalog max_total_bytes reached; skipping extra doc".to_string());
            break;
        }
        let path = PathBuf::from(entry);
        let metadata = fs::symlink_metadata(&path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                DocsCatalogError::Io(format!("docs.extra_paths missing: {}", path.display()))
            } else {
                DocsCatalogError::Io(err.to_string())
            }
        })?;
        if metadata.file_type().is_symlink() {
            warnings.push(format!("docs.extra_paths ignoring symlink: {}", path.display()));
            continue;
        }
        if metadata.is_dir() {
            collect_markdown_files(
                &path,
                &mut docs,
                &mut seen_ids,
                &mut visited_dirs,
                warnings,
                max_doc_bytes,
                &mut budget,
            )?;
        } else {
            if !is_markdown_file(&path) {
                warnings.push(format!(
                    "docs.extra_paths ignoring non-markdown file: {}",
                    path.display()
                ));
                continue;
            }
            if let Some(doc) =
                load_markdown_file(&path, &mut seen_ids, warnings, max_doc_bytes, &mut budget)?
            {
                docs.push(doc);
            }
        }
    }

    Ok(docs)
}

/// Bounded ingestion budget for extra documentation loading.
struct IngestionBudget {
    /// Maximum number of extra docs allowed in this ingestion pass.
    max_docs: usize,
    /// Maximum cumulative bytes allowed for extra docs in this ingestion pass.
    max_total_bytes: usize,
    /// Number of docs accepted so far.
    loaded_docs: usize,
    /// Total bytes accepted so far.
    loaded_bytes: usize,
}

impl IngestionBudget {
    /// Creates a new ingestion budget from doc and byte limits.
    const fn new(max_docs: usize, max_total_bytes: usize) -> Self {
        Self {
            max_docs,
            max_total_bytes,
            loaded_docs: 0,
            loaded_bytes: 0,
        }
    }

    /// Returns true when the document-count budget is exhausted.
    const fn docs_full(&self) -> bool {
        self.loaded_docs >= self.max_docs
    }

    /// Returns true when the byte budget is exhausted.
    const fn bytes_full(&self) -> bool {
        self.loaded_bytes >= self.max_total_bytes
    }

    /// Returns true when a candidate document can fit within the remaining budget.
    const fn can_fit(&self, bytes: usize) -> bool {
        !self.docs_full() && self.loaded_bytes.saturating_add(bytes) <= self.max_total_bytes
    }

    /// Accounts for an accepted document and its byte size.
    const fn account_doc(&mut self, bytes: usize) {
        self.loaded_docs = self.loaded_docs.saturating_add(1);
        self.loaded_bytes = self.loaded_bytes.saturating_add(bytes);
    }
}

/// Recursively collects Markdown files from a directory.
fn collect_markdown_files(
    dir: &Path,
    docs: &mut Vec<DocEntry>,
    seen_ids: &mut HashSet<String>,
    visited_dirs: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    max_doc_bytes: usize,
    budget: &mut IngestionBudget,
) -> Result<(), DocsCatalogError> {
    let canonical = dir.canonicalize().map_err(|err| DocsCatalogError::Io(err.to_string()))?;
    if !visited_dirs.insert(canonical) {
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| DocsCatalogError::Io(err.to_string()))? {
        let entry = entry.map_err(|err| DocsCatalogError::Io(err.to_string()))?;
        let file_type = entry.file_type().map_err(|err| DocsCatalogError::Io(err.to_string()))?;
        entries.push((entry.path(), file_type));
    }
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    for (path, file_type) in entries {
        if budget.docs_full() {
            warnings.push("docs catalog max_docs reached; skipping extra doc".to_string());
            break;
        }
        if budget.bytes_full() {
            warnings.push("docs catalog max_total_bytes reached; skipping extra doc".to_string());
            break;
        }

        if file_type.is_symlink() {
            warnings.push(format!("docs.extra_paths ignoring symlink: {}", path.display()));
            continue;
        }

        if file_type.is_dir() {
            collect_markdown_files(
                &path,
                docs,
                seen_ids,
                visited_dirs,
                warnings,
                max_doc_bytes,
                budget,
            )?;
            continue;
        }
        if !file_type.is_file() || !is_markdown_file(&path) {
            continue;
        }
        if let Some(doc) = load_markdown_file(&path, seen_ids, warnings, max_doc_bytes, budget)? {
            docs.push(doc);
        }
    }
    Ok(())
}

/// Loads a Markdown file into a doc entry, returning None for empty files.
fn load_markdown_file(
    path: &Path,
    seen_ids: &mut HashSet<String>,
    warnings: &mut Vec<String>,
    max_doc_bytes: usize,
    budget: &mut IngestionBudget,
) -> Result<Option<DocEntry>, DocsCatalogError> {
    if budget.docs_full() {
        warnings.push("docs catalog max_docs reached; skipping extra doc".to_string());
        return Ok(None);
    }
    if budget.bytes_full() {
        warnings.push("docs catalog max_total_bytes reached; skipping extra doc".to_string());
        return Ok(None);
    }

    let metadata =
        fs::symlink_metadata(path).map_err(|err| DocsCatalogError::Io(err.to_string()))?;
    if metadata.file_type().is_symlink() {
        warnings.push(format!("docs.extra_paths ignoring symlink: {}", path.display()));
        return Ok(None);
    }
    if !metadata.is_file() {
        warnings.push(format!("docs.extra_paths ignoring non-regular file: {}", path.display()));
        return Ok(None);
    }
    let file_bytes = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if file_bytes > max_doc_bytes {
        let id = normalize_doc_id(path);
        warnings.push(format!("docs entry '{id}' exceeds max_doc_bytes; skipping"));
        return Ok(None);
    }
    if !budget.can_fit(file_bytes) {
        let id = normalize_doc_id(path);
        warnings.push(format!("docs entry '{id}' exceeds max_total_bytes; skipping"));
        return Ok(None);
    }
    let contents = fs::read_to_string(path).map_err(|err| DocsCatalogError::Io(err.to_string()))?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        warnings.push(format!("docs.extra_paths skipping empty file: {}", path.display()));
        return Ok(None);
    }
    if contents.len() > max_doc_bytes {
        let id = normalize_doc_id(path);
        warnings.push(format!("docs entry '{id}' exceeds max_doc_bytes; skipping"));
        return Ok(None);
    }
    if !budget.can_fit(contents.len()) {
        let id = normalize_doc_id(path);
        warnings.push(format!("docs entry '{id}' exceeds max_total_bytes; skipping"));
        return Ok(None);
    }
    let (title, body) = extract_title_and_body(trimmed, path);
    let mut id = normalize_doc_id(path);
    if seen_ids.contains(&id) {
        let mut suffix = 2;
        while seen_ids.contains(&format!("{id}_{suffix}")) {
            suffix += 1;
        }
        id = format!("{id}_{suffix}");
    }
    seen_ids.insert(id.clone());
    budget.account_doc(contents.len());
    Ok(Some(DocEntry {
        id: id.clone(),
        title,
        body: body.to_string(),
        role: DocRole::Pattern,
        resource_uri: format!("{RESOURCE_URI_PREFIX}custom/{id}"),
        resource_description: format!("User-provided document from {}", path.display()),
    }))
}

/// Extracts a title from the first heading or falls back to the file stem.
fn extract_title_and_body<'a>(content: &'a str, path: &Path) -> (String, &'a str) {
    if let Some(first_line) = content.lines().next()
        && let Some(stripped) = first_line.strip_prefix("# ")
    {
        return (stripped.trim().to_string(), content);
    }
    let fallback = path.file_stem().and_then(|s| s.to_str()).unwrap_or("custom-doc");
    (fallback.to_string(), content)
}

/// Normalizes a filesystem path into a stable document identifier.
fn normalize_doc_id(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("custom_doc");
    stem.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '_' })
        .collect()
}

/// Returns true when the file extension indicates Markdown.
fn is_markdown_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

// ============================================================================
// SECTION: Search Implementation
// ============================================================================

/// Searches documentation for the provided query, returning up to `max_sections`.
#[must_use]
fn search_sections(docs: &[DocEntry], query: &str, max_sections: u32) -> SearchResult {
    let normalized_query = query.to_lowercase();
    let capped_limit = max_sections.clamp(1, ABSOLUTE_MAX_SECTIONS);
    let profile = profile_query(&normalized_query);

    let mut sections: Vec<(i32, usize, DocSection)> = docs
        .iter()
        .enumerate()
        .flat_map(|(idx, doc)| {
            split_doc_sections(doc).into_iter().map(move |section| (idx, section))
        })
        .filter_map(|(doc_idx, section)| {
            let lexical = lexical_score(&section, &normalized_query);
            if lexical == 0 {
                return None;
            }
            let role_bonus = role_bonus(section.doc_role, &profile);
            Some((lexical + role_bonus, doc_idx, section))
        })
        .collect();

    sections.sort_by(|(score_a, doc_idx_a, section_a), (score_b, doc_idx_b, section_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| doc_idx_a.cmp(doc_idx_b))
            .then_with(|| section_a.heading.cmp(&section_b.heading))
    });

    let target = sections_limit_to_usize(capped_limit);
    let mut ranked_sections: Vec<SearchSection> = Vec::new();
    for (rank, (_score, _doc_idx, section)) in sections.into_iter().enumerate() {
        if ranked_sections.len() == target {
            break;
        }
        ranked_sections.push(SearchSection {
            rank,
            doc_id: section.doc_id,
            doc_title: section.doc_title,
            doc_role: section.doc_role,
            heading: section.heading,
            content: section.content,
        });
    }

    let docs_covered = coverage_from_sections(&ranked_sections);
    let suggested_followups = suggested_followups(&docs_covered);

    SearchResult {
        sections: ranked_sections,
        docs_covered,
        suggested_followups,
    }
}

/// Builds a search result with overview sections across doc roles.
fn overview_result(docs: &[DocEntry], limit: u32) -> SearchResult {
    let mut sections = Vec::new();
    let mut docs_covered = Vec::new();
    let doc_count = u32::try_from(docs.len()).unwrap_or(u32::MAX);
    let capped_limit = sections_limit_to_usize(limit.min(doc_count));

    for role in OVERVIEW_ORDER {
        if sections.len() == capped_limit {
            break;
        }
        if let Some(doc) = docs.iter().find(|doc| doc.role == *role)
            && let Some(intro) = split_doc_sections(doc).into_iter().next()
        {
            let rank = sections.len();
            sections.push(SearchSection {
                rank,
                doc_id: intro.doc_id,
                doc_title: intro.doc_title,
                doc_role: intro.doc_role,
                heading: intro.heading,
                content: intro.content,
            });
            docs_covered.push(DocCoverage {
                doc_id: doc.id.clone(),
                doc_title: doc.title.clone(),
                doc_role: doc.role,
            });
        }
    }

    SearchResult {
        sections,
        docs_covered,
        suggested_followups: vec![String::from(
            "Refine the query with comparator, provider, or trust keywords to target guidance.",
        )],
    }
}

/// Normalizes the requested section limit into a bounded usize.
fn sections_limit_to_usize(limit: u32) -> usize {
    usize::try_from(limit).unwrap_or_else(|_| {
        usize::try_from(ABSOLUTE_MAX_SECTIONS).map_or(usize::MAX, |value| value)
    })
}

/// Splits a document into heading-based sections.
fn split_doc_sections(doc: &DocEntry) -> Vec<DocSection> {
    let mut sections = Vec::new();
    let mut current_heading = doc.title.clone();
    let mut current_lines: Vec<String> = Vec::new();

    for line in doc.body.lines() {
        if let Some(stripped) = line.strip_prefix("## ") {
            push_section(doc, &current_heading, &current_lines, &mut sections);
            current_heading = stripped.trim().to_string();
            current_lines.clear();
            continue;
        }
        if let Some(stripped) = line.strip_prefix("### ") {
            push_section(doc, &current_heading, &current_lines, &mut sections);
            current_heading = stripped.trim().to_string();
            current_lines.clear();
            continue;
        }

        current_lines.push(line.to_string());
    }

    push_section(doc, &current_heading, &current_lines, &mut sections);
    sections
}

/// Pushes a section when content is present.
fn push_section(doc: &DocEntry, heading: &str, lines: &[String], sections: &mut Vec<DocSection>) {
    let content = lines.join("\n").trim().to_string();
    if content.is_empty() && heading.trim().is_empty() {
        return;
    }

    sections.push(DocSection {
        doc_id: doc.id.clone(),
        doc_title: doc.title.clone(),
        doc_role: doc.role,
        heading: heading.trim().to_string(),
        content,
    });
}

/// Scores a section against the query using lexical heuristics.
fn lexical_score(section: &DocSection, normalized_query: &str) -> i32 {
    let heading_lower = section.heading.to_lowercase();
    let content_lower = section.content.to_lowercase();
    let query_lower = normalized_query.to_lowercase();

    let mut score = 0;
    for term in query_lower.split_whitespace() {
        if term.is_empty() {
            continue;
        }
        if heading_lower.contains(term) {
            score += 3;
        }
        if content_lower.contains(term) {
            score += 1;
        }
    }
    score
}

#[derive(Debug, Clone, Default)]
/// Extracted query intent used to bias role ranking.
struct QueryProfile {
    /// Primary role inferred from the query.
    primary_role: Option<DocRole>,
    /// Secondary roles inferred from the query.
    secondary_roles: Vec<DocRole>,
}

/// Profiles a query to infer role intent.
fn profile_query(normalized_query: &str) -> QueryProfile {
    let mut role_counts: HashMap<DocRole, usize> = HashMap::new();

    let cues: &[(DocRole, &[&str])] = &[
        (DocRole::Ontology, &["schema", "provider", "comparator", "condition", "ret", "jsonpath"]),
        (DocRole::Decision, &["which tool", "precheck", "scenario", "runpack", "start", "next"]),
        (DocRole::Pattern, &["playbook", "workflow", "recipe", "integration", "example"]),
        (DocRole::Reasoning, &["why", "trust", "evidence", "security", "invariant"]),
    ];

    for (role, keywords) in cues.iter().copied() {
        let mut count = 0;
        for keyword in keywords {
            if normalized_query.contains(keyword) {
                count += 1;
            }
        }
        if count > 0 {
            role_counts.insert(role, count);
        }
    }

    if role_counts.is_empty() {
        return QueryProfile::default();
    }

    let mut sorted: Vec<(DocRole, usize)> = role_counts.into_iter().collect();
    sorted.sort_by(|(role_a, count_a), (role_b, count_b)| {
        count_b.cmp(count_a).then_with(|| role_a.order().cmp(&role_b.order()))
    });

    let primary_role = Some(sorted[0].0);
    let secondary_roles = sorted.into_iter().skip(1).map(|(role, _)| role).collect();

    QueryProfile {
        primary_role,
        secondary_roles,
    }
}

/// Returns a deterministic role bonus for ranking.
fn role_bonus(role: DocRole, profile: &QueryProfile) -> i32 {
    if Some(role) == profile.primary_role {
        return 2;
    }
    if profile.secondary_roles.contains(&role) {
        return 1;
    }
    0
}

/// Builds unique document coverage metadata from ranked sections.
fn coverage_from_sections(sections: &[SearchSection]) -> Vec<DocCoverage> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut coverage = Vec::new();

    for section in sections {
        if seen.insert(&section.doc_id) {
            coverage.push(DocCoverage {
                doc_id: section.doc_id.clone(),
                doc_title: section.doc_title.clone(),
                doc_role: section.doc_role,
            });
        }
    }

    coverage
}

/// Builds role-aware follow-up suggestions based on coverage.
fn suggested_followups(coverage: &[DocCoverage]) -> Vec<String> {
    if coverage.is_empty() {
        return Vec::new();
    }

    let present_roles: HashSet<DocRole> = coverage.iter().map(|c| c.doc_role).collect();
    let mut suggestions = Vec::new();

    if !present_roles.contains(&DocRole::Decision) {
        suggestions.push(String::from(
            "If you need tool selection guidance, search the tooling or LLM playbook docs.",
        ));
    }
    if !present_roles.contains(&DocRole::Reasoning) {
        suggestions.push(String::from(
            "For conceptual grounding, query evidence flow or security guidance.",
        ));
    }
    if !present_roles.contains(&DocRole::Pattern) {
        suggestions.push(String::from(
            "For reusable workflows, search the playbook or integration patterns.",
        ));
    }
    if !present_roles.contains(&DocRole::Ontology) {
        suggestions.push(String::from(
            "For schema or provider details, search the providers or condition authoring docs.",
        ));
    }

    suggestions
}

// ============================================================================
// SECTION: Resources Helper Types
// ============================================================================

/// Resource metadata returned from `resources/list`.
///
/// # Invariants
/// - `uri` values use the [`RESOURCE_URI_PREFIX`].
/// - `mime_type` is the embedded docs MIME type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceMetadata {
    /// Stable MCP URI for the resource.
    pub uri: String,
    /// Human-readable name for resource listings.
    pub name: String,
    /// Short description of the document contents.
    pub description: String,
    /// MIME type for resource content.
    #[serde(rename = "mimeType")]
    pub mime_type: &'static str,
}

/// Resource content returned from `resources/read`.
///
/// # Invariants
/// - `uri` values use the [`RESOURCE_URI_PREFIX`].
/// - `mime_type` is the embedded docs MIME type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceContent {
    /// URI matching the requested resource.
    pub uri: String,
    /// MIME type for the payload.
    #[serde(rename = "mimeType")]
    pub mime_type: &'static str,
    /// Raw Markdown body.
    pub text: String,
}

impl DocEntry {
    /// Returns the metadata used by `resources/list`.
    #[must_use]
    pub fn metadata(&self) -> ResourceMetadata {
        ResourceMetadata {
            uri: self.resource_uri.clone(),
            name: self.title.clone(),
            description: self.resource_description.clone(),
            mime_type: DOC_MIME_TYPE,
        }
    }

    /// Returns the payload used by `resources/read`.
    #[must_use]
    pub fn content(&self) -> ResourceContent {
        ResourceContent {
            uri: self.resource_uri.clone(),
            mime_type: DOC_MIME_TYPE,
            text: self.body.clone(),
        }
    }
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "Test assertions use expect/unwrap for clarity."
    )]

    use super::DocsCatalog;
    use super::DocsSearchRequest;
    use super::RESOURCE_URI_PREFIX;
    use crate::config::DocsConfig;

    #[test]
    fn search_returns_sections() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let result = catalog.search(&DocsSearchRequest {
            query: "precheck evidence".to_string(),
            max_sections: Some(2),
        });
        assert!(!result.sections.is_empty(), "search should return sections");
    }

    #[test]
    fn resources_use_expected_prefix() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let first = catalog.docs().first().expect("doc");
        assert!(
            first.resource_uri.starts_with(RESOURCE_URI_PREFIX),
            "resource URIs should use the docs prefix"
        );
    }

    // ============================================================================
    // SECTION: DocsCatalog::from_config() Tests (20 tests)
    // ============================================================================

    #[test]
    fn docs_catalog_from_config_loads_defaults() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("should load");
        assert_eq!(catalog.docs().len(), 13, "should have 13 default docs");
        assert!(catalog.warnings().is_empty(), "should have no warnings");
    }

    #[test]
    fn docs_catalog_from_config_respects_enabled_false() {
        let config = DocsConfig {
            enabled: false,
            ..DocsConfig::default()
        };
        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 0, "should have no docs when disabled");
        assert!(catalog.is_empty(), "catalog should be empty");
    }

    #[test]
    fn docs_catalog_from_config_excludes_defaults_when_disabled() {
        let config = DocsConfig {
            include_default_docs: false,
            ..DocsConfig::default()
        };
        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 0, "should have no docs when defaults excluded");
    }

    #[test]
    fn docs_catalog_from_config_includes_13_default_docs() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("should load");
        let docs = catalog.docs();
        assert_eq!(docs.len(), 13, "should have exactly 13 default docs");

        // Verify all expected doc IDs are present
        let expected_ids = [
            "evidence_flow_and_execution_model",
            "security_guide",
            "tooling_summary",
            "authoring_formats",
            "condition_authoring",
            "ret_logic",
            "llm_native_playbook",
            "providers_summary",
            "provider_protocol",
            "provider_schema_authoring",
            "provider_development",
            "json_evidence_playbook",
            "runpack_architecture",
        ];

        for expected_id in &expected_ids {
            assert!(
                docs.iter().any(|doc| doc.id == *expected_id),
                "should include doc: {expected_id}"
            );
        }
    }

    #[test]
    fn docs_catalog_from_config_default_docs_have_correct_roles() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("should load");
        let docs = catalog.docs();

        // Verify specific docs have expected roles
        let evidence_flow = docs.iter().find(|d| d.id == "evidence_flow_and_execution_model");
        assert!(evidence_flow.is_some());
        assert_eq!(evidence_flow.unwrap().role, super::DocRole::Reasoning);

        let tooling = docs.iter().find(|d| d.id == "tooling_summary");
        assert!(tooling.is_some());
        assert_eq!(tooling.unwrap().role, super::DocRole::Decision);

        let conditions = docs.iter().find(|d| d.id == "condition_authoring");
        assert!(conditions.is_some());
        assert_eq!(conditions.unwrap().role, super::DocRole::Ontology);

        let provider_protocol = docs.iter().find(|d| d.id == "provider_protocol");
        assert!(provider_protocol.is_some());
        assert_eq!(provider_protocol.unwrap().role, super::DocRole::Pattern);
    }

    #[test]
    fn docs_catalog_from_config_default_docs_have_unique_uris() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("should load");
        let docs = catalog.docs();

        let mut uris = std::collections::HashSet::new();
        for doc in docs {
            assert!(uris.insert(&doc.resource_uri), "URI should be unique: {}", doc.resource_uri);
        }
    }

    #[test]
    fn docs_catalog_from_config_fails_max_doc_bytes_too_small_for_defaults() {
        let config = DocsConfig {
            max_doc_bytes: 100, // Too small for default docs
            ..DocsConfig::default()
        };
        let result = DocsCatalog::from_config(&config);
        assert!(result.is_err(), "should fail when max_doc_bytes too small");
        assert!(result.unwrap_err().to_string().contains("max_doc_bytes"));
    }

    #[test]
    fn docs_catalog_from_config_fails_max_docs_too_small_for_defaults() {
        let config = DocsConfig {
            max_docs: 5, // Too small for 13 default docs
            ..DocsConfig::default()
        };
        let result = DocsCatalog::from_config(&config);
        assert!(result.is_err(), "should fail when max_docs too small");
        assert!(result.unwrap_err().to_string().contains("max_docs"));
    }

    #[test]
    fn docs_catalog_from_config_fails_max_total_bytes_too_small_for_defaults() {
        let config = DocsConfig {
            max_total_bytes: 1000, // Too small for all default docs
            ..DocsConfig::default()
        };
        let result = DocsCatalog::from_config(&config);
        assert!(result.is_err(), "should fail when max_total_bytes too small");
        assert!(result.unwrap_err().to_string().contains("max_total_bytes"));
    }

    #[test]
    fn docs_catalog_from_config_empty_extra_paths() {
        let config = DocsConfig {
            extra_paths: Vec::new(),
            ..DocsConfig::default()
        };
        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 13, "should have only default docs");
    }

    #[test]
    fn docs_catalog_from_config_warns_when_extra_doc_skipped() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        let doc_path = temp_dir.path().join("large.md");

        // Create a doc that exceeds max_doc_bytes
        let large_content = "# Large Doc\n".to_string() + &"x".repeat(300_000);
        fs::write(&doc_path, large_content).expect("write file");

        let config = DocsConfig {
            extra_paths: vec![doc_path.to_string_lossy().to_string()],
            max_doc_bytes: 256 * 1024, // 256KB limit
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert!(!catalog.warnings().is_empty(), "should have warnings");
        assert!(
            catalog.warnings().iter().any(|w| w.contains("exceeds max_doc_bytes")),
            "warning should mention max_doc_bytes"
        );
    }

    #[test]
    fn docs_catalog_from_config_stops_at_max_docs() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");

        // Create multiple small docs
        for i in 0 .. 5 {
            let doc_path = temp_dir.path().join(format!("doc{i}.md"));
            fs::write(&doc_path, format!("# Doc {i}\nContent")).expect("write file");
        }

        let config = DocsConfig {
            extra_paths: vec![temp_dir.path().to_string_lossy().to_string()],
            max_docs: 15, // 13 defaults + 2 extras
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 15, "should stop at max_docs");
        assert!(
            catalog.warnings().iter().any(|w| w.contains("max_docs reached")),
            "should warn about hitting max_docs"
        );
    }

    #[test]
    fn docs_catalog_from_config_respects_max_total_bytes() {
        // Test that max_total_bytes is enforced by checking catalog accepts valid config
        let config = DocsConfig {
            max_total_bytes: 2_000_000, // Large enough for defaults
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load with large limit");
        assert!(!catalog.docs().is_empty(), "should have docs");

        // Verify that a config with very small max_total_bytes would fail
        let tiny_config = DocsConfig {
            max_total_bytes: 1000, // Too small for defaults
            ..DocsConfig::default()
        };

        let result = DocsCatalog::from_config(&tiny_config);
        assert!(result.is_err(), "should fail with tiny max_total_bytes");
    }

    #[test]
    fn docs_catalog_from_config_fails_on_missing_extra_path() {
        let config = DocsConfig {
            extra_paths: vec!["/nonexistent/path/to/docs".to_string()],
            ..DocsConfig::default()
        };
        let result = DocsCatalog::from_config(&config);
        assert!(result.is_err(), "should fail on missing path");
        assert!(result.unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn docs_catalog_from_config_loads_file() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        let doc_path = temp_dir.path().join("test.md");
        fs::write(&doc_path, "# Test Doc\nTest content").expect("write file");

        let config = DocsConfig {
            extra_paths: vec![doc_path.to_string_lossy().to_string()],
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 14, "should have 13 defaults + 1 extra");
        assert!(catalog.docs().iter().any(|d| d.title == "Test Doc"), "should include extra doc");
    }

    #[test]
    fn docs_catalog_from_config_loads_directory_recursively() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).expect("create subdir");

        fs::write(temp_dir.path().join("doc1.md"), "# Doc 1\nContent").expect("write");
        fs::write(sub_dir.join("doc2.md"), "# Doc 2\nContent").expect("write");

        let config = DocsConfig {
            extra_paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 15, "should have 13 defaults + 2 extras");
    }

    #[test]
    fn docs_catalog_from_config_directory_ingestion_is_deterministic() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        let dir_a = temp_dir.path().join("a");
        let dir_b = temp_dir.path().join("b");
        fs::create_dir(&dir_a).expect("create dir a");
        fs::create_dir(&dir_b).expect("create dir b");
        fs::write(dir_a.join("guide.md"), "# A Guide\nContent").expect("write a guide");
        fs::write(dir_b.join("guide.md"), "# B Guide\nContent").expect("write b guide");

        let config = DocsConfig {
            include_default_docs: false,
            extra_paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 2, "should load both docs");
        assert_eq!(catalog.docs()[0].id, "guide", "first duplicate keeps base id");
        assert_eq!(catalog.docs()[1].id, "guide_2", "second duplicate uses deterministic suffix");
        assert_eq!(catalog.docs()[0].title, "A Guide", "lexicographic traversal is stable");
        assert_eq!(catalog.docs()[1].title, "B Guide", "lexicographic traversal is stable");
    }

    #[test]
    fn docs_catalog_from_config_enforces_extra_doc_total_budget() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        fs::write(temp_dir.path().join("a.md"), "# A\n1234567890").expect("write a");
        fs::write(temp_dir.path().join("b.md"), "# B\n1234567890").expect("write b");

        let config = DocsConfig {
            include_default_docs: false,
            max_total_bytes: 20,
            extra_paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 1, "budget should allow only one doc");
        assert!(
            catalog.warnings().iter().any(|warning| warning.contains("max_total_bytes")),
            "should warn when total doc budget is exceeded"
        );
    }

    #[test]
    fn docs_catalog_from_config_skips_non_markdown() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("create temp dir");
        fs::write(temp_dir.path().join("doc.md"), "# Doc\nContent").expect("write");
        fs::write(temp_dir.path().join("readme.txt"), "Not markdown").expect("write");

        let config = DocsConfig {
            extra_paths: vec![temp_dir.path().to_string_lossy().to_string()],
            ..DocsConfig::default()
        };

        let catalog = DocsCatalog::from_config(&config).expect("should load");
        assert_eq!(catalog.docs().len(), 14, "should skip non-markdown files");
    }

    // ============================================================================
    // SECTION: DocsCatalog::search() Tests (24 tests)
    // ============================================================================

    #[test]
    fn docs_search_returns_ranked_sections() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let result = catalog.search(&DocsSearchRequest {
            query: "evidence provider".to_string(),
            max_sections: Some(5),
        });
        assert!(!result.sections.is_empty(), "should return sections");
        assert!(result.sections.len() <= 5, "should respect max_sections");

        // Verify sections are ranked
        for (i, section) in result.sections.iter().enumerate() {
            assert_eq!(section.rank, i, "rank should match position");
        }
    }

    #[test]
    fn docs_search_respects_max_sections() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(3),
        });
        assert!(result.sections.len() <= 3, "should not exceed max_sections=3");

        let result2 = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(7),
        });
        assert!(result2.sections.len() <= 7, "should not exceed max_sections=7");
    }

    #[test]
    fn docs_search_empty_query_returns_overview() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let result = catalog.search(&DocsSearchRequest {
            query: String::new(),
            max_sections: Some(4),
        });

        assert!(!result.sections.is_empty(), "overview should return sections");
        assert!(result.sections.len() <= 4, "should respect max_sections for overview");

        // Overview should include different roles
        let roles: std::collections::HashSet<_> =
            result.sections.iter().map(|s| s.doc_role).collect();
        assert!(roles.len() > 1, "overview should span multiple roles");
    }

    #[test]
    fn docs_search_includes_docs_covered() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let result = catalog.search(&DocsSearchRequest {
            query: "provider check".to_string(),
            max_sections: Some(5),
        });

        assert!(!result.docs_covered.is_empty(), "should list covered docs");

        // All docs_covered should correspond to returned sections
        for coverage in &result.docs_covered {
            assert!(
                result.sections.iter().any(|s| s.doc_id == coverage.doc_id),
                "covered doc should appear in sections"
            );
        }
    }

    #[test]
    fn docs_search_includes_suggested_followups() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let result = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(3),
        });

        // Should have followup suggestions
        assert!(!result.suggested_followups.is_empty(), "should suggest followups");
    }

    #[test]
    fn docs_search_ranks_heading_matches_higher() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let result = catalog.search(&DocsSearchRequest {
            query: "Provider Protocol".to_string(), // Matches heading exactly
            max_sections: Some(10),
        });

        // The "Provider Protocol" doc should rank highly
        let top_results: Vec<_> = result.sections.iter().take(3).collect();
        assert!(
            top_results.iter().any(|s| s.doc_title.contains("Provider Protocol")
                || s.heading.contains("Provider Protocol")),
            "heading match should rank in top 3"
        );
    }

    #[test]
    fn docs_search_deterministic_across_runs() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");
        let query = DocsSearchRequest {
            query: "evidence trust".to_string(),
            max_sections: Some(5),
        };

        let result1 = catalog.search(&query);
        let result2 = catalog.search(&query);

        assert_eq!(result1.sections.len(), result2.sections.len(), "same result count");
        for (s1, s2) in result1.sections.iter().zip(result2.sections.iter()) {
            assert_eq!(s1.doc_id, s2.doc_id, "same doc_id");
            assert_eq!(s1.heading, s2.heading, "same heading");
            assert_eq!(s1.rank, s2.rank, "same rank");
        }
    }

    #[test]
    fn docs_search_clamps_max_sections_to_absolute_max() {
        let config = DocsConfig {
            max_sections: 10, // ABSOLUTE_MAX_SECTIONS
            ..DocsConfig::default()
        };
        let catalog = DocsCatalog::from_config(&config).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(100), // Request more than allowed
        });

        assert!(result.sections.len() <= 10, "should clamp to ABSOLUTE_MAX_SECTIONS");
    }

    #[test]
    fn docs_search_handles_whitespace_in_query() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "  provider   check  ".to_string(), // Extra whitespace
            max_sections: Some(5),
        });

        assert!(!result.sections.is_empty(), "should handle whitespace");
    }

    #[test]
    fn docs_search_case_insensitive_matching() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result1 = catalog.search(&DocsSearchRequest {
            query: "PROVIDER".to_string(),
            max_sections: Some(5),
        });

        let result2 = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(5),
        });

        // Should return similar results (case insensitive)
        assert_eq!(
            result1.sections.len(),
            result2.sections.len(),
            "case should not affect match count"
        );
    }

    #[test]
    fn docs_search_empty_catalog_returns_empty() {
        let config = DocsConfig {
            enabled: false,
            ..DocsConfig::default()
        };
        let catalog = DocsCatalog::from_config(&config).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "anything".to_string(),
            max_sections: Some(5),
        });

        assert!(result.sections.is_empty(), "empty catalog should return no results");
        assert!(result.docs_covered.is_empty());
    }

    #[test]
    fn docs_search_no_matches_returns_empty() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "xyzabc123impossible".to_string(), // No matches
            max_sections: Some(5),
        });

        assert!(result.sections.is_empty(), "no matches should return empty");
        assert!(result.docs_covered.is_empty());
    }

    #[test]
    fn docs_search_uses_default_max_sections_when_none() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: None, // Use default
        });

        // Default is 3
        assert!(result.sections.len() <= 3, "should use default max_sections=3");
    }

    #[test]
    fn docs_search_profiles_query_ontology_keywords() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "schema provider comparator".to_string(), // Ontology keywords
            max_sections: Some(10),
        });

        // Should bias towards Ontology role docs
        let ontology_count =
            result.sections.iter().filter(|s| s.doc_role == super::DocRole::Ontology).count();
        assert!(ontology_count > 0, "should return some Ontology docs");
    }

    #[test]
    fn docs_search_profiles_query_decision_keywords() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "which tool precheck scenario".to_string(), // Decision keywords
            max_sections: Some(10),
        });

        // Should include Decision role docs
        let decision_count =
            result.sections.iter().filter(|s| s.doc_role == super::DocRole::Decision).count();
        assert!(decision_count > 0, "should return some Decision docs");
    }

    #[test]
    fn docs_search_profiles_query_pattern_keywords() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "playbook workflow recipe".to_string(), // Pattern keywords
            max_sections: Some(10),
        });

        // Should include Pattern role docs
        let pattern_count =
            result.sections.iter().filter(|s| s.doc_role == super::DocRole::Pattern).count();
        assert!(pattern_count > 0, "should return some Pattern docs");
    }

    #[test]
    fn docs_search_profiles_query_reasoning_keywords() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "why trust evidence security".to_string(), // Reasoning keywords
            max_sections: Some(10),
        });

        // Should include Reasoning role docs
        let reasoning_count =
            result.sections.iter().filter(|s| s.doc_role == super::DocRole::Reasoning).count();
        assert!(reasoning_count > 0, "should return some Reasoning docs");
    }

    #[test]
    fn docs_search_max_sections_minimum_is_one() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(0), // Invalid, should clamp to 1
        });

        assert!(result.sections.len() <= 1, "should clamp to minimum of 1");
        assert!(!result.sections.is_empty() || result.docs_covered.is_empty());
    }

    #[test]
    fn docs_search_overview_suggests_refinement() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: String::new(), // Empty query triggers overview
            max_sections: Some(4),
        });

        // Overview should suggest refining the query
        assert!(
            result
                .suggested_followups
                .iter()
                .any(|s| s.to_lowercase().contains("refine") || s.to_lowercase().contains("query")),
            "overview should suggest query refinement"
        );
    }

    #[test]
    fn docs_search_sections_have_complete_metadata() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        let result = catalog.search(&DocsSearchRequest {
            query: "provider".to_string(),
            max_sections: Some(3),
        });

        for section in &result.sections {
            assert!(!section.doc_id.is_empty(), "doc_id should not be empty");
            assert!(!section.doc_title.is_empty(), "doc_title should not be empty");
            assert!(
                !section.heading.is_empty() || !section.content.is_empty(),
                "should have content"
            );
        }
    }

    #[test]
    fn docs_search_stable_ordering_for_identical_scores() {
        let catalog = DocsCatalog::from_config(&DocsConfig::default()).expect("catalog");

        // Run same query multiple times
        let query = DocsSearchRequest {
            query: "evidence".to_string(),
            max_sections: Some(10),
        };

        let results: Vec<_> = (0 .. 5).map(|_| catalog.search(&query)).collect();

        // All results should be identical
        for i in 1 .. results.len() {
            assert_eq!(
                results[0].sections.len(),
                results[i].sections.len(),
                "run {i} should have same count"
            );
            for (j, (s0, si)) in
                results[0].sections.iter().zip(results[i].sections.iter()).enumerate()
            {
                assert_eq!(s0.doc_id, si.doc_id, "run {i}, section {j}: same doc_id");
                assert_eq!(s0.heading, si.heading, "run {i}, section {j}: same heading");
            }
        }
    }

    // ============================================================================
    // SECTION: Internal Helper Function Tests (20 tests)
    // ============================================================================

    #[test]
    fn split_doc_sections_creates_intro_section() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test Doc".to_string(),
            body: "Intro content without headings".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert!(!sections.is_empty(), "should create intro section");
        assert_eq!(sections[0].heading, "Test Doc", "intro heading should be doc title");
    }

    #[test]
    fn split_doc_sections_splits_on_h2_heading() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "Intro\n## Section 1\nContent 1\n## Section 2\nContent 2".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert_eq!(sections.len(), 3, "should have intro + 2 sections");
        assert_eq!(sections[1].heading, "Section 1");
        assert_eq!(sections[2].heading, "Section 2");
    }

    #[test]
    fn split_doc_sections_splits_on_h3_heading() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "Intro\n### Subsection 1\nContent".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert!(sections.len() >= 2, "should have intro + subsection");
        assert!(sections.iter().any(|s| s.heading == "Subsection 1"));
    }

    #[test]
    fn split_doc_sections_preserves_heading_text() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "## Heading With Spaces  \nContent".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert!(sections.iter().any(|s| s.heading == "Heading With Spaces"));
    }

    #[test]
    fn split_doc_sections_trims_content() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "  \n  Intro content  \n  ".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert_eq!(sections[0].content.trim(), sections[0].content, "content should be trimmed");
    }

    #[test]
    fn split_doc_sections_no_headings_returns_single_section() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "Just content\nNo headings\nMore content".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert_eq!(sections.len(), 1, "should have single intro section");
        assert!(sections[0].content.contains("Just content"));
    }

    #[test]
    fn split_doc_sections_consecutive_headings() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "## Heading 1\n## Heading 2\nContent".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        assert!(sections.len() >= 2, "should create sections for each heading");
    }

    #[test]
    fn split_doc_sections_preserves_code_blocks() {
        let doc = super::DocEntry {
            id: "test".to_string(),
            title: "Test".to_string(),
            body: "## Code Example\n```rust\nfn main() {}\n```".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "test".to_string(),
        };

        let sections = super::split_doc_sections(&doc);
        let code_section = sections.iter().find(|s| s.heading == "Code Example");
        assert!(code_section.is_some());
        assert!(code_section.unwrap().content.contains("```"));
    }

    #[test]
    fn lexical_score_heading_match_scores_3() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Provider Protocol".to_string(),
            content: "Some content".to_string(),
        };

        let score = super::lexical_score(&section, "provider");
        assert!(score >= 3, "heading match should score at least 3");
    }

    #[test]
    fn lexical_score_content_match_scores_1() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Heading".to_string(),
            content: "This content mentions provider".to_string(),
        };

        let score = super::lexical_score(&section, "provider");
        assert!(score >= 1, "content match should score at least 1");
    }

    #[test]
    fn lexical_score_multiple_term_matches_accumulate() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Provider Protocol".to_string(),
            content: "Provider protocol details".to_string(),
        };

        let score = super::lexical_score(&section, "provider protocol");
        assert!(score >= 6, "multiple matches should accumulate (got {score})");
    }

    #[test]
    fn lexical_score_no_match_returns_zero() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Heading".to_string(),
            content: "Content".to_string(),
        };

        let score = super::lexical_score(&section, "nonexistent");
        assert_eq!(score, 0, "no match should return 0");
    }

    #[test]
    fn lexical_score_case_insensitive() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Provider".to_string(),
            content: "content".to_string(),
        };

        let score1 = super::lexical_score(&section, "PROVIDER");
        let score2 = super::lexical_score(&section, "provider");
        assert_eq!(score1, score2, "scoring should be case insensitive");
    }

    #[test]
    fn lexical_score_empty_query_returns_zero() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Heading".to_string(),
            content: "Content".to_string(),
        };

        let score = super::lexical_score(&section, "");
        assert_eq!(score, 0, "empty query should return 0");
    }

    #[test]
    fn lexical_score_whitespace_only_terms_ignored() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Heading".to_string(),
            content: "Content".to_string(),
        };

        let score = super::lexical_score(&section, "   ");
        assert_eq!(score, 0, "whitespace-only query should return 0");
    }

    #[test]
    fn lexical_score_partial_word_match_counts() {
        let section = super::DocSection {
            doc_id: "test".to_string(),
            doc_title: "Test".to_string(),
            doc_role: super::DocRole::Pattern,
            heading: "Providers".to_string(),
            content: "content".to_string(),
        };

        let score = super::lexical_score(&section, "provider");
        assert!(score > 0, "partial word match should count");
    }

    #[test]
    fn doc_role_order_is_stable() {
        assert_eq!(super::DocRole::Reasoning.order(), 0);
        assert_eq!(super::DocRole::Decision.order(), 1);
        assert_eq!(super::DocRole::Ontology.order(), 2);
        assert_eq!(super::DocRole::Pattern.order(), 3);
    }

    #[test]
    fn doc_entry_metadata_has_correct_fields() {
        let doc = super::DocEntry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            body: "Body content".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "Description".to_string(),
        };

        let metadata = doc.metadata();
        assert_eq!(metadata.uri, "test://uri");
        assert_eq!(metadata.name, "Test Title");
        assert_eq!(metadata.description, "Description");
        assert_eq!(metadata.mime_type, "text/markdown");
    }

    #[test]
    fn doc_entry_content_has_correct_fields() {
        let doc = super::DocEntry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            body: "Body content".to_string(),
            role: super::DocRole::Pattern,
            resource_uri: "test://uri".to_string(),
            resource_description: "Description".to_string(),
        };

        let content = doc.content();
        assert_eq!(content.uri, "test://uri");
        assert_eq!(content.text, "Body content");
        assert_eq!(content.mime_type, "text/markdown");
    }
}
