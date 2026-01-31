// system-tests/tests/helpers/docs.rs
// ============================================================================
// Module: Docs Helpers
// Description: Local structs for parsing docs search and resource payloads.
// Purpose: Decode MCP docs responses without relying on internal types.
// Dependencies: serde
// ============================================================================

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocRole {
    Reasoning,
    Decision,
    Ontology,
    Pattern,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchSection {
    pub rank: usize,
    pub doc_id: String,
    pub doc_title: String,
    pub doc_role: DocRole,
    pub heading: String,
    pub content: String,
}

#[allow(
    clippy::struct_field_names,
    reason = "Field names mirror docs search payloads for clarity."
)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocCoverage {
    pub doc_id: String,
    pub doc_title: String,
    pub doc_role: DocRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchResult {
    pub sections: Vec<SearchSection>,
    pub docs_covered: Vec<DocCoverage>,
    pub suggested_followups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ResourceMetadata {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub text: String,
}
