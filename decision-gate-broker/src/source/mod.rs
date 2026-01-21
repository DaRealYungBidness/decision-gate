// decision-gate-broker/src/source/mod.rs
// ============================================================================
// Module: Decision Gate Broker Sources
// Description: Source traits and reference implementations for payload resolution.
// Purpose: Resolve external content references into payload bytes.
// Dependencies: decision-gate-core, thiserror
// ============================================================================

//! ## Overview
//! Sources fetch external content referenced by Decision Gate packet payloads.
//! Implementations must fail closed on invalid URIs or fetch errors.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::ContentRef;
use thiserror::Error;

// ============================================================================
// SECTION: Source Payload
// ============================================================================

/// Payload bytes resolved from an external source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePayload {
    /// Raw payload bytes.
    pub bytes: Vec<u8>,
    /// Optional content type hint.
    pub content_type: Option<String>,
}

// ============================================================================
// SECTION: Source Errors
// ============================================================================

/// Errors emitted by broker sources.
#[derive(Debug, Error)]
pub enum SourceError {
    /// Unsupported or missing URI scheme.
    #[error("unsupported uri scheme: {0}")]
    UnsupportedScheme(String),
    /// URI failed to parse or resolve.
    #[error("invalid uri: {0}")]
    InvalidUri(String),
    /// Resource was not found.
    #[error("resource not found: {0}")]
    NotFound(String),
    /// Source reported an I/O failure.
    #[error("io failure: {0}")]
    Io(String),
    /// HTTP source failed.
    #[error("http failure: {0}")]
    Http(String),
    /// Inline source failed to decode payload.
    #[error("inline decode failure: {0}")]
    Decode(String),
}

// ============================================================================
// SECTION: Source Trait
// ============================================================================

/// Resolves content references into payload bytes.
pub trait Source: Send + Sync {
    /// Fetches payload bytes for the provided content reference.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] when the content cannot be resolved.
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError>;
}

// ============================================================================
// SECTION: Implementations
// ============================================================================

pub mod file;
pub mod http;
pub mod inline;

pub use file::FileSource;
pub use http::HttpSource;
pub use inline::InlineSource;
