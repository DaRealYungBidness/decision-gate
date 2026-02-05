// crates/decision-gate-core/src/core/providers.rs
// ============================================================================
// Module: Built-in Provider Identifiers
// Description: Canonical identifiers reserved for built-in evidence providers.
// Purpose: Centralize builtin provider IDs for config validation and registry checks.
// Dependencies: none
// ============================================================================

//! ## Overview
//! Canonical identifiers reserved for built-in evidence providers.
//! Invariants:
//! - Identifiers are lowercase ASCII strings.
//! - Identifiers remain stable for config and contract validation.
//!
//! Security posture: provider identifiers are treated as untrusted input.
//! See `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Built-in Provider Identifiers
// ============================================================================

/// Reserved identifiers for built-in providers.
///
/// # Invariants
/// - Identifiers are lowercase ASCII strings.
/// - Identifiers remain stable for config and contract validation.
pub const BUILTIN_PROVIDER_IDS: [&str; 4] = ["time", "env", "json", "http"];

/// Returns true when the identifier is reserved for a built-in provider.
#[must_use]
pub fn is_builtin_provider_id(provider_id: &str) -> bool {
    BUILTIN_PROVIDER_IDS.iter().any(|id| id == &provider_id)
}
