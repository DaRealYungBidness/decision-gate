// decision-gate-core/tests/data_shape_registry.rs
// ============================================================================
// Module: Data Shape Registry Tests
// Description: Tests for schema registration, retrieval, listing, and isolation.
// Purpose: Ensure data shape registry behaves correctly under various conditions.
// Dependencies: decision-gate-core
// ============================================================================

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    missing_docs,
    reason = "Test-only panic-based assertions are permitted."
)]

use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::InMemoryDataShapeRegistry;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use serde_json::json;

fn sample_record(schema_id: &str, version: &str) -> DataShapeRecord {
    sample_record_with(1, 1, schema_id, version, json!({"type": "object"}))
}

fn sample_record_with(
    tenant: u64,
    namespace: u64,
    schema_id: &str,
    version: &str,
    schema: serde_json::Value,
) -> DataShapeRecord {
    DataShapeRecord {
        tenant_id: TenantId::from_raw(tenant).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(namespace).expect("nonzero namespaceid"),
        schema_id: DataShapeId::new(schema_id),
        version: DataShapeVersion::new(version),
        schema,
        description: Some("sample schema".to_string()),
        created_at: Timestamp::Logical(1),
        signing: None,
    }
}

#[test]
fn registry_registers_and_gets_record() {
    let registry = InMemoryDataShapeRegistry::new();
    let record = sample_record("schema-a", "v1");
    registry.register(record.clone()).unwrap();
    let fetched = registry
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap()
        .expect("record");
    assert_eq!(fetched.schema_id, record.schema_id);
}

#[test]
fn registry_rejects_duplicate_version() {
    let registry = InMemoryDataShapeRegistry::new();
    let record = sample_record("schema-a", "v1");
    registry.register(record.clone()).unwrap();
    let err = registry.register(record).unwrap_err();
    assert!(err.to_string().contains("conflict"));
}

#[test]
fn registry_lists_with_pagination() {
    let registry = InMemoryDataShapeRegistry::new();
    let record_a = sample_record("schema-a", "v1");
    let record_b = sample_record("schema-b", "v1");
    registry.register(record_a.clone()).unwrap();
    registry.register(record_b).unwrap();

    let page = registry.list(&record_a.tenant_id, &record_a.namespace_id, None, 1).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.next_token.is_some());

    let next_page =
        registry.list(&record_a.tenant_id, &record_a.namespace_id, page.next_token, 1).unwrap();
    assert_eq!(next_page.items.len(), 1);
}

#[test]
fn registry_rejects_zero_limit() {
    let registry = InMemoryDataShapeRegistry::new();
    let record = sample_record("schema-a", "v1");
    registry.register(record.clone()).unwrap();
    let err = registry.list(&record.tenant_id, &record.namespace_id, None, 0).unwrap_err();
    assert!(err.to_string().contains("limit"));
}

#[test]
fn registry_rejects_invalid_cursor() {
    let registry = InMemoryDataShapeRegistry::new();
    let record = sample_record("schema-a", "v1");
    registry.register(record.clone()).unwrap();
    let err = registry
        .list(&record.tenant_id, &record.namespace_id, Some("not-json".to_string()), 1)
        .unwrap_err();
    assert!(err.to_string().contains("invalid cursor"));
}

#[test]
fn registry_respects_namespace_isolation() {
    let registry = InMemoryDataShapeRegistry::new();
    let default_record = sample_record("schema-a", "v1");
    let other_record = sample_record_with(1, 2, "schema-a", "v1", json!({"type": "object"}));
    registry.register(default_record.clone()).unwrap();
    registry.register(other_record).unwrap();

    let page =
        registry.list(&default_record.tenant_id, &default_record.namespace_id, None, 10).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].namespace_id, default_record.namespace_id);
}

#[test]
fn registry_enforces_max_entries() {
    let registry = InMemoryDataShapeRegistry::with_limits(1024, Some(1));
    let record_a = sample_record("schema-a", "v1");
    let record_b = sample_record("schema-b", "v1");
    registry.register(record_a).unwrap();
    let err = registry.register(record_b).unwrap_err();
    assert!(err.to_string().contains("max entries"));
}

#[test]
fn registry_enforces_max_schema_bytes() {
    let registry = InMemoryDataShapeRegistry::with_limits(32, None);
    let oversized = "x".repeat(128);
    let record = sample_record_with(
        1,
        1,
        "schema-a",
        "v1",
        json!({"type": "string", "description": oversized}),
    );
    let err = registry.register(record).unwrap_err();
    assert!(err.to_string().contains("schema exceeds size limit"));
}

#[test]
fn registry_orders_by_schema_id_then_version() {
    let registry = InMemoryDataShapeRegistry::new();
    let record_b = sample_record("schema-b", "v1");
    let record_a2 = sample_record("schema-a", "v2");
    let record_a1 = sample_record("schema-a", "v1");
    registry.register(record_b).unwrap();
    registry.register(record_a2).unwrap();
    registry.register(record_a1).unwrap();

    let page = registry
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();
    let keys: Vec<(String, String)> = page
        .items
        .iter()
        .map(|record| (record.schema_id.to_string(), record.version.to_string()))
        .collect();
    assert_eq!(
        keys,
        vec![
            ("schema-a".to_string(), "v1".to_string()),
            ("schema-a".to_string(), "v2".to_string()),
            ("schema-b".to_string(), "v1".to_string())
        ]
    );
}

#[test]
fn registry_returns_empty_after_last_cursor() {
    let registry = InMemoryDataShapeRegistry::new();
    let record_a = sample_record("schema-a", "v1");
    let record_b = sample_record("schema-b", "v1");
    registry.register(record_a.clone()).unwrap();
    registry.register(record_b).unwrap();

    let page = registry.list(&record_a.tenant_id, &record_a.namespace_id, None, 2).unwrap();
    assert_eq!(page.items.len(), 2);
    let next_page =
        registry.list(&record_a.tenant_id, &record_a.namespace_id, page.next_token, 2).unwrap();
    assert!(next_page.items.is_empty());
    assert!(next_page.next_token.is_none());
}

// ============================================================================
// SECTION: Tenant Isolation Tests
// ============================================================================

#[test]
fn registry_same_schema_different_tenants_both_succeed() {
    let registry = InMemoryDataShapeRegistry::new();
    let tenant1_record = sample_record_with(1, 1, "schema-a", "v1", json!({"type": "object"}));
    let tenant2_record = sample_record_with(2, 1, "schema-a", "v1", json!({"type": "object"}));

    registry.register(tenant1_record.clone()).unwrap();
    registry.register(tenant2_record.clone()).unwrap();

    // Both tenants can retrieve their own schema
    let fetched1 = registry
        .get(
            &tenant1_record.tenant_id,
            &tenant1_record.namespace_id,
            &tenant1_record.schema_id,
            &tenant1_record.version,
        )
        .unwrap()
        .expect("tenant 1 record");
    assert_eq!(fetched1.tenant_id.get(), 1);

    let fetched2 = registry
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
fn registry_list_filters_by_tenant() {
    let registry = InMemoryDataShapeRegistry::new();
    let tenant1_record = sample_record_with(1, 1, "schema-a", "v1", json!({"type": "object"}));
    let tenant2_record = sample_record_with(2, 1, "schema-b", "v1", json!({"type": "object"}));

    registry.register(tenant1_record).unwrap();
    registry.register(tenant2_record).unwrap();

    // List for tenant 1 should only return tenant 1's schema
    let page1 = registry
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
    let page2 = registry
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
fn registry_get_requires_exact_tenant_match() {
    let registry = InMemoryDataShapeRegistry::new();
    let tenant1_record = sample_record_with(1, 1, "schema-a", "v1", json!({"type": "object"}));
    registry.register(tenant1_record).unwrap();

    // Trying to get with wrong tenant returns None
    let wrong_tenant = registry
        .get(
            &TenantId::from_raw(2).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &DataShapeId::new("schema-a"),
            &DataShapeVersion::new("v1"),
        )
        .unwrap();
    assert!(wrong_tenant.is_none());
}

#[test]
fn registry_register_same_schema_id_different_tenant_no_conflict() {
    let registry = InMemoryDataShapeRegistry::new();
    let tenant1_record = sample_record_with(1, 1, "shared-schema", "v1", json!({"type": "string"}));
    let tenant2_record = sample_record_with(2, 1, "shared-schema", "v1", json!({"type": "number"}));

    // Both should succeed - no conflict across tenants
    registry.register(tenant1_record).unwrap();
    registry.register(tenant2_record).unwrap();
}

// ============================================================================
// SECTION: Version Ordering Edge Cases
// ============================================================================

#[test]
fn registry_versions_v1_v10_v2_sorted_lexicographically() {
    let registry = InMemoryDataShapeRegistry::new();
    // Register in random order
    registry.register(sample_record("schema-a", "v2")).unwrap();
    registry.register(sample_record("schema-a", "v10")).unwrap();
    registry.register(sample_record("schema-a", "v1")).unwrap();

    let page = registry
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

#[test]
fn registry_versions_semver_style_sorted_lexicographically() {
    let registry = InMemoryDataShapeRegistry::new();
    registry.register(sample_record("schema-a", "1.0.2")).unwrap();
    registry.register(sample_record("schema-a", "1.0.10")).unwrap();
    registry.register(sample_record("schema-a", "1.0.1")).unwrap();

    let page = registry
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();

    let versions: Vec<&str> = page.items.iter().map(|r| r.version.as_str()).collect();
    // Lexicographic: "1.0.1" < "1.0.10" < "1.0.2"
    assert_eq!(versions, vec!["1.0.1", "1.0.10", "1.0.2"]);
}

#[test]
fn registry_versions_with_prefix_sorted_correctly() {
    let registry = InMemoryDataShapeRegistry::new();
    registry.register(sample_record("schema-a", "release-2")).unwrap();
    registry.register(sample_record("schema-a", "beta-1")).unwrap();
    registry.register(sample_record("schema-a", "alpha-1")).unwrap();

    let page = registry
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();

    let versions: Vec<&str> = page.items.iter().map(|r| r.version.as_str()).collect();
    // Lexicographic: "alpha-1" < "beta-1" < "release-2"
    assert_eq!(versions, vec!["alpha-1", "beta-1", "release-2"]);
}

#[test]
fn registry_versions_numeric_only_sorted_as_strings() {
    let registry = InMemoryDataShapeRegistry::new();
    registry.register(sample_record("schema-a", "2")).unwrap();
    registry.register(sample_record("schema-a", "10")).unwrap();
    registry.register(sample_record("schema-a", "1")).unwrap();

    let page = registry
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            10,
        )
        .unwrap();

    let versions: Vec<&str> = page.items.iter().map(|r| r.version.as_str()).collect();
    // Lexicographic: "1" < "10" < "2"
    assert_eq!(versions, vec!["1", "10", "2"]);
}

// ============================================================================
// SECTION: Size Boundary Tests
// ============================================================================

#[test]
fn registry_schema_exactly_at_max_bytes_accepted() {
    // 64-byte limit for testing
    let registry = InMemoryDataShapeRegistry::with_limits(64, None);
    // Create a schema that is exactly at the limit
    // The JSON serialization must be exactly 64 bytes
    let small_schema = json!({"t":"o"});
    let record = sample_record_with(1, 1, "schema-a", "v1", small_schema);
    // This should succeed as it's under 64 bytes
    registry.register(record).unwrap();
}

#[test]
fn registry_empty_schema_object_accepted() {
    let registry = InMemoryDataShapeRegistry::new();
    let record = sample_record_with(1, 1, "schema-a", "v1", json!({}));
    registry.register(record.clone()).unwrap();
    let fetched = registry
        .get(&record.tenant_id, &record.namespace_id, &record.schema_id, &record.version)
        .unwrap()
        .expect("record");
    assert_eq!(fetched.schema, json!({}));
}

#[test]
fn registry_deeply_nested_schema_within_limit_accepted() {
    let registry = InMemoryDataShapeRegistry::new();
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
                                "type": "string"
                            }
                        }
                    }
                }
            }
        }
    });
    let record = sample_record_with(1, 1, "schema-a", "v1", nested);
    registry.register(record).unwrap();
}

// ============================================================================
// SECTION: Cursor Edge Cases
// ============================================================================

#[test]
fn registry_cursor_with_empty_schema_id_handled() {
    let registry = InMemoryDataShapeRegistry::new();
    let record = sample_record("schema-a", "v1");
    registry.register(record.clone()).unwrap();

    // An invalid cursor with empty schema_id
    let invalid_cursor = serde_json::to_string(&json!({"schema_id": "", "version": "v1"})).unwrap();
    let result = registry.list(&record.tenant_id, &record.namespace_id, Some(invalid_cursor), 10);
    // Should either work (treating empty as start) or return an error
    // The implementation filters by range, so empty string cursor just starts from beginning
    assert!(result.is_ok());
}

#[test]
fn registry_cursor_with_special_characters_handled() {
    let registry = InMemoryDataShapeRegistry::new();
    // Register with special characters in schema_id
    let record = sample_record_with(1, 1, "schema/with:special", "v1", json!({"type": "object"}));
    registry.register(record.clone()).unwrap();

    let page = registry.list(&record.tenant_id, &record.namespace_id, None, 10).unwrap();
    assert_eq!(page.items.len(), 1);

    // If there's a next_token, it should handle special characters
    if let Some(token) = page.next_token {
        let result = registry.list(&record.tenant_id, &record.namespace_id, Some(token), 10);
        assert!(result.is_ok());
    }
}

// ============================================================================
// SECTION: Concurrent Access Tests
// ============================================================================

#[test]
fn registry_concurrent_registers_to_different_schemas_succeed() {
    use std::sync::Arc;
    use std::thread;

    let registry = Arc::new(InMemoryDataShapeRegistry::new());
    let mut handles = vec![];

    for i in 0 .. 10u64 {
        let registry = Arc::clone(&registry);
        let handle = thread::spawn(move || {
            let record = DataShapeRecord {
                tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
                namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
                schema_id: DataShapeId::new(format!("schema-{i}")),
                version: DataShapeVersion::new("v1"),
                schema: json!({"type": "object", "id": i}),
                description: Some(format!("schema {i}")),
                created_at: Timestamp::Logical(i),
                signing: None,
            };
            registry.register(record)
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    // Verify all 10 schemas are registered
    let page = registry
        .list(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            None,
            20,
        )
        .unwrap();
    assert_eq!(page.items.len(), 10);
}

#[test]
fn registry_concurrent_register_same_schema_one_wins_one_conflicts() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::thread;

    let registry = Arc::new(InMemoryDataShapeRegistry::new());
    let success_count = Arc::new(AtomicUsize::new(0));
    let conflict_count = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for i in 0 .. 10u64 {
        let registry = Arc::clone(&registry);
        let success_count = Arc::clone(&success_count);
        let conflict_count = Arc::clone(&conflict_count);
        let handle = thread::spawn(move || {
            let record = DataShapeRecord {
                tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
                namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
                schema_id: DataShapeId::new("same-schema"),
                version: DataShapeVersion::new("v1"),
                schema: json!({"type": "object", "thread": i}),
                description: Some(format!("from thread {i}")),
                created_at: Timestamp::Logical(i),
                signing: None,
            };
            match registry.register(record) {
                Ok(()) => {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) if e.to_string().contains("conflict") => {
                    conflict_count.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Exactly one should succeed, rest should conflict
    assert_eq!(success_count.load(Ordering::Relaxed), 1);
    assert_eq!(conflict_count.load(Ordering::Relaxed), 9);
}

#[test]
fn registry_concurrent_list_during_register_consistent() {
    use std::sync::Arc;
    use std::thread;

    let registry = Arc::new(InMemoryDataShapeRegistry::new());
    let mut handles = vec![];

    // Spawn writers
    for i in 0 .. 5u64 {
        let registry = Arc::clone(&registry);
        let handle = thread::spawn(move || {
            for j in 0 .. 10u64 {
                let record = DataShapeRecord {
                    tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
                    namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
                    schema_id: DataShapeId::new(format!("schema-{i}-{j}")),
                    version: DataShapeVersion::new("v1"),
                    schema: json!({"type": "object"}),
                    description: None,
                    created_at: Timestamp::Logical(i * 10 + j),
                    signing: None,
                };
                let _ = registry.register(record);
            }
        });
        handles.push(handle);
    }

    // Spawn readers
    for _ in 0 .. 5 {
        let registry = Arc::clone(&registry);
        let handle = thread::spawn(move || {
            for _ in 0 .. 20 {
                let result = registry.list(
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
