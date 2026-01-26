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
use crate::core::identifiers::ProviderId;

// ============================================================================
// SECTION: Evidence Queries
// ============================================================================

/// Canonical evidence query supported by Decision Gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceQuery {
    /// Evidence provider identifier.
    pub provider_id: ProviderId,
    /// Provider predicate or method name.
    pub predicate: String,
    /// Structured parameters for the provider predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

// ============================================================================
// SECTION: Comparators
// ============================================================================

/// Comparator applied to evidence values.
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
    /// Identifier for the signing key.
    pub key_id: String,
    /// Signature bytes.
    pub signature: Vec<u8>,
}

// ============================================================================
// SECTION: Evidence Results
// ============================================================================

/// Evidence provider error metadata recorded for audit trails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProviderError {
    /// Stable error code string.
    pub code: String,
    /// Provider error message.
    pub message: String,
}

/// Evidence result returned by providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceResult {
    /// Evidence payload value, if available.
    pub value: Option<EvidenceValue>,
    /// Trust lane classification for the evidence.
    #[serde(default)]
    pub lane: TrustLane,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRequirement {
    /// Anchor type identifier expected on evidence results.
    pub anchor_type: String,
    /// Required fields inside the anchor payload.
    pub required_fields: Vec<String>,
}

/// Provider-specific anchor policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAnchorPolicy {
    /// Provider identifier enforced by this policy.
    pub provider_id: ProviderId,
    /// Anchor requirements for the provider.
    pub requirement: AnchorRequirement,
}

/// Evidence anchor policy applied by the control plane and runpack verifier.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMissingError {
    /// Provider identifiers required by the scenario but not registered.
    pub missing_providers: Vec<String>,
    /// Capabilities required by the predicates when providers are present.
    pub required_capabilities: Vec<String>,
    /// Indicates a policy block (true when a provider is present but disallowed).
    pub blocked_by_policy: bool,
}
