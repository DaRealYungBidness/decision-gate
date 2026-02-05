// crates/decision-gate-core/src/core/summary.rs
// ============================================================================
// Module: Decision Gate Safe Summaries
// Description: Redacted, policy-safe summaries for client-facing responses.
// Purpose: Prevent evidence leakage while communicating gate status.
// Dependencies: crate::core::identifiers, serde
// ============================================================================

//! ## Overview
//! Safe summaries provide minimal, policy-safe status for clients without
//! leaking sensitive evidence values. They surface unmet gate identifiers and
//! retry guidance while preserving confidentiality.
//!
//! Security posture: summaries must avoid leaking evidence; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;

use crate::core::identifiers::GateId;

// ============================================================================
// SECTION: Safe Summary
// ============================================================================

/// Safe summary returned to clients when gates are unmet.
///
/// # Invariants
/// - Contains only safe, redacted status data (no evidence payloads).
/// - Strings are opaque and not normalized by this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeSummary {
    /// Summary status string.
    pub status: String,
    /// Gate identifiers that are not yet satisfied.
    pub unmet_gates: Vec<GateId>,
    /// Optional retry guidance.
    pub retry_hint: Option<String>,
    /// Optional policy tags for the summary.
    pub policy_tags: Vec<String>,
}

impl SafeSummary {
    /// Creates a safe summary with the provided status and unmet gates.
    #[must_use]
    pub fn new(status: impl Into<String>, unmet_gates: Vec<GateId>) -> Self {
        Self {
            status: status.into(),
            unmet_gates,
            retry_hint: None,
            policy_tags: Vec::new(),
        }
    }
}
