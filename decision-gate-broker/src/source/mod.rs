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
//! Security posture: all source inputs are untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::ContentRef;
use decision_gate_core::runtime::MAX_PAYLOAD_BYTES;
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

/// Maximum payload size accepted by broker sources.
pub const MAX_SOURCE_BYTES: usize = MAX_PAYLOAD_BYTES;

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
    /// Payload exceeded the configured byte limit.
    #[error("payload exceeds size limit: {actual_bytes} bytes (max {max_bytes})")]
    TooLarge {
        /// Maximum allowed bytes.
        max_bytes: usize,
        /// Actual payload size in bytes.
        actual_bytes: usize,
    },
    /// Configured byte limit cannot be represented for I/O operations.
    #[error("payload size limit exceeds platform bounds: {limit} bytes")]
    LimitOverflow {
        /// Configured size limit in bytes.
        limit: usize,
    },
}

/// Returns an error when a payload exceeds the configured size cap.
pub(crate) const fn enforce_max_bytes(actual_bytes: usize) -> Result<(), SourceError> {
    if actual_bytes > MAX_SOURCE_BYTES {
        return Err(SourceError::TooLarge {
            max_bytes: MAX_SOURCE_BYTES,
            actual_bytes,
        });
    }
    Ok(())
}

/// Converts the configured size limit to `u64` for I/O operations.
pub(crate) fn max_source_bytes_u64() -> Result<u64, SourceError> {
    u64::try_from(MAX_SOURCE_BYTES).map_err(|_| SourceError::LimitOverflow {
        limit: MAX_SOURCE_BYTES,
    })
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
