// decision-gate-mcp/tests/evidence_policy.rs
// ============================================================================
// Module: Evidence Policy Tests
// Description: Tests for evidence disclosure policy enforcement.
// Purpose: Verify raw value redaction and provider opt-in are enforced.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Tests evidence disclosure policy enforcement including raw value redaction
//! and provider opt-in requirements.
//!
//! Security posture: Evidence disclosure is a trust boundary - raw values
//! should only be disclosed when policy explicitly permits.
//! Threat model: TM-EVI-001 - Information disclosure via evidence queries.

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

use decision_gate_core::EvidenceQuery;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::ProviderId;
use decision_gate_mcp::FederatedEvidenceProvider;
use decision_gate_mcp::McpNoopAuditSink;
use decision_gate_mcp::ToolRouter;
use decision_gate_mcp::auth::DefaultToolAuthz;
use decision_gate_mcp::auth::NoopAuditSink;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::tools::EvidenceQueryRequest;
use decision_gate_mcp::tools::EvidenceQueryResponse;
use decision_gate_mcp::tools::ToolRouterConfig;
use serde_json::json;

use crate::common::local_request_context;
use crate::common::sample_context;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

fn router_with_policy(policy: EvidencePolicyConfig) -> ToolRouter {
    let mut config = common::sample_config();
    config.evidence = policy;
    let evidence = FederatedEvidenceProvider::from_config(&config).unwrap();
    let capabilities = CapabilityRegistry::from_config(&config).unwrap();
    let store = decision_gate_core::SharedRunStateStore::from_store(
        decision_gate_core::InMemoryRunStateStore::new(),
    );
    let schema_registry = decision_gate_core::SharedDataShapeRegistry::from_registry(
        decision_gate_core::InMemoryDataShapeRegistry::new(),
    );
    let provider_transports = config
        .providers
        .iter()
        .map(|provider| {
            let transport = match provider.provider_type {
                decision_gate_mcp::config::ProviderType::Builtin => {
                    decision_gate_mcp::tools::ProviderTransport::Builtin
                }
                decision_gate_mcp::config::ProviderType::Mcp => {
                    decision_gate_mcp::tools::ProviderTransport::Mcp
                }
            };
            (provider.name.clone(), transport)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let schema_registry_limits = decision_gate_mcp::tools::SchemaRegistryLimits {
        max_schema_bytes: config.schema_registry.max_schema_bytes,
        max_entries: config
            .schema_registry
            .max_entries
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
    };
    let trust_requirement = config.effective_trust_requirement();
    let allow_default_namespace = config.allow_default_namespace();
    let authz = std::sync::Arc::new(DefaultToolAuthz::from_config(config.server.auth.as_ref()));
    let audit = std::sync::Arc::new(NoopAuditSink);
    ToolRouter::new(ToolRouterConfig {
        evidence,
        evidence_policy: config.evidence,
        validation: config.validation,
        dispatch_policy: config.policy.dispatch_policy().expect("dispatch policy"),
        store,
        schema_registry,
        provider_transports,
        schema_registry_limits,
        capabilities: std::sync::Arc::new(capabilities),
        authz,
        audit,
        trust_requirement,
        precheck_audit: std::sync::Arc::new(McpNoopAuditSink),
        precheck_audit_payloads: config.server.audit.log_precheck_payloads,
        allow_default_namespace,
    })
}

fn query_time_now(router: &ToolRouter) -> EvidenceQueryResponse {
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("time"),
            predicate: "now".to_string(),
            params: None,
        },
        context: sample_context(),
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "evidence_query",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    serde_json::from_value(result).unwrap()
}

fn query_env_path(router: &ToolRouter) -> EvidenceQueryResponse {
    let request = EvidenceQueryRequest {
        query: EvidenceQuery {
            provider_id: ProviderId::new("env"),
            predicate: "get".to_string(),
            params: Some(json!({"key": "PATH"})),
        },
        context: sample_context(),
    };
    let result = router
        .handle_tool_call(
            &local_request_context(),
            "evidence_query",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    serde_json::from_value(result).unwrap()
}

// ============================================================================
// SECTION: Default Policy Tests
// ============================================================================

/// Verifies default policy redacts raw values.
#[test]
fn default_policy_redacts_raw_values() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    // Default: allow_raw_values = false
    assert!(response.result.value.is_none(), "Raw values should be redacted by default");
}

/// Verifies default policy still returns evidence hash.
#[test]
fn default_policy_returns_hash() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(response.result.evidence_hash.is_some(), "Evidence hash should always be present");
}

/// Verifies default policy redacts content type.
#[test]
fn default_policy_redacts_content_type() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(
        response.result.content_type.is_none(),
        "Content type should be redacted with raw values"
    );
}

// ============================================================================
// SECTION: Raw Values Allowed Tests
// ============================================================================

/// Verifies allowing raw values returns the value.
#[test]
fn allow_raw_values_returns_value() {
    let policy = EvidencePolicyConfig {
        allow_raw_values: true,
        require_provider_opt_in: false,
    };
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(response.result.value.is_some(), "Raw values should be returned when allowed");
}

/// Verifies allowing raw values also includes content type.
#[test]
fn allow_raw_values_includes_content_type() {
    let policy = EvidencePolicyConfig {
        allow_raw_values: true,
        require_provider_opt_in: false,
    };
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(
        response.result.content_type.is_some(),
        "Content type should be included with raw values"
    );
}

// ============================================================================
// SECTION: Provider Opt-In Tests
// ============================================================================

/// Verifies `require_provider_opt_in` blocks raw values for non-opted providers.
#[test]
fn provider_opt_in_required_blocks_non_opted() {
    let policy = EvidencePolicyConfig {
        allow_raw_values: true,
        require_provider_opt_in: true,
    };
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    // Builtin time provider doesn't opt-in by default
    assert!(
        response.result.value.is_none(),
        "Raw values should be blocked when provider hasn't opted in"
    );
}

/// Verifies hash is still provided even when provider hasn't opted in.
#[test]
fn provider_opt_in_still_returns_hash() {
    let policy = EvidencePolicyConfig {
        allow_raw_values: true,
        require_provider_opt_in: true,
    };
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(
        response.result.evidence_hash.is_some(),
        "Evidence hash should be present even without provider opt-in"
    );
}

// ============================================================================
// SECTION: Policy Combination Tests
// ============================================================================

/// Verifies `allow_raw_values=false` takes precedence over opt-in.
#[test]
fn allow_raw_false_takes_precedence() {
    // Even if opt-in is not required, raw values disabled = no values
    let policy = EvidencePolicyConfig {
        allow_raw_values: false,
        require_provider_opt_in: false,
    };
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(
        response.result.value.is_none(),
        "Raw values should be redacted when allow_raw_values=false"
    );
}

/// Verifies both conditions must be met: allowed AND (not required OR opted-in).
#[test]
fn both_conditions_required_for_raw_values() {
    // Test all 4 combinations:
    // 1. allow=false, require_opt_in=false -> no value
    // 2. allow=false, require_opt_in=true -> no value
    // 3. allow=true, require_opt_in=true -> no value (provider hasn't opted in)
    // 4. allow=true, require_opt_in=false -> has value

    let combinations = [
        (false, false, false), // no value expected
        (false, true, false),  // no value expected
        (true, true, false),   // no value expected (no opt-in)
        (true, false, true),   // value expected
    ];

    for (allow_raw, require_opt_in, expect_value) in combinations {
        let policy = EvidencePolicyConfig {
            allow_raw_values: allow_raw,
            require_provider_opt_in: require_opt_in,
        };
        let router = router_with_policy(policy);
        let response = query_time_now(&router);

        assert_eq!(
            response.result.value.is_some(),
            expect_value,
            "Mismatch for allow={allow_raw}, require_opt_in={require_opt_in}"
        );
    }
}

// ============================================================================
// SECTION: Multiple Provider Tests
// ============================================================================

/// Verifies policy applies consistently across providers.
#[test]
fn policy_applies_to_all_providers() {
    let policy = EvidencePolicyConfig {
        allow_raw_values: false,
        require_provider_opt_in: true,
    };
    let router = router_with_policy(policy);

    // Query time provider
    let time_response = query_time_now(&router);
    assert!(time_response.result.value.is_none());

    // Query env provider
    let env_response = query_env_path(&router);
    assert!(env_response.result.value.is_none());
}

/// Verifies each query computes its own evidence hash.
#[test]
fn each_query_has_unique_hash() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);

    let time_response = query_time_now(&router);
    let env_response = query_env_path(&router);

    let time_hash = time_response.result.evidence_hash.unwrap();
    let env_hash = env_response.result.evidence_hash.unwrap();

    // Different queries should produce different hashes
    assert_ne!(time_hash.value, env_hash.value);
}

// ============================================================================
// SECTION: Evidence Hash Computation Tests
// ============================================================================

/// Verifies hash is computed when not provided by provider.
#[test]
fn hash_computed_when_missing() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    assert!(response.result.evidence_hash.is_some());
    assert!(!response.result.evidence_hash.unwrap().value.is_empty());
}

/// Verifies hash algorithm is SHA256.
#[test]
fn hash_uses_sha256() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);
    let response = query_time_now(&router);

    let hash = response.result.evidence_hash.unwrap();
    assert_eq!(hash.algorithm, HashAlgorithm::Sha256);
}

/// Verifies hash is deterministic for same evidence.
#[test]
fn hash_is_deterministic() {
    let policy = EvidencePolicyConfig::default();
    let router = router_with_policy(policy);

    // Query twice with same logical timestamp context
    let response1 = query_time_now(&router);
    let response2 = query_time_now(&router);

    let hash1 = response1.result.evidence_hash.unwrap();
    let hash2 = response2.result.evidence_hash.unwrap();

    // Same context = same evidence = same hash
    assert_eq!(hash1.value, hash2.value);
}
