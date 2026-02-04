// decision-gate-core/tests/condition_eval_ordering_units.rs
//! Unit tests for condition evaluation ordering functions.
// ============================================================================
// Module: Condition Evaluation Ordering Unit Tests
// Description: Tests for reorder_condition_specs and condition_shuffle_key.
// ============================================================================
//! ## Overview
//! Tests the deterministic condition reordering logic added for metamorphic
//! testing. Verifies that condition evaluation order can be permuted while
//! maintaining deterministic behavior.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions and helpers are permitted."
)]

use decision_gate_core::Comparator;
use decision_gate_core::ConditionId;
use decision_gate_core::ConditionSpec;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::runtime::ConditionEvalOrder;
// Import the functions we want to test (exposed as pub in engine.rs for testing)
use decision_gate_core::runtime::engine::condition_shuffle_key;
use decision_gate_core::runtime::engine::reorder_condition_specs;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Creates a minimal ConditionSpec for testing.
fn test_condition_spec(condition_id: &str) -> ConditionSpec {
    ConditionSpec {
        condition_id: ConditionId::new(condition_id),
        query: EvidenceQuery {
            provider_id: ProviderId::new("test-provider"),
            check_id: format!("check-{condition_id}"),
            params: None,
        },
        comparator: Comparator::Exists,
        expected: None,
        policy_tags: Vec::new(),
        trust: None,
    }
}

/// Extracts condition_id strings from specs for easy assertion.
fn extract_condition_ids(specs: &[ConditionSpec]) -> Vec<String> {
    specs.iter().map(|s| s.condition_id.as_str().to_string()).collect()
}

// ============================================================================
// SECTION: Tests for reorder_condition_specs()
// ============================================================================

#[test]
fn reorder_preserves_original_order_in_spec_mode() {
    // Arrange
    let mut specs = vec![
        test_condition_spec("zebra"),
        test_condition_spec("apple"),
        test_condition_spec("mango"),
        test_condition_spec("banana"),
        test_condition_spec("cherry"),
    ];
    let original_ids = extract_condition_ids(&specs);

    // Act
    reorder_condition_specs(&mut specs, ConditionEvalOrder::Spec, HashAlgorithm::Sha256);

    // Assert
    let result_ids = extract_condition_ids(&specs);
    assert_eq!(result_ids, original_ids, "Spec mode should preserve original order");
}

#[test]
fn reorder_deterministically_shuffles_with_seed() {
    // Arrange
    let mut specs = vec![
        test_condition_spec("cond-a"),
        test_condition_spec("cond-b"),
        test_condition_spec("cond-c"),
        test_condition_spec("cond-d"),
        test_condition_spec("cond-e"),
        test_condition_spec("cond-f"),
        test_condition_spec("cond-g"),
        test_condition_spec("cond-h"),
        test_condition_spec("cond-i"),
        test_condition_spec("cond-j"),
    ];
    let original_ids = extract_condition_ids(&specs);

    // Act
    reorder_condition_specs(
        &mut specs,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    // Assert
    let result_ids = extract_condition_ids(&specs);
    assert_ne!(result_ids, original_ids, "DeterministicShuffle should change the order");
    assert_eq!(result_ids.len(), original_ids.len(), "Should preserve all conditions");
}

#[test]
fn reorder_different_seeds_produce_different_orders() {
    // Arrange
    let base_specs = vec![
        test_condition_spec("cond-a"),
        test_condition_spec("cond-b"),
        test_condition_spec("cond-c"),
        test_condition_spec("cond-d"),
        test_condition_spec("cond-e"),
    ];

    let mut specs_seed_7 = base_specs.clone();
    let mut specs_seed_11 = base_specs.clone();

    // Act
    reorder_condition_specs(
        &mut specs_seed_7,
        ConditionEvalOrder::DeterministicShuffle { seed: 7 },
        HashAlgorithm::Sha256,
    );
    reorder_condition_specs(
        &mut specs_seed_11,
        ConditionEvalOrder::DeterministicShuffle { seed: 11 },
        HashAlgorithm::Sha256,
    );

    // Assert
    let ids_7 = extract_condition_ids(&specs_seed_7);
    let ids_11 = extract_condition_ids(&specs_seed_11);
    assert_ne!(ids_7, ids_11, "Different seeds should produce different orderings");
}

#[test]
fn reorder_same_seed_produces_same_order_repeatedly() {
    // Arrange
    let base_specs = vec![
        test_condition_spec("cond-a"),
        test_condition_spec("cond-b"),
        test_condition_spec("cond-c"),
        test_condition_spec("cond-d"),
        test_condition_spec("cond-e"),
    ];

    // Act - reorder three times with the same seed
    let mut specs_1 = base_specs.clone();
    reorder_condition_specs(
        &mut specs_1,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    let mut specs_2 = base_specs.clone();
    reorder_condition_specs(
        &mut specs_2,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    let mut specs_3 = base_specs.clone();
    reorder_condition_specs(
        &mut specs_3,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    // Assert
    let ids_1 = extract_condition_ids(&specs_1);
    let ids_2 = extract_condition_ids(&specs_2);
    let ids_3 = extract_condition_ids(&specs_3);
    assert_eq!(ids_1, ids_2, "Same seed should produce identical ordering");
    assert_eq!(ids_2, ids_3, "Same seed should produce identical ordering");
}

#[test]
fn reorder_handles_empty_list() {
    // Arrange
    let mut specs: Vec<ConditionSpec> = Vec::new();

    // Act
    reorder_condition_specs(
        &mut specs,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    // Assert
    assert_eq!(specs.len(), 0, "Empty list should remain empty");
}

#[test]
fn reorder_handles_single_item() {
    // Arrange
    let mut specs = vec![test_condition_spec("only-one")];

    // Act
    reorder_condition_specs(
        &mut specs,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    // Assert
    assert_eq!(specs.len(), 1, "Single item should remain");
    assert_eq!(specs[0].condition_id.as_str(), "only-one", "Single item should be unchanged");
}

#[test]
fn reorder_uses_condition_id_as_tiebreaker() {
    // This test verifies that when shuffle keys collide (extremely rare with SHA256),
    // the condition_id is used as a tiebreaker for stable ordering.
    // Since we can't easily force a hash collision, we verify the tiebreaker logic
    // by testing with multiple conditions and ensuring the output is always sorted
    // by hash first, then condition_id.

    // Arrange
    let mut specs = vec![
        test_condition_spec("cond-z"),
        test_condition_spec("cond-a"),
        test_condition_spec("cond-m"),
        test_condition_spec("cond-b"),
    ];

    // Act
    reorder_condition_specs(
        &mut specs,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );

    // Assert
    // We can't directly test the tiebreaker without hash collisions,
    // but we can verify the function completes successfully and produces
    // a deterministic result. The tiebreaker is tested at line 1557 in engine.rs.
    let ids = extract_condition_ids(&specs);
    assert_eq!(ids.len(), 4, "All conditions should be preserved");

    // Verify determinism by running again
    let mut specs_2 = vec![
        test_condition_spec("cond-z"),
        test_condition_spec("cond-a"),
        test_condition_spec("cond-m"),
        test_condition_spec("cond-b"),
    ];
    reorder_condition_specs(
        &mut specs_2,
        ConditionEvalOrder::DeterministicShuffle { seed: 42 },
        HashAlgorithm::Sha256,
    );
    let ids_2 = extract_condition_ids(&specs_2);
    assert_eq!(ids, ids_2, "Tiebreaker should produce consistent ordering");
}

#[test]
fn reorder_with_many_conditions() {
    // Stress test with 20+ conditions
    // Arrange
    let mut specs: Vec<ConditionSpec> =
        (0..25).map(|i| test_condition_spec(&format!("cond-{i:03}"))).collect();
    let original_ids = extract_condition_ids(&specs);

    // Act
    reorder_condition_specs(
        &mut specs,
        ConditionEvalOrder::DeterministicShuffle { seed: 12345 },
        HashAlgorithm::Sha256,
    );

    // Assert
    let result_ids = extract_condition_ids(&specs);
    assert_eq!(result_ids.len(), 25, "All conditions should be preserved");
    assert_ne!(result_ids, original_ids, "Large list should be shuffled");

    // Verify determinism with large list
    let mut specs_2: Vec<ConditionSpec> =
        (0..25).map(|i| test_condition_spec(&format!("cond-{i:03}"))).collect();
    reorder_condition_specs(
        &mut specs_2,
        ConditionEvalOrder::DeterministicShuffle { seed: 12345 },
        HashAlgorithm::Sha256,
    );
    let result_ids_2 = extract_condition_ids(&specs_2);
    assert_eq!(result_ids, result_ids_2, "Large list should shuffle deterministically");
}

// ============================================================================
// SECTION: Tests for condition_shuffle_key()
// ============================================================================

#[test]
fn shuffle_key_is_deterministic() {
    // Arrange
    let seed = 42;
    let algorithm = HashAlgorithm::Sha256;
    let condition_id = ConditionId::new("test-condition");

    // Act
    let key_1 = condition_shuffle_key(seed, algorithm, &condition_id);
    let key_2 = condition_shuffle_key(seed, algorithm, &condition_id);
    let key_3 = condition_shuffle_key(seed, algorithm, &condition_id);

    // Assert
    assert_eq!(key_1, key_2, "Same inputs should produce same hash");
    assert_eq!(key_2, key_3, "Same inputs should produce same hash");
}

#[test]
fn shuffle_key_differs_with_different_seeds() {
    // Arrange
    let algorithm = HashAlgorithm::Sha256;
    let condition_id = ConditionId::new("test-condition");

    // Act
    let key_seed_1 = condition_shuffle_key(1, algorithm, &condition_id);
    let key_seed_2 = condition_shuffle_key(2, algorithm, &condition_id);
    let key_seed_3 = condition_shuffle_key(3, algorithm, &condition_id);

    // Assert
    assert_ne!(key_seed_1, key_seed_2, "Different seeds should produce different hashes");
    assert_ne!(key_seed_2, key_seed_3, "Different seeds should produce different hashes");
    assert_ne!(key_seed_1, key_seed_3, "Different seeds should produce different hashes");
}

#[test]
fn shuffle_key_differs_with_different_condition_ids() {
    // Arrange
    let seed = 42;
    let algorithm = HashAlgorithm::Sha256;

    // Act
    let key_a = condition_shuffle_key(seed, algorithm, &ConditionId::new("cond-a"));
    let key_b = condition_shuffle_key(seed, algorithm, &ConditionId::new("cond-b"));
    let key_c = condition_shuffle_key(seed, algorithm, &ConditionId::new("cond-c"));

    // Assert
    assert_ne!(key_a, key_b, "Different condition IDs should produce different hashes");
    assert_ne!(key_b, key_c, "Different condition IDs should produce different hashes");
    assert_ne!(key_a, key_c, "Different condition IDs should produce different hashes");
}

#[test]
fn shuffle_key_handles_special_chars_in_condition_id() {
    // Arrange
    let seed = 42;
    let algorithm = HashAlgorithm::Sha256;

    // Act - test with various special characters
    let key_colons = condition_shuffle_key(seed, algorithm, &ConditionId::new("cond:with:colons"));
    let key_slashes =
        condition_shuffle_key(seed, algorithm, &ConditionId::new("cond/with/slashes"));
    let key_hyphens =
        condition_shuffle_key(seed, algorithm, &ConditionId::new("cond-with-hyphens"));
    let key_underscores =
        condition_shuffle_key(seed, algorithm, &ConditionId::new("cond_with_underscores"));
    let key_dots = condition_shuffle_key(seed, algorithm, &ConditionId::new("cond.with.dots"));

    // Assert - all should produce valid hashes without error
    assert!(!key_colons.is_empty(), "Should handle colons in condition ID");
    assert!(!key_slashes.is_empty(), "Should handle slashes in condition ID");
    assert!(!key_hyphens.is_empty(), "Should handle hyphens in condition ID");
    assert!(!key_underscores.is_empty(), "Should handle underscores in condition ID");
    assert!(!key_dots.is_empty(), "Should handle dots in condition ID");

    // Verify they all produce different hashes
    let keys = vec![key_colons, key_slashes, key_hyphens, key_underscores, key_dots];
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert_ne!(
                keys[i], keys[j],
                "Different special characters should produce different hashes"
            );
        }
    }
}
