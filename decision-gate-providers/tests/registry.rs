// decision-gate-providers/tests/registry.rs
// ============================================================================
// Module: Provider Registry Tests
// Description: Validate provider registry routing, policy, and validation.
// Purpose: Ensure provider routing and security policy enforcement is correct.
// Dependencies: decision-gate-providers, decision-gate-core, serde_json
// ============================================================================
//! ## Overview
//! Covers provider routing, access policy enforcement, and validation behavior.
//!
//! Security posture: Registry is a trust boundary - policy violations must fail closed.
//! Threat model: TM-REG-001 - Provider access control bypass.

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
    reason = "Test-only output and panic-based assertions are permitted."
)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderId;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_providers::ProviderAccessPolicy;
use decision_gate_providers::ProviderRegistry;
use serde_json::json;

use crate::common::sample_context;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

/// Dummy provider that always returns an error for basic tests.
struct DummyProvider;

impl EvidenceProvider for DummyProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Err(EvidenceError::Provider("dummy".to_string()))
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

/// Provider that returns a configured value, useful for verifying routing.
struct ValueProvider {
    name: String,
}

impl ValueProvider {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
        }
    }
}

impl EvidenceProvider for ValueProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!({"provider": self.name}))),
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        })
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

/// Provider that tracks call count to verify routing.
struct CountingProvider {
    call_count: Arc<AtomicU32>,
}

impl CountingProvider {
    const fn new(counter: Arc<AtomicU32>) -> Self {
        Self {
            call_count: counter,
        }
    }
}

impl EvidenceProvider for CountingProvider {
    fn query(
        &self,
        _query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(EvidenceResult {
            value: Some(EvidenceValue::Json(json!({"called": true}))),
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        })
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

/// Builds a spec that references a single provider.
fn build_spec(provider_id: &str) -> ScenarioSpec {
    build_spec_with_predicates(&[(provider_id, "check")])
}

/// Builds a spec that references multiple providers via predicates.
fn build_spec_with_predicates(predicates: &[(&str, &str)]) -> ScenarioSpec {
    let gates: Vec<decision_gate_core::GateSpec> = predicates
        .iter()
        .enumerate()
        .map(|(i, (_, pred))| decision_gate_core::GateSpec {
            gate_id: decision_gate_core::GateId::new(format!("gate-{i}")),
            requirement: ret_logic::Requirement::predicate((*pred).into()),
        })
        .collect();

    let predicate_specs: Vec<decision_gate_core::PredicateSpec> = predicates
        .iter()
        .map(|(provider_id, pred)| decision_gate_core::PredicateSpec {
            predicate: (*pred).into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new(*provider_id),
                predicate: (*pred).to_string(),
                params: Some(json!({})),
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
        })
        .collect();

    ScenarioSpec {
        scenario_id: decision_gate_core::ScenarioId::new("scenario"),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: decision_gate_core::StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates,
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        predicates: predicate_specs,
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

// ============================================================================
// SECTION: Query Routing Tests
// ============================================================================

/// Verifies that queries are routed to the correct provider by name.
#[test]
fn query_routes_to_correct_provider() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    registry.register_provider("provider-a", ValueProvider::new("A"));
    registry.register_provider("provider-b", ValueProvider::new("B"));
    let ctx = sample_context();

    // Query provider A
    let query_a = EvidenceQuery {
        provider_id: ProviderId::new("provider-a"),
        predicate: "test".to_string(),
        params: None,
    };
    let result_a = registry.query(&query_a, &ctx).unwrap();
    let EvidenceValue::Json(value_a) = result_a.value.unwrap() else {
        panic!("Expected JSON value");
    };
    assert_eq!(value_a["provider"], "A");

    // Query provider B
    let query_b = EvidenceQuery {
        provider_id: ProviderId::new("provider-b"),
        predicate: "test".to_string(),
        params: None,
    };
    let result_b = registry.query(&query_b, &ctx).unwrap();
    let EvidenceValue::Json(value_b) = result_b.value.unwrap() else {
        panic!("Expected JSON value");
    };
    assert_eq!(value_b["provider"], "B");
}

/// Verifies that only the targeted provider receives the query.
#[test]
fn query_only_calls_targeted_provider() {
    let counter_a = Arc::new(AtomicU32::new(0));
    let counter_b = Arc::new(AtomicU32::new(0));

    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    registry.register_provider("provider-a", CountingProvider::new(counter_a.clone()));
    registry.register_provider("provider-b", CountingProvider::new(counter_b.clone()));
    let ctx = sample_context();

    // Query only provider A
    let query = EvidenceQuery {
        provider_id: ProviderId::new("provider-a"),
        predicate: "test".to_string(),
        params: None,
    };
    registry.query(&query, &ctx).unwrap();

    assert_eq!(counter_a.load(Ordering::SeqCst), 1, "Provider A should be called");
    assert_eq!(counter_b.load(Ordering::SeqCst), 0, "Provider B should NOT be called");
}

/// Verifies that querying an unregistered provider returns an error.
#[test]
fn query_unregistered_provider_fails() {
    let registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("nonexistent"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err());
    let EvidenceError::Provider(msg) = result.unwrap_err();
    assert!(msg.contains("not registered"), "Error should mention not registered: {msg}");
}

/// Verifies that registering a provider replaces any existing one.
#[test]
fn register_provider_replaces_existing() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    registry.register_provider("test", ValueProvider::new("first"));
    registry.register_provider("test", ValueProvider::new("second"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("test"),
        predicate: "check".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx).unwrap();
    let EvidenceValue::Json(value) = result.value.unwrap() else {
        panic!("Expected JSON value");
    };
    assert_eq!(value["provider"], "second", "Second registration should replace first");
}

// ============================================================================
// SECTION: Policy Enforcement Tests - Allowlist
// ============================================================================

/// Verifies that allowlist permits only specified providers.
#[test]
fn policy_allowlist_permits_listed_providers() {
    let mut allowlist = BTreeSet::new();
    allowlist.insert("allowed".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: Some(allowlist),
        denylist: BTreeSet::new(),
    };

    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("allowed", ValueProvider::new("allowed"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("allowed"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_ok(), "Allowlisted provider should be permitted");
}

/// Verifies that allowlist blocks non-listed providers.
#[test]
fn policy_allowlist_blocks_unlisted_providers() {
    let mut allowlist = BTreeSet::new();
    allowlist.insert("allowed".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: Some(allowlist),
        denylist: BTreeSet::new(),
    };

    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("blocked", ValueProvider::new("blocked"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("blocked"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err());
    let EvidenceError::Provider(msg) = result.unwrap_err();
    assert!(msg.contains("blocked by policy"), "Error: {msg}");
}

// ============================================================================
// SECTION: Policy Enforcement Tests - Denylist
// ============================================================================

/// Verifies that denylist blocks specified providers.
#[test]
fn policy_denylist_blocks_listed_providers() {
    let mut denylist = BTreeSet::new();
    denylist.insert("blocked".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: None,
        denylist,
    };

    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("blocked", ValueProvider::new("blocked"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("blocked"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err());
    let EvidenceError::Provider(msg) = result.unwrap_err();
    assert!(msg.contains("blocked by policy"), "Error: {msg}");
}

/// Verifies that denylist allows non-listed providers.
#[test]
fn policy_denylist_permits_unlisted_providers() {
    let mut denylist = BTreeSet::new();
    denylist.insert("blocked".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: None,
        denylist,
    };

    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("allowed", ValueProvider::new("allowed"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("allowed"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_ok(), "Non-denylisted provider should be permitted");
}

/// Verifies that denylist takes precedence over allowlist.
#[test]
fn policy_denylist_takes_precedence_over_allowlist() {
    let mut allowlist = BTreeSet::new();
    allowlist.insert("both".to_string());
    let mut denylist = BTreeSet::new();
    denylist.insert("both".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: Some(allowlist),
        denylist,
    };

    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("both", ValueProvider::new("both"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("both"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err(), "Denylist should take precedence over allowlist");
}

/// Verifies that `allow_all` policy permits any registered provider.
#[test]
fn policy_allow_all_permits_any_provider() {
    let policy = ProviderAccessPolicy::allow_all();

    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("any", ValueProvider::new("any"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("any"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_ok(), "allow_all policy should permit any provider");
}

// ============================================================================
// SECTION: Validation Tests
// ============================================================================

/// Verifies registry reports missing provider correctly.
#[test]
fn validate_reports_missing_provider() {
    let registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    let error = registry.validate_providers(&build_spec("missing")).unwrap_err();
    assert_eq!(error.missing_providers, vec!["missing".to_string()]);
    assert!(!error.blocked_by_policy);
}

/// Verifies registry reports blocked provider with policy flag.
#[test]
fn validate_reports_blocked_provider() {
    let mut allowlist = BTreeSet::new();
    allowlist.insert("other".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: Some(allowlist),
        denylist: BTreeSet::new(),
    };
    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("blocked", DummyProvider);
    let error = registry.validate_providers(&build_spec("blocked")).unwrap_err();
    assert!(error.blocked_by_policy);
    assert_eq!(error.missing_providers, vec!["blocked".to_string()]);
}

/// Verifies validation reports multiple missing providers.
#[test]
fn validate_reports_multiple_missing_providers() {
    let registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    let spec = build_spec_with_predicates(&[
        ("missing-a", "pred-a"),
        ("missing-b", "pred-b"),
        ("missing-c", "pred-c"),
    ]);
    let error = registry.validate_providers(&spec).unwrap_err();
    assert_eq!(error.missing_providers.len(), 3);
    assert!(error.missing_providers.contains(&"missing-a".to_string()));
    assert!(error.missing_providers.contains(&"missing-b".to_string()));
    assert!(error.missing_providers.contains(&"missing-c".to_string()));
    assert!(!error.blocked_by_policy);
}

/// Verifies validation reports required capabilities for missing providers.
#[test]
fn validate_reports_required_capabilities() {
    let registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    let spec =
        build_spec_with_predicates(&[("missing", "capability-a"), ("missing", "capability-b")]);
    let error = registry.validate_providers(&spec).unwrap_err();
    assert!(error.required_capabilities.contains(&"capability-a".to_string()));
    assert!(error.required_capabilities.contains(&"capability-b".to_string()));
}

/// Verifies validation passes when all providers are present and allowed.
#[test]
fn validate_passes_when_all_providers_present() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    registry.register_provider("provider-a", DummyProvider);
    registry.register_provider("provider-b", DummyProvider);
    let spec = build_spec_with_predicates(&[("provider-a", "pred-a"), ("provider-b", "pred-b")]);
    let result = registry.validate_providers(&spec);
    assert!(result.is_ok());
}

/// Verifies validation handles mix of present, missing, and blocked providers.
#[test]
fn validate_reports_mixed_missing_and_blocked() {
    let mut allowlist = BTreeSet::new();
    allowlist.insert("allowed".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: Some(allowlist),
        denylist: BTreeSet::new(),
    };
    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("allowed", DummyProvider);
    registry.register_provider("blocked", DummyProvider); // registered but not in allowlist

    let spec = build_spec_with_predicates(&[
        ("allowed", "pred-allowed"),
        ("blocked", "pred-blocked"),
        ("missing", "pred-missing"),
    ]);
    let error = registry.validate_providers(&spec).unwrap_err();

    // Both blocked and missing should appear in missing_providers
    assert_eq!(error.missing_providers.len(), 2);
    assert!(error.missing_providers.contains(&"blocked".to_string()));
    assert!(error.missing_providers.contains(&"missing".to_string()));
    // blocked_by_policy should be true because "blocked" is registered but not allowed
    assert!(error.blocked_by_policy);
}

// ============================================================================
// SECTION: Builtin Providers Tests
// ============================================================================

/// Verifies builtin providers are registered with expected names.
#[test]
fn builtin_providers_registers_expected_providers() {
    let registry = ProviderRegistry::with_builtin_providers().unwrap();
    let ctx = sample_context();

    // Verify time provider is registered
    let time_query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        predicate: "now".to_string(),
        params: None,
    };
    let time_result = registry.query(&time_query, &ctx);
    assert!(time_result.is_ok(), "time provider should be registered");

    // Verify env provider is registered
    let env_query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        predicate: "get".to_string(),
        params: Some(json!({"key": "PATH"})),
    };
    let env_result = registry.query(&env_query, &ctx);
    assert!(env_result.is_ok(), "env provider should be registered");

    // Verify json provider is registered by checking it routes to the provider
    // (even though the query params are invalid, it should reach the provider)
    let json_query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        predicate: "path".to_string(),
        params: Some(json!({"file": "nonexistent.json", "jsonpath": "$.test"})),
    };
    let json_result = registry.query(&json_query, &ctx);
    // The provider is registered if we get a Provider error (not "not registered")
    match json_result {
        Ok(_) => (), // Unexpected but acceptable
        Err(EvidenceError::Provider(msg)) => {
            assert!(!msg.contains("not registered"), "json provider should be registered: {msg}");
        }
    }
}

/// Verifies policy accessor returns the configured policy.
#[test]
fn policy_accessor_returns_configured_policy() {
    let mut denylist = BTreeSet::new();
    denylist.insert("denied".to_string());
    let policy = ProviderAccessPolicy {
        allowlist: None,
        denylist: denylist.clone(),
    };
    let registry = ProviderRegistry::new(policy.clone());

    assert_eq!(registry.policy(), &policy);
    assert!(registry.policy().denylist.contains("denied"));
}

// ============================================================================
// SECTION: Edge Cases
// ============================================================================

/// Verifies empty allowlist blocks all providers.
#[test]
fn empty_allowlist_blocks_all_providers() {
    let policy = ProviderAccessPolicy {
        allowlist: Some(BTreeSet::new()), // Empty allowlist
        denylist: BTreeSet::new(),
    };
    let mut registry = ProviderRegistry::new(policy);
    registry.register_provider("any", ValueProvider::new("any"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("any"),
        predicate: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err(), "Empty allowlist should block all providers");
}

/// Verifies case sensitivity in provider IDs.
#[test]
fn provider_ids_are_case_sensitive() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    registry.register_provider("Provider", ValueProvider::new("uppercase"));
    let ctx = sample_context();

    // Query with exact case
    let query_exact = EvidenceQuery {
        provider_id: ProviderId::new("Provider"),
        predicate: "test".to_string(),
        params: None,
    };
    assert!(registry.query(&query_exact, &ctx).is_ok());

    // Query with different case should fail
    let query_lower = EvidenceQuery {
        provider_id: ProviderId::new("provider"),
        predicate: "test".to_string(),
        params: None,
    };
    assert!(registry.query(&query_lower, &ctx).is_err());
}

/// Verifies policy `is_allowed` checks both allowlist and denylist correctly.
#[test]
fn policy_is_allowed_logic() {
    // Test 1: No allowlist, empty denylist - allows all
    let policy1 = ProviderAccessPolicy::allow_all();
    assert!(policy1.is_allowed("any"));

    // Test 2: Allowlist with entry - only allows listed
    let mut allowlist = BTreeSet::new();
    allowlist.insert("listed".to_string());
    let policy2 = ProviderAccessPolicy {
        allowlist: Some(allowlist),
        denylist: BTreeSet::new(),
    };
    assert!(policy2.is_allowed("listed"));
    assert!(!policy2.is_allowed("unlisted"));

    // Test 3: Denylist - blocks listed, allows others
    let mut denylist = BTreeSet::new();
    denylist.insert("blocked".to_string());
    let policy3 = ProviderAccessPolicy {
        allowlist: None,
        denylist,
    };
    assert!(!policy3.is_allowed("blocked"));
    assert!(policy3.is_allowed("unblocked"));

    // Test 4: Both lists - denylist takes precedence
    let mut allowlist4 = BTreeSet::new();
    allowlist4.insert("both".to_string());
    let mut denylist4 = BTreeSet::new();
    denylist4.insert("both".to_string());
    let policy4 = ProviderAccessPolicy {
        allowlist: Some(allowlist4),
        denylist: denylist4,
    };
    assert!(!policy4.is_allowed("both"), "Denylist takes precedence");
}

/// Verifies default policy is `allow_all`.
#[test]
fn default_policy_is_allow_all() {
    let policy = ProviderAccessPolicy::default();
    assert!(policy.allowlist.is_none());
    assert!(policy.denylist.is_empty());
    assert!(policy.is_allowed("any_provider"));
}
