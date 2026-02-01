// decision-gate-store-sqlite/tests/schema_registry.rs
// ============================================================================
// Module: SQLite Schema Registry Tests
// Description: Tests for SQLite-backed schema persistence, tenant isolation,
//              concurrent access, and corruption recovery.
// Purpose: Ensure SQLite schema registry behaves correctly under various conditions.
// Dependencies: decision-gate-store-sqlite, decision-gate-core
// ============================================================================

//! ## Overview
//! Conformance tests for the SQLite-backed data shape registry.
//! Exercises tenant isolation, pagination, corruption handling, and concurrent
//! access patterns. Security posture: tests model untrusted registry storage per
//! `Docs/security/threat_model.md`.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    unused_imports,
    missing_docs,
    reason = "Test-only panic-based assertions are permitted."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::path::PathBuf;

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeSignature;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_store_sqlite::SqliteRunStateStore;
use decision_gate_store_sqlite::SqliteStoreConfig;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;
use decision_gate_store_sqlite::store::MAX_SCHEMA_BYTES;
use serde_json::json;
use tempfile::TempDir;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

fn sample_record(schema_id: &str, version: &str) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema: json!({"type": "object"}),
        description: Some("sample schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}

struct SqliteFixture {
    _dir: TempDir,
    path: PathBuf,
    store: SqliteRunStateStore,
}

fn sqlite_fixture() -> SqliteFixture {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("store.db");
    let config = SqliteStoreConfig {
        path: path.clone(),
        busy_timeout_ms: 5_000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions: None,
    };
    let store = SqliteRunStateStore::new(config).expect("store");
    SqliteFixture {
        _dir: dir,
        path,
        store,
    }
}

fn insert_oversized_record(fixture: &SqliteFixture, schema_id: &str) -> DataShapeRecord {
    let payload = "x".repeat(MAX_SCHEMA_BYTES);
    let schema = json!({ "payload": payload });
    let schema_bytes = canonical_json_bytes(&schema).expect("schema bytes");
    assert!(schema_bytes.len() > MAX_SCHEMA_BYTES, "schema payload must exceed size limit");
    let hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &schema_bytes);
    let created_at_json = serde_json::to_string(&Timestamp::Logical(1)).expect("created_at_json");
    let record = DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new("v1"),
        schema,
        description: None,
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let hash_algorithm = match hash.algorithm {
        decision_gate_core::hashing::HashAlgorithm::Sha256 => "sha256",
    };
    let connection = rusqlite::Connection::open(&fixture.path).expect("open registry db");
    connection
        .execute(
            "INSERT INTO data_shapes (
                tenant_id, namespace_id, schema_id, version,
                schema_json, schema_hash, hash_algorithm, description,
                signing_key_id, signing_signature, signing_algorithm,
                created_at_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                record.tenant_id.to_string(),
                record.namespace_id.to_string(),
                record.schema_id.as_str(),
                record.version.as_str(),
                schema_bytes,
                hash.value,
                hash_algorithm,
                record.description.as_deref(),
                None::<String>,
                None::<String>,
                None::<String>,
                created_at_json,
            ],
        )
        .expect("insert oversized schema");
    record
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn sqlite_registry_roundtrip() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();
    let fetched = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap()
        .expect("record");
    assert_eq!(fetched.schema_id, record.schema_id);
}

#[test]
fn sqlite_registry_preserves_signing_metadata() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let mut record = sample_record("schema-signing", "v1");
    record.signing = Some(DataShapeSignature {
        key_id: "key-1".to_string(),
        signature: "signature-1".to_string(),
        algorithm: Some("ed25519".to_string()),
    });
    store.register(record.clone()).unwrap();
    let fetched = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap()
        .expect("record present");
    assert_eq!(fetched.signing, record.signing);
}

#[test]
fn sqlite_registry_rejects_duplicate() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();
    let err = store.register(record).unwrap_err();
    assert!(err.to_string().contains("conflict"));
}

#[test]
fn sqlite_registry_lists_with_pagination() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record_alpha_v1 = sample_record("schema-a", "v1");
    let record_alpha_v2 = sample_record("schema-a", "v2");
    let record_bravo_v1 = sample_record("schema-b", "v1");
    store.register(record_bravo_v1).unwrap();
    store.register(record_alpha_v2).unwrap();
    store.register(record_alpha_v1.clone()).unwrap();

    let page =
        store.list(&record_alpha_v1.tenant_id, &record_alpha_v1.namespace_id, None, 2).unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page.next_token.is_some());
    assert_eq!(page.items[0].schema_id.as_str(), "schema-a");
    assert_eq!(page.items[0].version.as_str(), "v1");

    let next_page = store
        .list(&record_alpha_v1.tenant_id, &record_alpha_v1.namespace_id, page.next_token, 2)
        .unwrap();
    assert_eq!(next_page.items.len(), 1);
    assert_eq!(next_page.items[0].schema_id.as_str(), "schema-b");
}

#[test]
fn sqlite_registry_rejects_invalid_cursor() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();
    let err = store
        .list(&record.tenant_id, &record.namespace_id, Some("not-json".to_string()), 1)
        .unwrap_err();
    assert!(err.to_string().contains("invalid cursor"));
}

#[test]
fn sqlite_registry_rejects_zero_limit() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();
    let err = store.list(&record.tenant_id, &record.namespace_id, None, 0).unwrap_err();
    assert!(err.to_string().contains("limit"));
}

#[test]
fn sqlite_registry_respects_namespace_isolation() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record_default = sample_record("schema-a", "v1");
    let record_other = DataShapeRecord {
        namespace_id: NamespaceId::from_raw(2).expect("nonzero namespaceid"),
        ..sample_record("schema-a", "v2")
    };
    store.register(record_default.clone()).unwrap();
    store.register(record_other).unwrap();

    let page =
        store.list(&record_default.tenant_id, &record_default.namespace_id, None, 10).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].namespace_id, record_default.namespace_id);
}

#[test]
fn sqlite_registry_detects_schema_hash_mismatch() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();

    let connection = rusqlite::Connection::open(&fixture.path).unwrap();
    connection
        .execute(
            "UPDATE data_shapes SET schema_hash = 'bad' WHERE schema_id = ?1 AND version = ?2",
            rusqlite::params![record.schema_id.as_str(), record.version.as_str()],
        )
        .unwrap();

    let err = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap_err();
    assert!(err.to_string().contains("schema hash mismatch"));
}

#[test]
fn sqlite_registry_rejects_invalid_hash_algorithm() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();

    let connection = rusqlite::Connection::open(&fixture.path).unwrap();
    connection
        .execute(
            "UPDATE data_shapes SET hash_algorithm = 'invalid' WHERE schema_id = ?1 AND version = \
             ?2",
            rusqlite::params![record.schema_id.as_str(), record.version.as_str()],
        )
        .unwrap();

    let err = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap_err();
    assert!(err.to_string().contains("unsupported hash algorithm"));
}

// ============================================================================
// SECTION: Tenant Isolation Tests
// ============================================================================

#[test]
fn sqlite_registry_same_schema_different_tenants_both_persist() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let tenant1_record = DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("schema-a"),
        version: DataShapeVersion::new("v1"),
        schema: json!({"type": "object", "tenant": 1}),
        description: Some("tenant 1".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let tenant2_record = DataShapeRecord {
        tenant_id: TenantId::from_raw(2).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("schema-a"),
        version: DataShapeVersion::new("v1"),
        schema: json!({"type": "object", "tenant": 2}),
        description: Some("tenant 2".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    };

    store.register(tenant1_record.clone()).unwrap();
    store.register(tenant2_record.clone()).unwrap();

    // Both tenants can retrieve their own schema
    let fetched1 = store
        .get(
            &tenant1_record.tenant_id,
            &tenant1_record.namespace_id,
            &tenant1_record.schema_id,
            &tenant1_record.version,
        )
        .unwrap()
        .expect("tenant 1 record");
    assert_eq!(fetched1.tenant_id.get(), 1);

    let fetched2 = store
        .get(
            &tenant2_record.tenant_id,
            &tenant2_record.namespace_id,
            &tenant2_record.schema_id,
            &tenant2_record.version,
        )
        .unwrap()
        .expect("tenant-2 record");
    assert_eq!(fetched2.tenant_id.get(), 2);
}

#[test]
fn sqlite_registry_list_filters_by_tenant() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let tenant1_record = DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("schema-a"),
        version: DataShapeVersion::new("v1"),
        schema: json!({"type": "object"}),
        description: None,
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    let tenant2_record = DataShapeRecord {
        tenant_id: TenantId::from_raw(2).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("schema-b"),
        version: DataShapeVersion::new("v1"),
        schema: json!({"type": "object"}),
        description: None,
        created_at: Timestamp::Logical(1),
        signing: None,
    };

    store.register(tenant1_record).unwrap();
    store.register(tenant2_record).unwrap();

    // List for tenant 1 should only return tenant 1's schema
    let page1 = store
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].tenant_id.get(), 1);

    // List for tenant-2 should only return tenant-2's schema
    let page2 = store
        .list(
            &TenantId::from_raw(2).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].tenant_id.get(), 2);
}

#[test]
fn sqlite_registry_get_requires_exact_tenant_match() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("schema-a"),
        version: DataShapeVersion::new("v1"),
        schema: json!({"type": "object"}),
        description: None,
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    store.register(record).unwrap();

    // Wrong tenant returns None
    let wrong_tenant = store
        .get(
            &TenantId::from_raw(2).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &DataShapeId::new("schema-a"),
            &DataShapeVersion::new("v1"),
        )
        .unwrap();
    assert!(wrong_tenant.is_none());
}

// ============================================================================
// SECTION: Version Ordering Edge Cases
// ============================================================================

#[test]
fn sqlite_registry_versions_sorted_lexicographically() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    // Register in random order
    store
        .register(DataShapeRecord {
            tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
            namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            schema_id: DataShapeId::new("schema-a"),
            version: DataShapeVersion::new("v2"),
            schema: json!({"type": "object"}),
            description: None,
            created_at: Timestamp::Logical(1),
            signing: None,
        })
        .unwrap();
    store
        .register(DataShapeRecord {
            tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
            namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            schema_id: DataShapeId::new("schema-a"),
            version: DataShapeVersion::new("v10"),
            schema: json!({"type": "object"}),
            description: None,
            created_at: Timestamp::Logical(2),
            signing: None,
        })
        .unwrap();
    store
        .register(DataShapeRecord {
            tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
            namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            schema_id: DataShapeId::new("schema-a"),
            version: DataShapeVersion::new("v1"),
            schema: json!({"type": "object"}),
            description: None,
            created_at: Timestamp::Logical(3),
            signing: None,
        })
        .unwrap();

    let page = store
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();

    let versions: Vec<&str> = page.items.iter().map(|r| r.version.as_str()).collect();
    // Lexicographic: "v1" < "v10" < "v2"
    assert_eq!(versions, vec!["v1", "v10", "v2"]);
}

// ============================================================================
// SECTION: Concurrent Access Tests
// ============================================================================

#[test]
fn sqlite_registry_concurrent_writes_different_schemas_no_deadlock() {
    use std::sync::Arc;
    use std::thread;

    let fixture = sqlite_fixture();
    let path = fixture.path.clone();
    let mut handles = vec![];

    for i in 0 .. 5u64 {
        let path = path.clone();
        let handle = thread::spawn(move || {
            let config = SqliteStoreConfig {
                path,
                busy_timeout_ms: 5_000,
                journal_mode: SqliteStoreMode::Wal,
                sync_mode: SqliteSyncMode::Full,
                max_versions: None,
            };
            let store = SqliteRunStateStore::new(config).expect("store");
            for j in 0 .. 3u64 {
                let record = DataShapeRecord {
                    tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
                    namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
                    schema_id: DataShapeId::new(format!("schema-{i}-{j}")),
                    version: DataShapeVersion::new("v1"),
                    schema: json!({"type": "object", "thread": i, "index": j}),
                    description: None,
                    created_at: Timestamp::Logical(i * 10 + j),
                    signing: None,
                };
                store.register(record).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all 15 schemas are registered
    let page = fixture
        .store
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            20,
        )
        .unwrap();
    assert_eq!(page.items.len(), 15);
}

#[test]
fn sqlite_registry_concurrent_read_write_consistent() {
    use std::sync::Arc;
    use std::thread;

    let fixture = sqlite_fixture();
    let path = fixture.path.clone();

    // Pre-register some schemas
    for i in 0 .. 5u64 {
        let record = DataShapeRecord {
            tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
            namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            schema_id: DataShapeId::new(format!("initial-{i}")),
            version: DataShapeVersion::new("v1"),
            schema: json!({"type": "object"}),
            description: None,
            created_at: Timestamp::Logical(i),
            signing: None,
        };
        fixture.store.register(record).unwrap();
    }

    let mut handles = vec![];

    // Writers
    for i in 0 .. 3u64 {
        let path = path.clone();
        let handle = thread::spawn(move || {
            let config = SqliteStoreConfig {
                path,
                busy_timeout_ms: 5_000,
                journal_mode: SqliteStoreMode::Wal,
                sync_mode: SqliteSyncMode::Full,
                max_versions: None,
            };
            let store = SqliteRunStateStore::new(config).expect("store");
            for j in 0 .. 5u64 {
                let record = DataShapeRecord {
                    tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
                    namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
                    schema_id: DataShapeId::new(format!("concurrent-{i}-{j}")),
                    version: DataShapeVersion::new("v1"),
                    schema: json!({"type": "object"}),
                    description: None,
                    created_at: Timestamp::Logical(100 + i * 10 + j),
                    signing: None,
                };
                let _ = store.register(record); // Ignore conflicts
            }
        });
        handles.push(handle);
    }

    // Readers
    for _ in 0 .. 3 {
        let path = path.clone();
        let handle = thread::spawn(move || {
            let config = SqliteStoreConfig {
                path,
                busy_timeout_ms: 5_000,
                journal_mode: SqliteStoreMode::Wal,
                sync_mode: SqliteSyncMode::Full,
                max_versions: None,
            };
            let store = SqliteRunStateStore::new(config).expect("store");
            for _ in 0 .. 10 {
                let result = store.list(
                    &TenantId::from_raw(1).expect("nonzero tenantid"),
                    &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
                    None,
                    100,
                );
                // Should never fail
                assert!(result.is_ok());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

// ============================================================================
// SECTION: Corruption Recovery Tests
// ============================================================================

#[test]
fn sqlite_registry_corrupted_schema_json_detected() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();

    // Corrupt the schema JSON
    let connection = rusqlite::Connection::open(&fixture.path).unwrap();
    connection
        .execute(
            "UPDATE data_shapes SET schema_json = X'DEADBEEF' WHERE schema_id = ?1 AND version = \
             ?2",
            rusqlite::params![record.schema_id.as_str(), record.version.as_str()],
        )
        .unwrap();

    // Try to retrieve - should fail with invalid data error
    let err = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap_err();
    // The error could be about JSON parsing or hash mismatch
    let err_str = err.to_string();
    assert!(
        err_str.contains("schema hash mismatch")
            || err_str.contains("invalid")
            || err_str.contains("error")
    );
}

#[test]
fn sqlite_registry_corrupted_timestamp_detected() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = sample_record("schema-a", "v1");
    store.register(record.clone()).unwrap();

    // Corrupt the timestamp JSON
    let connection = rusqlite::Connection::open(&fixture.path).unwrap();
    connection
        .execute(
            "UPDATE data_shapes SET created_at_json = 'not-valid-json' WHERE schema_id = ?1 AND \
             version = ?2",
            rusqlite::params![record.schema_id.as_str(), record.version.as_str()],
        )
        .unwrap();

    // Try to retrieve - should fail with parse error
    let err = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap_err();
    // Should fail with some kind of parse error
    assert!(!err.to_string().is_empty()); // Just verify it's an error
}

// ============================================================================
// SECTION: Size Limit Tests
// ============================================================================

#[test]
fn sqlite_registry_empty_schema_persists() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let record = DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("empty"),
        version: DataShapeVersion::new("v1"),
        schema: json!({}),
        description: None,
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    store.register(record.clone()).unwrap();
    let fetched = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap()
        .expect("record");
    assert_eq!(fetched.schema, json!({}));
}

#[test]
fn sqlite_registry_deeply_nested_schema_persists() {
    let fixture = sqlite_fixture();
    let store = &fixture.store;
    let nested = json!({
        "type": "object",
        "properties": {
            "level1": {
                "type": "object",
                "properties": {
                    "level2": {
                        "type": "object",
                        "properties": {
                            "level3": {
                                "type": "object",
                                "properties": {
                                    "level4": {"type": "string"}
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let record = DataShapeRecord {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new("nested"),
        version: DataShapeVersion::new("v1"),
        schema: nested.clone(),
        description: None,
        created_at: Timestamp::Logical(1),
        signing: None,
    };
    store.register(record.clone()).unwrap();
    let fetched = store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap()
        .expect("record");
    assert_eq!(fetched.schema, nested);
}

#[test]
fn sqlite_registry_rejects_oversized_schema_on_get() {
    let fixture = sqlite_fixture();
    let record = insert_oversized_record(&fixture, "schema-oversized-get");
    let err = fixture
        .store
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap_err();
    assert!(err.to_string().contains("schema exceeds size limit"));
}

#[test]
fn sqlite_registry_rejects_oversized_schema_on_list() {
    let fixture = sqlite_fixture();
    let record = insert_oversized_record(&fixture, "schema-oversized-list");
    let err = fixture.store.list(&record.tenant_id, &record.namespace_id, None, 10).unwrap_err();
    assert!(err.to_string().contains("schema exceeds size limit"));
}
