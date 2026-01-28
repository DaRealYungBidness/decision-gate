// enterprise/decision-gate-enterprise/src/usage.rs
// ============================================================================
// Module: Enterprise Usage Metering
// Description: Usage ledger, quota policy, and enforcement for managed cloud.
// Purpose: Provide billing-grade usage tracking with fail-closed quotas.
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use decision_gate_mcp::AuthContext;
use decision_gate_mcp::UsageCheckRequest;
use decision_gate_mcp::UsageDecision;
use decision_gate_mcp::UsageMeter;
use decision_gate_mcp::UsageMetric;
use decision_gate_mcp::UsageRecord;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

/// Usage scope for quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaScope {
    /// Quotas scoped per tenant.
    Tenant,
    /// Quotas scoped per namespace.
    Namespace,
}

/// Quota limit definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaLimit {
    /// Usage metric covered by the quota.
    pub metric: UsageMetric,
    /// Maximum units allowed within the window.
    pub max_units: u64,
    /// Window size in milliseconds.
    pub window_ms: u64,
    /// Scope for the quota.
    pub scope: QuotaScope,
}

/// Quota policy configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaPolicy {
    /// Quota limits enforced by the policy.
    pub limits: Vec<QuotaLimit>,
}

/// Usage ledger event.
#[derive(Debug, Clone)]
pub struct UsageEvent {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Usage metric.
    pub metric: UsageMetric,
    /// Units consumed.
    pub units: u64,
    /// Event timestamp (milliseconds since epoch).
    pub timestamp_ms: u128,
    /// Optional idempotency key.
    pub idempotency_key: Option<String>,
}

pub(crate) const fn metric_label(metric: UsageMetric) -> &'static str {
    match metric {
        UsageMetric::ToolCall => "tool_calls",
        UsageMetric::RunsStarted => "runs_started",
        UsageMetric::EvidenceQueries => "evidence_queries",
        UsageMetric::RunpackExports => "runpack_exports",
        UsageMetric::SchemasWritten => "schemas_written",
        UsageMetric::RegistryEntries => "registry_entries",
        UsageMetric::StorageBytes => "storage_bytes",
    }
}

/// Usage ledger errors.
#[derive(Debug, Error)]
pub enum UsageLedgerError {
    /// Ledger storage error.
    #[error("usage ledger error: {0}")]
    Storage(String),
}

/// Append-only usage ledger interface.
pub trait UsageLedger: Send + Sync {
    /// Appends an event to the ledger.
    ///
    /// # Errors
    ///
    /// Returns [`UsageLedgerError`] when the ledger cannot persist the event.
    fn append(&self, event: UsageEvent) -> Result<(), UsageLedgerError>;

    /// Returns total units for the metric within the window for the scope key.
    ///
    /// # Errors
    ///
    /// Returns [`UsageLedgerError`] when the ledger query fails.
    fn sum_since(
        &self,
        scope_key: &str,
        metric: UsageMetric,
        since_ms: u128,
    ) -> Result<u64, UsageLedgerError>;

    /// Returns true if the idempotency key has already been seen.
    ///
    /// # Errors
    ///
    /// Returns [`UsageLedgerError`] when the ledger lookup fails.
    fn seen_idempotency(&self, key: &str) -> Result<bool, UsageLedgerError>;
}

/// In-memory usage ledger (test/dev only).
#[derive(Default)]
pub struct InMemoryUsageLedger {
    /// Stored usage events, keyed by scope.
    events: Mutex<Vec<(String, UsageEvent)>>,
    /// Idempotency keys that have been observed.
    seen: Mutex<BTreeSet<String>>,
}

impl UsageLedger for InMemoryUsageLedger {
    fn append(&self, event: UsageEvent) -> Result<(), UsageLedgerError> {
        let scope_key = format!("{}/{}", event.tenant_id.as_str(), event.namespace_id.as_str());
        if let Some(key) = &event.idempotency_key {
            let mut seen = self
                .seen
                .lock()
                .map_err(|_| UsageLedgerError::Storage("ledger lock poisoned".to_string()))?;
            if seen.contains(key) {
                return Ok(());
            }
            seen.insert(key.clone());
        }
        {
            let mut events = self
                .events
                .lock()
                .map_err(|_| UsageLedgerError::Storage("ledger lock poisoned".to_string()))?;
            events.push((scope_key, event));
        }
        Ok(())
    }

    fn sum_since(
        &self,
        scope_key: &str,
        metric: UsageMetric,
        since_ms: u128,
    ) -> Result<u64, UsageLedgerError> {
        let mut total: u64 = 0;
        let (tenant_key, namespace_key) =
            scope_key.split_once('/').map_or((scope_key, None), |(t, n)| (t, Some(n)));
        {
            let events = self
                .events
                .lock()
                .map_err(|_| UsageLedgerError::Storage("ledger lock poisoned".to_string()))?;
            for (key, event) in events.iter() {
                if event.metric != metric || event.timestamp_ms < since_ms {
                    continue;
                }
                if let Some(ns) = namespace_key {
                    if ns == "*" {
                        if event.tenant_id.as_str() == tenant_key {
                            total = total.saturating_add(event.units);
                        }
                    } else if key == scope_key {
                        total = total.saturating_add(event.units);
                    }
                } else if event.tenant_id.as_str() == tenant_key {
                    total = total.saturating_add(event.units);
                }
            }
        }
        Ok(total)
    }

    fn seen_idempotency(&self, key: &str) -> Result<bool, UsageLedgerError> {
        let seen = self
            .seen
            .lock()
            .map_err(|_| UsageLedgerError::Storage("ledger lock poisoned".to_string()))?;
        Ok(seen.contains(key))
    }
}

/// Usage meter that enforces quotas via a usage ledger.
pub struct UsageQuotaEnforcer<L: UsageLedger> {
    /// Ledger backend used to persist events.
    ledger: L,
    /// Quota policy enforced by this meter.
    policy: QuotaPolicy,
}

impl<L: UsageLedger> UsageQuotaEnforcer<L> {
    /// Builds a new quota enforcer.
    #[must_use]
    pub const fn new(ledger: L, policy: QuotaPolicy) -> Self {
        Self {
            ledger,
            policy,
        }
    }

    /// Returns current wall-clock time in milliseconds since epoch.
    fn now_ms() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
    }

    /// Builds a quota scope key for a tenant + namespace.
    fn scope_key(scope: QuotaScope, tenant_id: &TenantId, namespace_id: &NamespaceId) -> String {
        match scope {
            QuotaScope::Tenant => format!("{}/{}", tenant_id.as_str(), "*"),
            QuotaScope::Namespace => format!("{}/{}", tenant_id.as_str(), namespace_id.as_str()),
        }
    }
}

impl<L: UsageLedger> UsageMeter for UsageQuotaEnforcer<L> {
    fn check(&self, _auth: &AuthContext, request: UsageCheckRequest<'_>) -> UsageDecision {
        let Some(tenant_id) = request.tenant_id else {
            return UsageDecision {
                allowed: false,
                reason: "missing_tenant_id".to_string(),
            };
        };
        let Some(namespace_id) = request.namespace_id else {
            return UsageDecision {
                allowed: false,
                reason: "missing_namespace_id".to_string(),
            };
        };
        for limit in &self.policy.limits {
            if limit.metric != request.metric {
                continue;
            }
            let since_ms = Self::now_ms().saturating_sub(u128::from(limit.window_ms));
            let key = Self::scope_key(limit.scope, tenant_id, namespace_id);
            let used = match self.ledger.sum_since(&key, request.metric, since_ms) {
                Ok(value) => value,
                Err(err) => {
                    return UsageDecision {
                        allowed: false,
                        reason: err.to_string(),
                    };
                }
            };
            if used.saturating_add(request.units) > limit.max_units {
                return UsageDecision {
                    allowed: false,
                    reason: "quota_exceeded".to_string(),
                };
            }
        }
        UsageDecision {
            allowed: true,
            reason: "allowed".to_string(),
        }
    }

    fn record(&self, _auth: &AuthContext, record: UsageRecord<'_>) {
        let tenant_id = match record.tenant_id {
            Some(id) => id.clone(),
            None => return,
        };
        let namespace_id = match record.namespace_id {
            Some(id) => id.clone(),
            None => return,
        };
        let idempotency_key = record.request_id.map(|id| {
            format!(
                "{}:{}:{}:{}",
                tenant_id.as_str(),
                namespace_id.as_str(),
                metric_label(record.metric),
                id
            )
        });
        let event = UsageEvent {
            tenant_id,
            namespace_id,
            metric: record.metric,
            units: record.units,
            timestamp_ms: Self::now_ms(),
            idempotency_key,
        };
        let _ = self.ledger.append(event);
    }
}

/// Helper for building quota policies from simple maps.
#[derive(Debug, Clone, Default)]
pub struct QuotaPolicyBuilder {
    /// Accumulated quota limits.
    limits: Vec<QuotaLimit>,
}

impl QuotaPolicyBuilder {
    /// Adds a quota limit.
    #[must_use]
    pub fn add_limit(mut self, limit: QuotaLimit) -> Self {
        self.limits.push(limit);
        self
    }

    /// Builds the quota policy.
    #[must_use]
    pub fn build(self) -> QuotaPolicy {
        QuotaPolicy {
            limits: self.limits,
        }
    }
}

/// Converts a simple per-metric quota map into a policy.
#[must_use]
pub fn policy_from_limits(
    scope: QuotaScope,
    window_ms: u64,
    limits: BTreeMap<UsageMetric, u64>,
) -> QuotaPolicy {
    let mut builder = QuotaPolicyBuilder::default();
    for (metric, max_units) in limits {
        builder = builder.add_limit(QuotaLimit {
            metric,
            max_units,
            window_ms,
            scope,
        });
    }
    builder.build()
}
