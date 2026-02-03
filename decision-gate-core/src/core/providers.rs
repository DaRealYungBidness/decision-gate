// decision-gate-core/src/core/providers.rs
// ============================================================================
// Module: Built-in Provider Identifiers
// Description: Canonical identifiers reserved for built-in evidence providers.
// Purpose: Centralize builtin provider IDs for config validation and registry checks.
// Dependencies: none
// ============================================================================

//! Canonical identifiers for built-in evidence providers.

/// Reserved identifiers for built-in providers.
pub const BUILTIN_PROVIDER_IDS: [&str; 4] = ["time", "env", "json", "http"];

/// Returns true when the identifier is reserved for a built-in provider.
#[must_use]
pub fn is_builtin_provider_id(provider_id: &str) -> bool {
    BUILTIN_PROVIDER_IDS.iter().any(|id| id == &provider_id)
}

#[cfg(test)]
mod tests {
    use super::BUILTIN_PROVIDER_IDS;
    use super::is_builtin_provider_id;

    #[test]
    fn builtin_provider_ids_include_expected_values() {
        for id in BUILTIN_PROVIDER_IDS {
            assert!(is_builtin_provider_id(id));
        }
        assert!(!is_builtin_provider_id("external"));
    }
}
