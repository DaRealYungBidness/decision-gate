// decision-gate-core/src/core/evidence.rs
// ============================================================================
// Module: Decision Gate Evidence Model
// Description: Evidence queries, results, and comparators for gate evaluation.
// Purpose: Provide backend-agnostic evidence contracts for Decision Gate gates.
// Dependencies: crate::core::hashing, serde, serde_json
// ============================================================================

//! ## Overview
//! Evidence queries describe the information needed to evaluate conditions.
//! Evidence results include hashes, anchors, and references suitable for
//! offline verification. The Decision Gate runtime applies comparators to evidence
//! values to derive condition truth values.
//!
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::core::hashing::HashDigest;
use crate::core::identifiers::ProviderId;

// ============================================================================
// SECTION: Evidence Queries
// ============================================================================

/// Canonical evidence query supported by Decision Gate.
///
/// # Invariants
/// - `provider_id` and `check_id` must be non-empty after spec validation.
/// - `params` is optional and unvalidated at this layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceQuery {
    /// Evidence provider identifier.
    pub provider_id: ProviderId,
    /// Provider check identifier.
    pub check_id: String,
    /// Structured parameters for the provider check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

// ============================================================================
// SECTION: Comparators
// ============================================================================

/// Comparator applied to evidence values.
///
/// # Invariants
/// - Variants are stable for serialization and contract matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
    /// Lexicographic greater-than comparison for strings.
    LexGreaterThan,
    /// Lexicographic greater-than-or-equal comparison for strings.
    LexGreaterThanOrEqual,
    /// Lexicographic less-than comparison for strings.
    LexLessThan,
    /// Lexicographic less-than-or-equal comparison for strings.
    LexLessThanOrEqual,
    /// String containment comparison.
    Contains,
    /// Membership in an expected set.
    InSet,
    /// Deep equality comparison for arrays/objects.
    DeepEquals,
    /// Deep inequality comparison for arrays/objects.
    DeepNotEquals,
    /// Evidence exists (value must be present).
    Exists,
    /// Evidence does not exist (value must be absent).
    NotExists,
}

// ============================================================================
// SECTION: Trust Lanes
// ============================================================================

/// Evidence trust lane classification.
///
/// # Invariants
/// - `Verified` is strictly more trusted than `Asserted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustLane {
    /// Evidence was pulled from a provider (verified lane).
    #[default]
    Verified,
    /// Evidence was asserted by a client (unverified lane).
    Asserted,
}

impl TrustLane {
    /// Returns true when this lane is at least as strict as the requirement.
    #[must_use]
    pub const fn satisfies(self, requirement: TrustRequirement) -> bool {
        self.rank() >= requirement.min_lane.rank()
    }

    /// Returns the lane ordering (higher is stricter).
    const fn rank(self) -> u8 {
        match self {
            Self::Asserted => 0,
            Self::Verified => 1,
        }
    }
}

/// Trust requirement for evidence usage.
///
/// # Invariants
/// - `min_lane` is the minimum acceptable trust level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRequirement {
    /// Minimum acceptable lane for evidence.
    pub min_lane: TrustLane,
}

impl Default for TrustRequirement {
    fn default() -> Self {
        Self {
            min_lane: TrustLane::Verified,
        }
    }
}

impl TrustRequirement {
    /// Returns the stricter of two trust requirements.
    #[must_use]
    pub const fn stricter(self, other: Self) -> Self {
        if self.min_lane.rank() >= other.min_lane.rank() { self } else { other }
    }
}

// ============================================================================
// SECTION: Evidence Values
// ============================================================================

/// Evidence payload value used for condition comparison.
///
/// # Invariants
/// - JSON values are compared using canonical rules in the control plane.
/// - Byte payloads are opaque and unstructured at this layer.
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
///
/// # Invariants
/// - `anchor_type` is a stable identifier within a provider domain.
/// - `anchor_value` is opaque unless validated against an anchor policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAnchor {
    /// Anchor type identifier (`snapshot`, `log_offset`, `receipt_id`, etc.).
    pub anchor_type: String,
    /// Anchor value.
    pub anchor_value: String,
}

/// Reference to an evidence artifact in an external system or runpack.
///
/// # Invariants
/// - `uri` is opaque and may be runpack-relative or external.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Reference URI or runpack-relative path.
    pub uri: String,
}

/// Optional evidence signature metadata.
///
/// # Invariants
/// - Signature bytes are opaque; verification is performed elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSignature {
    /// Signature scheme identifier.
    pub scheme: String,
    /// Identifier for the signing key.
    pub key_id: String,
    /// Signature bytes.
    pub signature: Vec<u8>,
}

// ============================================================================
// SECTION: Evidence Results
// ============================================================================

/// Evidence provider error metadata recorded for audit trails.
///
/// # Invariants
/// - `code` is a stable provider-specific identifier.
/// - `message` should be safe for logs; evidence payloads must not appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProviderError {
    /// Stable error code string.
    pub code: String,
    /// Provider error message.
    pub message: String,
    /// Optional structured error details for recovery.
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

/// Evidence result returned by providers.
///
/// # Invariants
/// - When `error` is set, `value` and `evidence_hash` should be `None`.
/// - When produced by the control plane, `evidence_hash` matches the canonical hash of `value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceResult {
    /// Evidence payload value, if available.
    pub value: Option<EvidenceValue>,
    /// Trust lane classification for the evidence.
    #[serde(default)]
    pub lane: TrustLane,
    /// Optional provider error metadata when evidence is invalid or missing.
    #[serde(default)]
    pub error: Option<EvidenceProviderError>,
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

// ============================================================================
// SECTION: Anchor Policy
// ============================================================================

/// Anchor requirements for evidence providers.
///
/// # Invariants
/// - `anchor_type` must match the expected provider anchor type when enforced.
/// - `required_fields` are keys expected in the anchor payload object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRequirement {
    /// Anchor type identifier expected on evidence results.
    pub anchor_type: String,
    /// Required fields inside the anchor payload.
    pub required_fields: Vec<String>,
}

/// Provider-specific anchor policy.
///
/// # Invariants
/// - `provider_id` identifies the provider this policy applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAnchorPolicy {
    /// Provider identifier enforced by this policy.
    pub provider_id: ProviderId,
    /// Anchor requirements for the provider.
    pub requirement: AnchorRequirement,
}

/// Evidence anchor policy applied by the control plane and runpack verifier.
///
/// # Invariants
/// - Empty policies impose no anchor requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceAnchorPolicy {
    /// Provider-specific anchor requirements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<ProviderAnchorPolicy>,
}

impl EvidenceAnchorPolicy {
    /// Returns the anchor requirement for a provider, if configured.
    #[must_use]
    pub fn requirement_for(&self, provider_id: &ProviderId) -> Option<&AnchorRequirement> {
        self.providers
            .iter()
            .find(|policy| policy.provider_id == *provider_id)
            .map(|policy| &policy.requirement)
    }
}

// ============================================================================
// SECTION: Provider Validation
// ============================================================================

/// Provider-missing diagnostics returned by evidence registries.
///
/// # Invariants
/// - `missing_providers` lists required provider identifiers that are unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMissingError {
    /// Provider identifiers required by the scenario but not registered.
    pub missing_providers: Vec<String>,
    /// Capabilities required by the checks when providers are present.
    pub required_capabilities: Vec<String>,
    /// Indicates a policy block (true when a provider is present but disallowed).
    pub blocked_by_policy: bool,
}
