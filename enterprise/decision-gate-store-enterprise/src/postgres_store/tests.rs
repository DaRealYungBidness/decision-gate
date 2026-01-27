// enterprise/decision-gate-store-enterprise/src/postgres_store/tests.rs
// ============================================================================
// Module: Postgres Store Unit Tests
// Description: Unit tests for cursor encoding/decoding helpers.
// Purpose: Validate cursor handling and hash labels without a live DB.
// ============================================================================

#![allow(clippy::expect_used, reason = "Unit tests use expect for setup clarity.")]

use decision_gate_core::DataShapeRegistryError;
use decision_gate_core::hashing::HashAlgorithm;

use super::PostgresStore;
use super::hash_algorithm_label;

#[test]
fn postgres_store_cursor_roundtrip() {
    let cursor = PostgresStore::encode_cursor("schema-1", "1.2.3");
    let decoded = PostgresStore::decode_cursor(&cursor).expect("decode cursor");
    assert_eq!(decoded.schema_id, "schema-1");
    assert_eq!(decoded.version, "1.2.3");
}

#[test]
fn postgres_store_decode_cursor_rejects_invalid() {
    let result = PostgresStore::decode_cursor("not-json");
    assert!(matches!(result, Err(DataShapeRegistryError::Invalid(_))));
}

#[test]
fn postgres_store_hash_algorithm_label_matches() {
    assert_eq!(hash_algorithm_label(HashAlgorithm::Sha256), "sha256");
}

#[test]
fn postgres_store_decode_cursor_rejects_missing_fields() {
    let result = PostgresStore::decode_cursor("{\"schema_id\":\"schema-1\"}");
    assert!(matches!(result, Err(DataShapeRegistryError::Invalid(_))));
}
