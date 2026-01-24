// decision-gate-core/src/core/hashing.rs
// ============================================================================
// Module: Decision Gate Canonical Hashing
// Description: RFC 8785 JSON canonicalization and content hashing utilities.
// Purpose: Provide deterministic hashes for specs, logs, and runpack artifacts.
// Dependencies: serde, serde_jcs, sha2
// ============================================================================

//! ## Overview
//! Decision Gate hashes all canonical JSON using RFC 8785 (JCS) to guarantee stable,
//! replayable digests. Binary payloads are hashed directly over raw bytes.
//!
//! Security posture: hashing is part of audit integrity; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io;
use std::io::Write;

use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use thiserror::Error;

// ============================================================================
// SECTION: Hash Algorithm
// ============================================================================

/// Supported hash algorithms for Decision Gate artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    /// SHA-256 hashing (FIPS-friendly default).
    Sha256,
}

/// Default hash algorithm for Decision Gate.
pub const DEFAULT_HASH_ALGORITHM: HashAlgorithm = HashAlgorithm::Sha256;

// ============================================================================
// SECTION: Hash Digest
// ============================================================================

/// Deterministic content hash representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashDigest {
    /// Hash algorithm identifier.
    pub algorithm: HashAlgorithm,
    /// Lowercase hex-encoded digest bytes.
    pub value: String,
}

impl HashDigest {
    /// Creates a new digest from raw bytes.
    #[must_use]
    pub fn new(algorithm: HashAlgorithm, bytes: &[u8]) -> Self {
        Self {
            algorithm,
            value: hex_encode(bytes),
        }
    }
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Errors raised when computing canonical hashes.
#[derive(Debug, Error)]
pub enum HashError {
    /// JSON canonicalization failed.
    #[error("failed to canonicalize json: {0}")]
    Canonicalization(String),
    /// Canonical JSON exceeds the configured size limit.
    #[error("canonical json exceeds size limit ({actual} > {limit})")]
    SizeLimitExceeded {
        /// Maximum allowed bytes.
        limit: usize,
        /// Observed size in bytes.
        actual: usize,
    },
}

// ============================================================================
// SECTION: Hashing Helpers
// ============================================================================

/// Returns canonical JSON bytes for a serializable value using RFC 8785.
///
/// # Errors
///
/// Returns [`HashError::Canonicalization`] when serialization fails.
pub fn canonical_json_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, HashError> {
    serde_jcs::to_vec(value).map_err(|err| HashError::Canonicalization(err.to_string()))
}

/// Returns canonical JSON bytes using RFC 8785 with a size limit.
///
/// # Errors
///
/// Returns [`HashError::Canonicalization`] when serialization fails.
/// Returns [`HashError::SizeLimitExceeded`] when the encoded JSON exceeds `max_bytes`.
pub fn canonical_json_bytes_with_limit<T: Serialize + ?Sized>(
    value: &T,
    max_bytes: usize,
) -> Result<Vec<u8>, HashError> {
    let mut out = Vec::new();
    let mut writer = LimitedWriter::new(&mut out, max_bytes);
    match serde_jcs::to_writer(&mut writer, value) {
        Ok(()) => Ok(out),
        Err(err) => {
            if let Some(actual) = writer.limit_hit() {
                return Err(HashError::SizeLimitExceeded {
                    limit: max_bytes,
                    actual,
                });
            }
            Err(HashError::Canonicalization(err.to_string()))
        }
    }
}

/// Hashes canonical JSON using the provided algorithm.
///
/// # Errors
///
/// Returns [`HashError::Canonicalization`] when serialization fails.
pub fn hash_canonical_json<T: Serialize + ?Sized>(
    algorithm: HashAlgorithm,
    value: &T,
) -> Result<HashDigest, HashError> {
    let bytes = canonical_json_bytes(value)?;
    Ok(hash_bytes(algorithm, &bytes))
}

/// Hashes canonical JSON using the provided algorithm with a size limit.
///
/// # Errors
///
/// Returns [`HashError::Canonicalization`] when serialization fails.
/// Returns [`HashError::SizeLimitExceeded`] when the encoded JSON exceeds `max_bytes`.
pub fn hash_canonical_json_with_limit<T: Serialize + ?Sized>(
    algorithm: HashAlgorithm,
    value: &T,
    max_bytes: usize,
) -> Result<HashDigest, HashError> {
    let bytes = canonical_json_bytes_with_limit(value, max_bytes)?;
    Ok(hash_bytes(algorithm, &bytes))
}

/// Hashes raw bytes using the provided algorithm.
#[must_use]
pub fn hash_bytes(algorithm: HashAlgorithm, bytes: &[u8]) -> HashDigest {
    match algorithm {
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(bytes);
            let digest = hasher.finalize();
            HashDigest::new(HashAlgorithm::Sha256, &digest)
        }
    }
}

// ============================================================================
// SECTION: Hex Encoding
// ============================================================================

/// Encodes bytes as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let hi = usize::from(byte >> 4);
        let lo = usize::from(byte & 0x0f);
        out.push(char::from(HEX[hi]));
        out.push(char::from(HEX[lo]));
    }
    out
}

// ============================================================================
// SECTION: Size-Limited Writer
// ============================================================================

/// Writer that fails when a size limit would be exceeded.
struct LimitedWriter<'a> {
    /// Output buffer for encoded bytes.
    inner: &'a mut Vec<u8>,
    /// Maximum allowed byte count.
    max_bytes: usize,
    /// Bytes written so far.
    written: usize,
    /// Total bytes attempted when limit was hit.
    limit_hit: Option<usize>,
}

impl<'a> LimitedWriter<'a> {
    /// Creates a limited writer over the provided buffer.
    const fn new(inner: &'a mut Vec<u8>, max_bytes: usize) -> Self {
        Self {
            inner,
            max_bytes,
            written: 0,
            limit_hit: None,
        }
    }

    /// Returns the attempted size when the limit was exceeded.
    const fn limit_hit(&self) -> Option<usize> {
        self.limit_hit
    }
}

impl Write for LimitedWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let remaining = self.max_bytes.saturating_sub(self.written);
        if buf.len() > remaining {
            self.limit_hit = Some(self.written.saturating_add(buf.len()));
            return Err(io::Error::other("size limit exceeded"));
        }
        self.inner.extend_from_slice(buf);
        self.written = self.written.saturating_add(buf.len());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
