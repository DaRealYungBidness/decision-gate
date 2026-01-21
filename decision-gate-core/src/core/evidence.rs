// decision-gate-core/src/core/evidence.rs
// ============================================================================
// Module: Decision Gate Evidence Model
// Description: Evidence queries, results, and comparators for gate evaluation.
// Purpose: Provide backend-agnostic evidence contracts for Decision Gate gates.
// Dependencies: crate::core::hashing, serde, serde_json
// ============================================================================

//! ## Overview
//! Evidence queries describe the information needed to evaluate predicates.
//! Evidence results include hashes, anchors, and references suitable for
//! offline verification. The Decision Gate runtime applies comparators to evidence
//! values to derive predicate truth values.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::core::hashing::HashDigest;

// ============================================================================
// SECTION: Evidence Queries
// ============================================================================

/// Canonical evidence query shapes supported by Decision Gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceQuery {
    /// Query a named state predicate with structured parameters.
    StatePredicate {
        /// Predicate name within the backend domain.
        name: String,
        /// Backend-specific parameters (must be canonicalizable JSON).
        params: Value,
    },
    /// Query for evidence in a log stream.
    LogContains {
        /// Log stream identifier.
        stream: String,
        /// Structured filter describing the log predicate.
        filter: Value,
    },
    /// Query for evidence in a commit or event stream.
    CommitContains {
        /// Structured filter describing the commit predicate.
        filter: Value,
    },
    /// Query for a specific receipt by system and identifier.
    Receipt {
        /// External system name.
        system: String,
        /// Receipt identifier.
        id: String,
    },
    /// Backend-specific opaque query for adapter extensions.
    Custom {
        /// Adapter key that owns the query format.
        adapter_key: String,
        /// Opaque serialized query payload.
        bytes: Vec<u8>,
    },
}

// ============================================================================
// SECTION: Comparators
// ============================================================================

/// Comparator applied to evidence values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    /// Value equality comparison.
    Equals,
    /// Value inequality comparison.
    NotEquals,
    /// Numeric greater-than comparison.
    GreaterThan,
    /// Numeric greater-than-or-equal comparison.
    GreaterThanOrEqual,
    /// Numeric less-than comparison.
    LessThan,
    /// Numeric less-than-or-equal comparison.
    LessThanOrEqual,
    /// String containment comparison.
    Contains,
    /// Membership in an expected set.
    InSet,
    /// Evidence exists (value must be present).
    Exists,
    /// Evidence does not exist (value must be absent).
    NotExists,
}

// ============================================================================
// SECTION: Evidence Values
// ============================================================================

/// Evidence payload value used for predicate comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EvidenceValue {
    /// Canonical JSON value.
    Json(Value),
    /// Raw bytes value for binary evidence.
    Bytes(Vec<u8>),
}

// ============================================================================
// SECTION: Evidence Anchors
// ============================================================================

/// Stable anchor used to identify evidence locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAnchor {
    /// Anchor type identifier (`snapshot`, `log_offset`, `receipt_id`, etc.).
    pub anchor_type: String,
    /// Anchor value.
    pub anchor_value: String,
}

/// Reference to an evidence artifact in an external system or runpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Reference URI or runpack-relative path.
    pub uri: String,
}

/// Optional evidence signature metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSignature {
    /// Signature scheme identifier.
    pub scheme: String,
    /// Signature bytes.
    pub signature: Vec<u8>,
}

// ============================================================================
// SECTION: Evidence Results
// ============================================================================

/// Evidence result returned by providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceResult {
    /// Evidence payload value, if available.
    pub value: Option<EvidenceValue>,
    /// Canonical hash of the evidence payload.
    pub evidence_hash: Option<HashDigest>,
    /// Reference to the evidence artifact.
    pub evidence_ref: Option<EvidenceRef>,
    /// Stable evidence anchor for offline verification.
    pub evidence_anchor: Option<EvidenceAnchor>,
    /// Optional signature metadata.
    pub signature: Option<EvidenceSignature>,
    /// Content type of the evidence payload when present.
    pub content_type: Option<String>,
}
