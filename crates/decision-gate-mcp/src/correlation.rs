// crates/decision-gate-mcp/src/correlation.rs
// ============================================================================
// Module: Correlation Policy
// Description: Sanitization and generation for client/server correlation IDs.
// Purpose: Provide deterministic, fail-closed correlation handling for MCP.
// Dependencies: rand
// ============================================================================

//! ## Overview
//!
//! This module defines the correlation ID policy for Decision Gate MCP.
//! Client-provided correlation identifiers are **unsafe** and must be
//! sanitized before use. Invalid inputs are rejected to maintain strict,
//! auditable boundaries. Server correlation IDs are generated per request
//! using a boot-scoped random seed plus a monotonic counter.
//! Security posture: correlation headers are untrusted input and must be
//! sanitized; see `Docs/security/threat_model.md`.

use std::fmt;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use rand::RngCore;
use rand::rngs::OsRng;

/// Header name for client-provided correlation identifiers.
pub const CLIENT_CORRELATION_HEADER: &str = "x-correlation-id";
/// Header name for server-issued correlation identifiers.
pub const SERVER_CORRELATION_HEADER: &str = "x-server-correlation-id";
/// Maximum allowed length for client correlation identifiers.
pub const MAX_CLIENT_CORRELATION_ID_LENGTH: usize = 128;

/// Typed rejection reason for invalid client correlation IDs.
///
/// # Invariants
/// - Variants are stable for audit labeling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrelationIdRejection {
    /// Input was empty after trimming.
    EmptyAfterTrim,
    /// Input exceeded the maximum length.
    TooLong,
    /// Input contained whitespace after trimming.
    ContainsWhitespace,
    /// Input contained control characters after trimming.
    ContainsControlChar,
    /// Input contained non-ASCII characters.
    NonAscii,
    /// Input contained disallowed ASCII characters.
    ContainsDisallowedChar,
}

impl CorrelationIdRejection {
    /// Returns a stable label for this rejection reason.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::EmptyAfterTrim => "empty_after_trim",
            Self::TooLong => "too_long",
            Self::ContainsWhitespace => "contains_whitespace",
            Self::ContainsControlChar => "contains_control_char",
            Self::NonAscii => "non_ascii",
            Self::ContainsDisallowedChar => "contains_disallowed_char",
        }
    }
}

impl fmt::Display for CorrelationIdRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Correlation context containing unsafe client and server identifiers.
///
/// # Invariants
/// - `server_id` is always populated for issued contexts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationContext {
    /// Sanitized client correlation ID (unsafe input).
    pub unsafe_client_id: Option<String>,
    /// Server-generated correlation ID.
    pub server_id: String,
}

impl CorrelationContext {
    /// Builds a correlation context from a client header and generator.
    ///
    /// # Errors
    /// Returns [`CorrelationIdRejection`] when the client ID is invalid.
    pub fn from_header(
        header: Option<&str>,
        generator: &CorrelationIdGenerator,
    ) -> Result<Self, CorrelationIdRejection> {
        let unsafe_client_id = sanitize_client_correlation_id(header)?;
        let server_id = generator.issue();
        Ok(Self {
            unsafe_client_id,
            server_id,
        })
    }
}

/// Boot-scoped correlation ID generator.
///
/// # Invariants
/// - Issued identifiers are unique within the process lifetime.
#[derive(Debug)]
pub struct CorrelationIdGenerator {
    /// Prefix included in every generated correlation ID.
    prefix: &'static str,
    /// Boot-scoped random identifier for entropy.
    boot_id: u64,
    /// Monotonic counter for IDs issued in this process.
    counter: AtomicU64,
}

impl CorrelationIdGenerator {
    /// Creates a new generator with the given prefix.
    #[must_use]
    pub fn new(prefix: &'static str) -> Self {
        let mut bytes = [0u8; 8];
        OsRng.fill_bytes(&mut bytes);
        Self {
            prefix,
            boot_id: u64::from_be_bytes(bytes),
            counter: AtomicU64::new(1),
        }
    }

    /// Issues a new server correlation ID.
    #[must_use]
    pub fn issue(&self) -> String {
        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("{}-{:016x}-{:016x}", self.prefix, self.boot_id, seq)
    }
}

/// Sanitizes a client correlation ID using strict token rules.
///
/// Returns `Ok(None)` when no header value is provided. Any invalid value
/// returns a structured rejection reason.
///
/// # Errors
/// Returns [`CorrelationIdRejection`] when the value is empty, too long,
/// or contains disallowed characters.
pub fn sanitize_client_correlation_id(
    value: Option<&str>,
) -> Result<Option<String>, CorrelationIdRejection> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CorrelationIdRejection::EmptyAfterTrim);
    }
    if trimmed.len() > MAX_CLIENT_CORRELATION_ID_LENGTH {
        return Err(CorrelationIdRejection::TooLong);
    }
    for ch in trimmed.chars() {
        if !ch.is_ascii() {
            return Err(CorrelationIdRejection::NonAscii);
        }
        if ch.is_ascii_whitespace() {
            return Err(CorrelationIdRejection::ContainsWhitespace);
        }
        if ch.is_control() {
            return Err(CorrelationIdRejection::ContainsControlChar);
        }
        if !is_tchar(ch) {
            return Err(CorrelationIdRejection::ContainsDisallowedChar);
        }
    }
    Ok(Some(trimmed.to_string()))
}

/// Returns true when the character is a valid HTTP token character.
const fn is_tchar(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '!' | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '*'
                | '+'
                | '-'
                | '.'
                | '^'
                | '_'
                | '`'
                | '|'
                | '~'
        )
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod tests;
