// enterprise/decision-gate-enterprise/tests/usage_sqlite.rs
// ============================================================================
// Module: SQLite Usage Ledger Tests
// Description: Unit tests for SQLite usage ledger storage.
// Purpose: Validate append, sum, and idempotency behavior.
// ============================================================================

//! `SQLite` usage ledger unit tests.

#![allow(clippy::expect_used, reason = "Tests use expect for setup clarity.")]

use std::sync::Arc;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_enterprise::usage::UsageEvent;
use decision_gate_enterprise::usage::UsageLedger;
use decision_gate_enterprise::usage_sqlite::SqliteUsageLedger;
use decision_gate_mcp::UsageMetric;
use tempfile::NamedTempFile;

#[test]
fn sqlite_usage_ledger_sums_by_scope() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");
    let tenant = TenantId::new("tenant-1");
    let ns_a = NamespaceId::new("a");
    let ns_b = NamespaceId::new("b");
    ledger
        .append(UsageEvent {
            tenant_id: tenant.clone(),
            namespace_id: ns_a,
            metric: UsageMetric::ToolCall,
            units: 3,
            timestamp_ms: 1000,
            idempotency_key: None,
        })
        .expect("append");
    ledger
        .append(UsageEvent {
            tenant_id: tenant,
            namespace_id: ns_b,
            metric: UsageMetric::ToolCall,
            units: 2,
            timestamp_ms: 1000,
            idempotency_key: None,
        })
        .expect("append");
    let tenant_sum = ledger.sum_since("tenant-1/*", UsageMetric::ToolCall, 0).expect("sum");
    assert_eq!(tenant_sum, 5);
    let ns_sum = ledger.sum_since("tenant-1/a", UsageMetric::ToolCall, 0).expect("sum");
    assert_eq!(ns_sum, 3);
}

#[test]
fn sqlite_usage_ledger_idempotency_is_enforced() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");
    let event = UsageEvent {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        metric: UsageMetric::ToolCall,
        units: 1,
        timestamp_ms: 1000,
        idempotency_key: Some("id-1".to_string()),
    };
    ledger.append(event.clone()).expect("append");
    ledger.append(event).expect("append");
    let sum = ledger.sum_since("tenant-1/default", UsageMetric::ToolCall, 0).expect("sum");
    assert_eq!(sum, 1);
    assert!(ledger.seen_idempotency("id-1").expect("seen"));
}

// ---------------------------------------------------------------------------
// New tests
// ---------------------------------------------------------------------------

#[test]
fn sqlite_usage_ledger_time_window_filtering() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");

    ledger
        .append(UsageEvent {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            metric: UsageMetric::ToolCall,
            units: 5,
            timestamp_ms: 1000,
            idempotency_key: None,
        })
        .expect("append");

    // since_ms=2000 should exclude the event at timestamp_ms=1000.
    let sum_after = ledger.sum_since("tenant-1/default", UsageMetric::ToolCall, 2000).expect("sum");
    assert_eq!(sum_after, 0);

    // since_ms=500 should include the event at timestamp_ms=1000.
    let sum_before = ledger.sum_since("tenant-1/default", UsageMetric::ToolCall, 500).expect("sum");
    assert_eq!(sum_before, 5);
}

#[test]
fn sqlite_usage_seen_idempotency_false_for_unseen() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");
    let seen = ledger.seen_idempotency("nonexistent").expect("seen");
    assert!(!seen);
}

#[test]
fn sqlite_usage_null_idempotency_not_in_seen() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");

    // Append event with no idempotency key.
    ledger
        .append(UsageEvent {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            metric: UsageMetric::ToolCall,
            units: 1,
            timestamp_ms: 1000,
            idempotency_key: None,
        })
        .expect("append");

    // No idempotency key was stored, so anything returns false.
    assert!(!ledger.seen_idempotency("anything").expect("seen"));
}

#[test]
fn sqlite_usage_sum_since_filters_by_metric() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");

    // Append ToolCall event with 3 units.
    ledger
        .append(UsageEvent {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            metric: UsageMetric::ToolCall,
            units: 3,
            timestamp_ms: 1000,
            idempotency_key: Some("tc-1".to_string()),
        })
        .expect("append");

    // Append RunsStarted event with 7 units.
    ledger
        .append(UsageEvent {
            tenant_id: TenantId::new("tenant-1"),
            namespace_id: NamespaceId::new("default"),
            metric: UsageMetric::RunsStarted,
            units: 7,
            timestamp_ms: 1000,
            idempotency_key: Some("rs-1".to_string()),
        })
        .expect("append");

    let tc_sum = ledger.sum_since("tenant-1/default", UsageMetric::ToolCall, 0).expect("sum");
    assert_eq!(tc_sum, 3);

    let rs_sum = ledger.sum_since("tenant-1/default", UsageMetric::RunsStarted, 0).expect("sum");
    assert_eq!(rs_sum, 7);
}

#[test]
fn sqlite_usage_concurrent_append() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = Arc::new(SqliteUsageLedger::new(file.path()).expect("ledger"));
    let num_threads: usize = 4;
    let events_per_thread: usize = 100;
    let mut handles = Vec::new();

    for thread_idx in 0 .. num_threads {
        let ledger = Arc::clone(&ledger);
        let handle = std::thread::spawn(move || {
            let tenant = TenantId::new("tenant-1");
            let ns = NamespaceId::new("default");
            for event_idx in 0 .. events_per_thread {
                ledger
                    .append(UsageEvent {
                        tenant_id: tenant.clone(),
                        namespace_id: ns.clone(),
                        metric: UsageMetric::ToolCall,
                        units: 1,
                        timestamp_ms: 1000,
                        idempotency_key: Some(format!("t{thread_idx}-e{event_idx}")),
                    })
                    .expect("append");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread join");
    }

    let total = ledger.sum_since("tenant-1/*", UsageMetric::ToolCall, 0).expect("sum");
    assert_eq!(total, (num_threads * events_per_thread) as u64);
}

#[test]
fn sqlite_usage_empty_ledger_sum_is_zero() {
    let file = NamedTempFile::new().expect("temp file");
    let ledger = SqliteUsageLedger::new(file.path()).expect("ledger");
    let sum = ledger.sum_since("t1/*", UsageMetric::ToolCall, 0).expect("sum");
    assert_eq!(sum, 0);
}

#[test]
fn sqlite_usage_event_helper_constructs() {
    let tenant = TenantId::new("t1");
    let ns = NamespaceId::new("ns1");
    let event = decision_gate_enterprise::usage_sqlite::usage_event(
        tenant,
        ns,
        UsageMetric::RunsStarted,
        42,
        9999,
        Some("key-1".to_string()),
    );
    assert_eq!(event.tenant_id.as_str(), "t1");
    assert_eq!(event.namespace_id.as_str(), "ns1");
    assert_eq!(event.metric, UsageMetric::RunsStarted);
    assert_eq!(event.units, 42);
    assert_eq!(event.timestamp_ms, 9999);
    assert_eq!(event.idempotency_key.as_deref(), Some("key-1"));
}
