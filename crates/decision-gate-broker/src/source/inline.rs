// crates/decision-gate-broker/src/source/inline.rs
// ============================================================================
// Module: Decision Gate Inline Source
// Description: Inline payload source for embedded content references.
// Purpose: Decode inline payloads encoded into content URIs.
// Dependencies: base64, decision-gate-core
// ============================================================================

//! ## Overview
//! [`InlineSource`] resolves `inline:` URIs that embed payload bytes directly.
//! Supported prefixes: `inline+json:`, `inline+bytes:`, and `inline:`.
//! Invariants:
//! - Base64 payloads are size-checked before and after decoding.
//!
//! Security posture: treats inline payloads as untrusted input; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use decision_gate_core::ContentRef;

use crate::source::Source;
use crate::source::SourceError;
use crate::source::SourcePayload;
use crate::source::enforce_max_bytes;

// ============================================================================
// SECTION: Inline Source
// ============================================================================

/// Inline payload source using base64-encoded payloads.
///
/// # Invariants
/// - Decoded payloads are capped at [`crate::source::MAX_SOURCE_BYTES`].
#[derive(Debug, Default, Clone, Copy)]
pub struct InlineSource;

impl InlineSource {
    /// Creates a new inline source.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Decodes a base64-encoded payload.
    fn decode_base64(encoded: &str) -> Result<Vec<u8>, SourceError> {
        let estimated = encoded.len().div_ceil(4).saturating_mul(3);
        enforce_max_bytes(estimated)?;
        let bytes = STANDARD
            .decode(encoded.as_bytes())
            .map_err(|err| SourceError::Decode(err.to_string()))?;
        enforce_max_bytes(bytes.len())?;
        Ok(bytes)
    }
}

impl Source for InlineSource {
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError> {
        let uri = content_ref.uri.as_str();
        if let Some(encoded) = uri.strip_prefix("inline+json:") {
            let bytes = Self::decode_base64(encoded)?;
            return Ok(SourcePayload {
                bytes,
                content_type: Some("application/json".to_string()),
            });
        }
        if let Some(encoded) = uri.strip_prefix("inline+bytes:") {
            let bytes = Self::decode_base64(encoded)?;
            return Ok(SourcePayload {
                bytes,
                content_type: Some("application/octet-stream".to_string()),
            });
        }
        if let Some(encoded) = uri.strip_prefix("inline:") {
            let bytes = Self::decode_base64(encoded)?;
            return Ok(SourcePayload {
                bytes,
                content_type: None,
            });
        }
        Err(SourceError::UnsupportedScheme("inline".to_string()))
    }
}
