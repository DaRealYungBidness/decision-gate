// enterprise/decision-gate-store-enterprise/tests/postgres_store.rs
// ============================================================================
// Module: Postgres Store Tests
// Description: Unit tests for Postgres store configuration and helpers.
// Purpose: Validate error handling without a live database.
// ============================================================================

//! Postgres store unit tests.

use decision_gate_store_enterprise::postgres_store::PostgresStoreConfig;
use decision_gate_store_enterprise::postgres_store::shared_postgres_store;

#[test]
fn postgres_store_default_config_is_valid_shape() {
    let config = PostgresStoreConfig::default();
    assert!(!config.connection.is_empty());
    assert!(config.max_connections > 0);
    assert!(config.connect_timeout_ms > 0);
    assert!(config.statement_timeout_ms > 0);
}

#[test]
fn postgres_store_invalid_connection_string_fails() {
    let config = PostgresStoreConfig {
        connection: "not-a-url".to_string(),
        max_connections: 1,
        connect_timeout_ms: 1,
        statement_timeout_ms: 1,
    };
    let result = shared_postgres_store(&config);
    assert!(result.is_err());
}

// ============================================================================
// New tests
// ============================================================================

#[test]
fn postgres_store_config_serde_roundtrip() {
    let original = PostgresStoreConfig::default();
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: PostgresStoreConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original.connection, restored.connection);
    assert_eq!(original.max_connections, restored.max_connections);
    assert_eq!(original.connect_timeout_ms, restored.connect_timeout_ms);
    assert_eq!(original.statement_timeout_ms, restored.statement_timeout_ms);
}
