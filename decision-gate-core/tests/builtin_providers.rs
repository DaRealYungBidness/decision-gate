// decision-gate-core/tests/builtin_providers.rs
// ============================================================================
// Module: Built-in Provider Identifier Tests
// Description: Unit tests for built-in provider ID helpers.
// Purpose: Validate reserved provider ID list and matching helper behavior.
// Dependencies: decision-gate-core
// ============================================================================

//! ## Overview
//! Validates the behavior and invariants of built-in provider identifiers.

// ============================================================================
// SECTION: Tests
// ============================================================================

use decision_gate_core::BUILTIN_PROVIDER_IDS;
use decision_gate_core::is_builtin_provider_id;

#[test]
fn builtin_provider_ids_include_expected_values() {
    for id in BUILTIN_PROVIDER_IDS {
        assert!(is_builtin_provider_id(id));
    }
    assert!(!is_builtin_provider_id("external"));
    assert!(!is_builtin_provider_id("TIME"));
}
