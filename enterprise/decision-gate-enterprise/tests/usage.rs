// enterprise/decision-gate-enterprise/tests/usage.rs
// ============================================================================
// Module: Usage Meter Tests
// Description: Unit tests for quota enforcement and idempotency behavior.
// Purpose: Validate usage metering correctness without external services.
// ============================================================================

//! Usage meter unit tests.

#![allow(clippy::expect_used, reason = "Tests use expect for setup clarity.")]

use std::collections::BTreeMap;

use decision_gate_contract::ToolName;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_enterprise::usage::InMemoryUsageLedger;
use decision_gate_enterprise::usage::QuotaLimit;
use decision_gate_enterprise::usage::QuotaPolicy;
use decision_gate_enterprise::usage::QuotaScope;
use decision_gate_enterprise::usage::UsageQuotaEnforcer;
use decision_gate_mcp::AuthContext;
use decision_gate_mcp::UsageCheckRequest;
use decision_gate_mcp::UsageMeter;
use decision_gate_mcp::UsageMetric;
use decision_gate_mcp::UsageRecord;
use decision_gate_mcp::auth::AuthMethod;

const fn auth_context() -> AuthContext {
    AuthContext {
        method: AuthMethod::Local,
        subject: None,
        token_fingerprint: None,
    }
}

#[test]
fn usage_enforcer_blocks_missing_tenant_or_namespace() {
    let policy = QuotaPolicy::default();
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let check = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: None,
            namespace_id: None,
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(!check.allowed);
}

#[test]
fn usage_enforcer_enforces_quota() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 1,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    let check = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: Some("req-1"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(check.allowed);
    meter.record(
        &auth,
        UsageRecord {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: Some("req-1"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    let check_after = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: Some("req-2"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(!check_after.allowed);
}

#[test]
fn usage_enforcer_idempotency_deduplicates() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 2,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    for _ in 0 .. 2 {
        meter.record(
            &auth,
            UsageRecord {
                tool: &ToolName::RunpackExport,
                tenant_id: Some(&tenant_id),
                namespace_id: Some(&namespace_id),
                request_id: Some("req-1"),
                metric: UsageMetric::ToolCall,
                units: 1,
            },
        );
    }
    let check = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: Some("req-2"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(check.allowed);
}

// ---------------------------------------------------------------------------
// New tests
// ---------------------------------------------------------------------------

struct FailingLedger;

impl decision_gate_enterprise::usage::UsageLedger for FailingLedger {
    fn append(
        &self,
        _event: decision_gate_enterprise::usage::UsageEvent,
    ) -> Result<(), decision_gate_enterprise::usage::UsageLedgerError> {
        Ok(())
    }

    fn sum_since(
        &self,
        _scope_key: &str,
        _metric: UsageMetric,
        _since_ms: u128,
    ) -> Result<u64, decision_gate_enterprise::usage::UsageLedgerError> {
        Err(decision_gate_enterprise::usage::UsageLedgerError::Storage("injected".to_string()))
    }

    fn seen_idempotency(
        &self,
        _key: &str,
    ) -> Result<bool, decision_gate_enterprise::usage::UsageLedgerError> {
        Ok(false)
    }
}

#[test]
fn usage_check_fails_closed_on_ledger_error() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 100,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(FailingLedger, policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");
    let check = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(!check.allowed, "fail-closed: ledger error must deny");
}

#[test]
fn usage_check_with_tenant_but_no_namespace_denies() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 100,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let check = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: None,
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(!check.allowed);
    assert_eq!(check.reason, "missing_namespace_id");
}

#[test]
fn usage_namespace_scope_enforces_per_namespace() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 1,
            window_ms: 60_000,
            scope: QuotaScope::Namespace,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let ns_a = NamespaceId::new("a");
    let ns_b = NamespaceId::new("b");

    // Record 1 unit for namespace "a".
    meter.record(
        &auth,
        UsageRecord {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&ns_a),
            request_id: Some("req-a-1"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );

    // Check for namespace "b" -- should be allowed (different namespace).
    let check_b = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&ns_b),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(check_b.allowed, "namespace b should be allowed");

    // Check for namespace "a" -- should be denied (quota exhausted).
    let check_a = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&ns_a),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(!check_a.allowed, "namespace a should be denied");
}

#[test]
fn usage_multiple_metrics_independent() {
    let policy = QuotaPolicy {
        limits: vec![
            QuotaLimit {
                metric: UsageMetric::ToolCall,
                max_units: 5,
                window_ms: 60_000,
                scope: QuotaScope::Tenant,
            },
            QuotaLimit {
                metric: UsageMetric::RunsStarted,
                max_units: 2,
                window_ms: 60_000,
                scope: QuotaScope::Tenant,
            },
        ],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");

    // Record 4 ToolCalls.
    for i in 0 .. 4 {
        meter.record(
            &auth,
            UsageRecord {
                tool: &ToolName::RunpackExport,
                tenant_id: Some(&tenant_id),
                namespace_id: Some(&namespace_id),
                request_id: Some(&format!("tc-{i}")),
                metric: UsageMetric::ToolCall,
                units: 1,
            },
        );
    }

    // Check RunsStarted with 1 unit -> allowed (independent metric).
    let check_runs = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::RunsStarted,
            units: 1,
        },
    );
    assert!(check_runs.allowed, "RunsStarted should still be allowed");

    // Check ToolCall with 2 units -> denied (4 + 2 > 5).
    let check_tc = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 2,
        },
    );
    assert!(!check_tc.allowed, "ToolCall should be denied");
}

#[test]
fn usage_record_none_tenant_silently_returns() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 100,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let namespace_id = NamespaceId::new("default");

    // Record with tenant_id=None should not panic.
    meter.record(
        &auth,
        UsageRecord {
            tool: &ToolName::RunpackExport,
            tenant_id: None,
            namespace_id: Some(&namespace_id),
            request_id: Some("req-1"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );

    // Check with a valid tenant shows 0 usage.
    let tenant_id = TenantId::new("tenant-1");
    let check = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(check.allowed);
}

#[test]
fn usage_record_none_namespace_silently_returns() {
    let policy = QuotaPolicy::default();
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");

    // Record with namespace_id=None should not panic.
    meter.record(
        &auth,
        UsageRecord {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: None,
            request_id: Some("req-1"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
}

#[test]
fn usage_record_none_request_id_no_idempotency() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 10,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_id = TenantId::new("tenant-1");
    let namespace_id = NamespaceId::new("default");

    // Record twice with request_id=None -- no dedup expected.
    for _ in 0 .. 2 {
        meter.record(
            &auth,
            UsageRecord {
                tool: &ToolName::RunpackExport,
                tenant_id: Some(&tenant_id),
                namespace_id: Some(&namespace_id),
                request_id: None,
                metric: UsageMetric::ToolCall,
                units: 1,
            },
        );
    }

    // Check should show 2 units consumed (both recorded).
    // Requesting 8 more should be allowed (2+8=10 <= 10).
    let check_8 = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 8,
        },
    );
    assert!(check_8.allowed, "2+8=10 <= 10 should be allowed");

    // And requesting 9 should be denied (2+9=11 > 10).
    let check_9 = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_id),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 9,
        },
    );
    assert!(!check_9.allowed, "2+9=11 > 10 should be denied");
}

#[test]
fn usage_cross_tenant_isolation() {
    let policy = QuotaPolicy {
        limits: vec![QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 1,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        }],
    };
    let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
    let auth = auth_context();
    let tenant_1 = TenantId::new("tenant-1");
    let tenant_2 = TenantId::new("tenant-2");
    let namespace_id = NamespaceId::new("default");

    // Exhaust quota for tenant-1.
    meter.record(
        &auth,
        UsageRecord {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_1),
            namespace_id: Some(&namespace_id),
            request_id: Some("req-t1"),
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );

    // Tenant-1 should be denied.
    let check_t1 = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_1),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(!check_t1.allowed, "tenant-1 quota should be exhausted");

    // Tenant-2 should still be allowed (isolated).
    let check_t2 = meter.check(
        &auth,
        UsageCheckRequest {
            tool: &ToolName::RunpackExport,
            tenant_id: Some(&tenant_2),
            namespace_id: Some(&namespace_id),
            request_id: None,
            metric: UsageMetric::ToolCall,
            units: 1,
        },
    );
    assert!(check_t2.allowed, "tenant-2 should be allowed (isolated)");
}

#[test]
fn usage_ledger_seen_idempotency_false_for_unseen() {
    let ledger = InMemoryUsageLedger::default();
    let seen =
        decision_gate_enterprise::usage::UsageLedger::seen_idempotency(&ledger, "nonexistent")
            .expect("seen_idempotency");
    assert!(!seen);
}

#[test]
fn usage_ledger_sum_since_specific_namespace() {
    let ledger = InMemoryUsageLedger::default();
    let tenant = TenantId::new("t1");
    let ns_a = NamespaceId::new("ns-a");
    let ns_b = NamespaceId::new("ns-b");

    // Append events for two namespaces.
    decision_gate_enterprise::usage::UsageLedger::append(
        &ledger,
        decision_gate_enterprise::usage::UsageEvent {
            tenant_id: tenant.clone(),
            namespace_id: ns_a,
            metric: UsageMetric::ToolCall,
            units: 5,
            timestamp_ms: 1000,
            idempotency_key: Some("k1".to_string()),
        },
    )
    .expect("append");
    decision_gate_enterprise::usage::UsageLedger::append(
        &ledger,
        decision_gate_enterprise::usage::UsageEvent {
            tenant_id: tenant,
            namespace_id: ns_b,
            metric: UsageMetric::ToolCall,
            units: 3,
            timestamp_ms: 1000,
            idempotency_key: Some("k2".to_string()),
        },
    )
    .expect("append");

    // sum_since for t1/ns-a should count only ns-a events.
    let sum = decision_gate_enterprise::usage::UsageLedger::sum_since(
        &ledger,
        "t1/ns-a",
        UsageMetric::ToolCall,
        0,
    )
    .expect("sum_since");
    assert_eq!(sum, 5);
}

#[test]
fn usage_policy_from_limits_builds_correct_policy() {
    let limits: BTreeMap<UsageMetric, u64> = BTreeMap::new();
    let policy =
        decision_gate_enterprise::usage::policy_from_limits(QuotaScope::Tenant, 60_000, limits);
    assert!(policy.limits.is_empty());
}

#[test]
fn usage_quota_policy_builder_roundtrip() {
    let policy = decision_gate_enterprise::usage::QuotaPolicyBuilder::default()
        .add_limit(QuotaLimit {
            metric: UsageMetric::ToolCall,
            max_units: 10,
            window_ms: 60_000,
            scope: QuotaScope::Tenant,
        })
        .add_limit(QuotaLimit {
            metric: UsageMetric::RunsStarted,
            max_units: 5,
            window_ms: 60_000,
            scope: QuotaScope::Namespace,
        })
        .build();
    assert_eq!(policy.limits.len(), 2);
}

#[test]
fn usage_metric_label_all_variants() {
    // metric_label is pub(crate) and not accessible from external tests.
    // Verify all UsageMetric variants can be used in a QuotaLimit without panic.
    let all_metrics = [
        UsageMetric::ToolCall,
        UsageMetric::RunsStarted,
        UsageMetric::EvidenceQueries,
        UsageMetric::RunpackExports,
        UsageMetric::SchemasWritten,
        UsageMetric::RegistryEntries,
        UsageMetric::StorageBytes,
    ];
    for metric in all_metrics {
        let policy = QuotaPolicy {
            limits: vec![QuotaLimit {
                metric,
                max_units: 1,
                window_ms: 1000,
                scope: QuotaScope::Tenant,
            }],
        };
        let meter = UsageQuotaEnforcer::new(InMemoryUsageLedger::default(), policy);
        let auth = auth_context();
        let tenant_id = TenantId::new("t1");
        let namespace_id = NamespaceId::new("ns");
        // Record should not panic.
        meter.record(
            &auth,
            UsageRecord {
                tool: &ToolName::RunpackExport,
                tenant_id: Some(&tenant_id),
                namespace_id: Some(&namespace_id),
                request_id: None,
                metric,
                units: 1,
            },
        );
        // Check should not panic.
        let _check = meter.check(
            &auth,
            UsageCheckRequest {
                tool: &ToolName::RunpackExport,
                tenant_id: Some(&tenant_id),
                namespace_id: Some(&namespace_id),
                request_id: None,
                metric,
                units: 1,
            },
        );
    }
}
