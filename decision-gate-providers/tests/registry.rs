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
use decision_gate_core::NamespaceId;
use decision_gate_core::ProviderId;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::TrustLane;
use decision_gate_providers::BuiltinProviderConfigs;
use decision_gate_providers::JsonProviderConfig;
use decision_gate_providers::ProviderAccessPolicy;
use decision_gate_providers::ProviderRegistry;
use serde_json::json;
use tempfile::TempDir;

use crate::common::sample_context;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

fn builtin_configs() -> (TempDir, BuiltinProviderConfigs) {
    let dir = tempfile::tempdir().expect("temp dir");
    let json = JsonProviderConfig {
        root: dir.path().to_path_buf(),
        root_id: "registry-root".to_string(),
        max_bytes: 1024 * 1024,
        allow_yaml: true,
    };
    (dir, BuiltinProviderConfigs::new(json))
}

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
            lane: TrustLane::Verified,
            error: None,
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

fn register_ok<P>(registry: &mut ProviderRegistry, provider_id: &str, provider: P)
where
    P: EvidenceProvider + Send + Sync + 'static,
{
    registry
        .register_provider(provider_id, provider)
        .expect("provider registration should succeed");
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
            lane: TrustLane::Verified,
            error: None,
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
    build_spec_with_conditions(&[(provider_id, "check")])
}

/// Builds a spec that references multiple providers via conditions.
fn build_spec_with_conditions(conditions: &[(&str, &str)]) -> ScenarioSpec {
    let gates: Vec<decision_gate_core::GateSpec> = conditions
        .iter()
        .enumerate()
        .map(|(i, (_, condition_id))| decision_gate_core::GateSpec {
            gate_id: decision_gate_core::GateId::new(format!("gate-{i}")),
            requirement: ret_logic::Requirement::condition((*condition_id).into()),
            trust: None,
        })
        .collect();

    let condition_specs: Vec<decision_gate_core::ConditionSpec> = conditions
        .iter()
        .map(|(provider_id, condition_id)| decision_gate_core::ConditionSpec {
            condition_id: (*condition_id).into(),
            query: EvidenceQuery {
                provider_id: ProviderId::new(*provider_id),
                check_id: (*condition_id).to_string(),
                params: Some(json!({})),
            },
            comparator: decision_gate_core::Comparator::Equals,
            expected: Some(json!(true)),
            policy_tags: Vec::new(),
            trust: None,
        })
        .collect();

    ScenarioSpec {
        scenario_id: decision_gate_core::ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: decision_gate_core::StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates,
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: condition_specs,
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
    register_ok(&mut registry, "provider-a", ValueProvider::new("A"));
    register_ok(&mut registry, "provider-b", ValueProvider::new("B"));
    let ctx = sample_context();

    // Query provider A
    let query_a = EvidenceQuery {
        provider_id: ProviderId::new("provider-a"),
        check_id: "test".to_string(),
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
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "provider-a", CountingProvider::new(counter_a.clone()));
    register_ok(&mut registry, "provider-b", CountingProvider::new(counter_b.clone()));
    let ctx = sample_context();

    // Query only provider A
    let query = EvidenceQuery {
        provider_id: ProviderId::new("provider-a"),
        check_id: "test".to_string(),
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
        check_id: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err());
    let EvidenceError::Provider(msg) = result.unwrap_err();
    assert!(msg.contains("not registered"), "Error should mention not registered: {msg}");
}

/// Verifies that registering a provider rejects duplicates.
#[test]
fn register_provider_rejects_duplicates() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    register_ok(&mut registry, "test", ValueProvider::new("first"));
    let err = registry
        .register_provider("test", ValueProvider::new("second"))
        .expect_err("expected duplicate registration failure");
    let EvidenceError::Provider(message) = err;
    assert!(
        message.contains("already registered"),
        "error should mention duplicate registration: {message}"
    );

    let ctx = sample_context();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("test"),
        check_id: "check".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx).unwrap();
    let EvidenceValue::Json(value) = result.value.unwrap() else {
        panic!("Expected JSON value");
    };
    assert_eq!(value["provider"], "first", "Original registration must remain");
}

/// Verifies that calling builtin registration twice fails closed.
#[test]
fn register_builtin_providers_rejects_duplicates() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    let (_dir, configs) = builtin_configs();
    registry
        .register_builtin_providers(configs.clone())
        .expect("builtin registration should succeed");
    let err = registry
        .register_builtin_providers(configs)
        .expect_err("expected duplicate builtin registration failure");
    let EvidenceError::Provider(message) = err;
    assert!(
        message.contains("already registered"),
        "error should mention duplicate registration: {message}"
    );
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
    register_ok(&mut registry, "allowed", ValueProvider::new("allowed"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("allowed"),
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "blocked", ValueProvider::new("blocked"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("blocked"),
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "blocked", ValueProvider::new("blocked"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("blocked"),
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "allowed", ValueProvider::new("allowed"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("allowed"),
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "both", ValueProvider::new("both"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("both"),
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "any", ValueProvider::new("any"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("any"),
        check_id: "test".to_string(),
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
    register_ok(&mut registry, "blocked", DummyProvider);
    let error = registry.validate_providers(&build_spec("blocked")).unwrap_err();
    assert!(error.blocked_by_policy);
    assert_eq!(error.missing_providers, vec!["blocked".to_string()]);
}

/// Verifies validation reports multiple missing providers.
#[test]
fn validate_reports_multiple_missing_providers() {
    let registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    let spec = build_spec_with_conditions(&[
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
        build_spec_with_conditions(&[("missing", "capability-a"), ("missing", "capability-b")]);
    let error = registry.validate_providers(&spec).unwrap_err();
    assert!(error.required_capabilities.contains(&"capability-a".to_string()));
    assert!(error.required_capabilities.contains(&"capability-b".to_string()));
}

/// Verifies validation passes when all providers are present and allowed.
#[test]
fn validate_passes_when_all_providers_present() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    register_ok(&mut registry, "provider-a", DummyProvider);
    register_ok(&mut registry, "provider-b", DummyProvider);
    let spec = build_spec_with_conditions(&[("provider-a", "pred-a"), ("provider-b", "pred-b")]);
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
    register_ok(&mut registry, "allowed", DummyProvider);
    register_ok(&mut registry, "blocked", DummyProvider); // registered but not in allowlist

    let spec = build_spec_with_conditions(&[
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
    let (_dir, configs) = builtin_configs();
    let registry = ProviderRegistry::with_builtin_providers(configs).unwrap();
    let ctx = sample_context();

    // Verify time provider is registered
    let time_query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let time_result = registry.query(&time_query, &ctx);
    assert!(time_result.is_ok(), "time provider should be registered");

    // Verify env provider is registered
    let env_query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "PATH"})),
    };
    let env_result = registry.query(&env_query, &ctx);
    assert!(env_result.is_ok(), "env provider should be registered");

    // Verify json provider is registered by checking it routes to the provider
    // (even though the query params are invalid, it should reach the provider)
    let json_query = EvidenceQuery {
        provider_id: ProviderId::new("json"),
        check_id: "path".to_string(),
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
    register_ok(&mut registry, "any", ValueProvider::new("any"));
    let ctx = sample_context();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("any"),
        check_id: "test".to_string(),
        params: None,
    };
    let result = registry.query(&query, &ctx);
    assert!(result.is_err(), "Empty allowlist should block all providers");
}

/// Verifies case sensitivity in provider IDs.
#[test]
fn provider_ids_are_case_sensitive() {
    let mut registry = ProviderRegistry::new(ProviderAccessPolicy::default());
    register_ok(&mut registry, "Provider", ValueProvider::new("uppercase"));
    let ctx = sample_context();

    // Query with exact case
    let query_exact = EvidenceQuery {
        provider_id: ProviderId::new("Provider"),
        check_id: "test".to_string(),
        params: None,
    };
    assert!(registry.query(&query_exact, &ctx).is_ok());

    // Query with different case should fail
    let query_lower = EvidenceQuery {
        provider_id: ProviderId::new("provider"),
        check_id: "test".to_string(),
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
