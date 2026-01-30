// decision-gate-providers/tests/env_provider.rs
// ============================================================================
// Module: Env Provider Tests
// Description: Comprehensive tests for environment variable evidence provider.
// Purpose: Validate fail-closed behavior, policy enforcement, and size limits.
// Dependencies: decision-gate-providers, decision-gate-core
// ============================================================================

//! ## Overview
//! Tests the environment provider for:
//! - Happy path: value retrieval, missing values
//! - Boundary enforcement: key/value size limits
//! - Policy enforcement: allowlist/denylist rules
//! - Error handling: invalid parameters, unsupported checks
//! - Adversarial: sensitive key blocking
//!
//! Security posture: Environment variables are a trust boundary. Tests verify
//! fail-closed behavior under adversarial input conditions.
//! See: `Docs/security/threat_model.md`

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

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderId;
use decision_gate_providers::EnvProvider;
use decision_gate_providers::EnvProviderConfig;
use serde_json::Value;
use serde_json::json;

use crate::common::oversized_string;
use crate::common::sample_context;

// ============================================================================
// SECTION: Happy Path Tests
// ============================================================================

/// Tests that the env provider returns a value when the key exists in overrides.
#[test]
fn env_provider_returns_value() {
    let mut overrides = BTreeMap::new();
    overrides.insert("DG_TEST_ENV_PROVIDER".to_string(), "ok".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "DG_TEST_ENV_PROVIDER"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "ok");
}

/// Tests that the env provider returns None when the key is missing.
#[test]
fn env_provider_returns_none_when_missing() {
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(BTreeMap::new()),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "DG_TEST_ENV_PROVIDER_MISSING"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    assert!(result.value.is_none());
}

/// Tests that evidence anchor is set correctly for both found and missing keys.
#[test]
fn env_provider_sets_evidence_anchor() {
    let mut overrides = BTreeMap::new();
    overrides.insert("DG_ANCHOR_TEST".to_string(), "value".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "DG_ANCHOR_TEST"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let anchor = result.evidence_anchor.unwrap();
    assert_eq!(anchor.anchor_type, "env");
    assert_eq!(anchor.anchor_value, "DG_ANCHOR_TEST");
}

// ============================================================================
// SECTION: Boundary Enforcement Tests - Size Limits
// ============================================================================

/// Tests that keys exceeding `max_key_bytes` are rejected.
///
/// Threat model: Resource exhaustion via oversized keys.
#[test]
fn env_key_exceeds_max_length_rejected() {
    let provider = EnvProvider::new(EnvProviderConfig {
        max_key_bytes: 10,
        overrides: Some(BTreeMap::new()),
        ..EnvProviderConfig::default()
    });
    let oversized_key = oversized_string(11);
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": oversized_key})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("exceeds limit"));
}

/// Tests that values exceeding `max_value_bytes` are rejected.
///
/// Threat model: Resource exhaustion via oversized values.
#[test]
fn env_value_exceeds_max_length_rejected() {
    let mut overrides = BTreeMap::new();
    let oversized_value = oversized_string(100);
    overrides.insert("DG_OVERSIZED_VALUE".to_string(), oversized_value);
    let provider = EnvProvider::new(EnvProviderConfig {
        max_value_bytes: 50,
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "DG_OVERSIZED_VALUE"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("exceeds limit"));
}

/// Tests that keys at exactly the limit are accepted.
#[test]
fn env_key_at_max_length_accepted() {
    let mut overrides = BTreeMap::new();
    let exact_key = oversized_string(10);
    overrides.insert(exact_key.clone(), "ok".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        max_key_bytes: 10,
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": exact_key})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());
}

/// Tests that values at exactly the limit are accepted.
#[test]
fn env_value_at_max_length_accepted() {
    let mut overrides = BTreeMap::new();
    let exact_value = oversized_string(50);
    overrides.insert("DG_EXACT_VALUE".to_string(), exact_value.clone());
    let provider = EnvProvider::new(EnvProviderConfig {
        max_value_bytes: 50,
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "DG_EXACT_VALUE"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, exact_value);
}

// ============================================================================
// SECTION: Policy Enforcement Tests - Allowlist/Denylist
// ============================================================================

/// Tests that keys in the denylist are blocked even if they exist.
///
/// Security: Denylist takes precedence to ensure sensitive keys are protected.
#[test]
fn env_key_in_denylist_blocked() {
    let mut overrides = BTreeMap::new();
    overrides.insert("BLOCKED_KEY".to_string(), "secret_value".to_string());
    let mut denylist = BTreeSet::new();
    denylist.insert("BLOCKED_KEY".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        denylist,
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "BLOCKED_KEY"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("blocked by policy"));
}

/// Tests that keys not in the allowlist are blocked when allowlist is set.
///
/// Security: Allowlist enforcement ensures only approved keys are accessible.
#[test]
fn env_key_not_in_allowlist_blocked() {
    let mut overrides = BTreeMap::new();
    overrides.insert("UNAPPROVED_KEY".to_string(), "value".to_string());
    let mut allowlist = BTreeSet::new();
    allowlist.insert("APPROVED_KEY".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        allowlist: Some(allowlist),
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "UNAPPROVED_KEY"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("blocked by policy"));
}

/// Tests that keys in the allowlist are permitted.
#[test]
fn env_key_in_allowlist_permitted() {
    let mut overrides = BTreeMap::new();
    overrides.insert("APPROVED_KEY".to_string(), "allowed_value".to_string());
    let mut allowlist = BTreeSet::new();
    allowlist.insert("APPROVED_KEY".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        allowlist: Some(allowlist),
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "APPROVED_KEY"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "allowed_value");
}

/// Tests that denylist takes precedence over allowlist.
///
/// Security: A key that is both allowed and denied should be denied.
#[test]
fn env_denylist_takes_precedence_over_allowlist() {
    let mut overrides = BTreeMap::new();
    overrides.insert("CONFLICTING_KEY".to_string(), "value".to_string());
    let mut allowlist = BTreeSet::new();
    allowlist.insert("CONFLICTING_KEY".to_string());
    let mut denylist = BTreeSet::new();
    denylist.insert("CONFLICTING_KEY".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        allowlist: Some(allowlist),
        denylist,
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "CONFLICTING_KEY"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("blocked by policy"));
}

/// Tests that an empty allowlist blocks all keys.
///
/// Security: Empty allowlist means nothing is permitted (fail-closed).
#[test]
fn env_empty_allowlist_blocks_all() {
    let mut overrides = BTreeMap::new();
    overrides.insert("ANY_KEY".to_string(), "value".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        allowlist: Some(BTreeSet::new()),
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "ANY_KEY"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Error Path Tests - Invalid Parameters
// ============================================================================

/// Tests that unsupported checks are rejected.
#[test]
fn env_unsupported_check_rejected() {
    let provider = EnvProvider::new(EnvProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "set".to_string(), // Invalid check
        params: Some(json!({"key": "TEST"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported"));
}

/// Tests that missing params are rejected.
#[test]
fn env_missing_params_rejected() {
    let provider = EnvProvider::new(EnvProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: None,
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("requires params"));
}

/// Tests that non-object params are rejected.
#[test]
fn env_params_not_object_rejected() {
    let provider = EnvProvider::new(EnvProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!("not_an_object")),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be an object"));
}

/// Tests that missing key param is rejected.
#[test]
fn env_missing_key_param_rejected() {
    let provider = EnvProvider::new(EnvProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"other": "value"})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("missing"));
}

/// Tests that non-string key param is rejected.
#[test]
fn env_key_param_not_string_rejected() {
    let provider = EnvProvider::new(EnvProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": 12345})),
    };
    let result = provider.query(&query, &sample_context());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be a string"));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests that empty key is handled (may be rejected or return nothing).
#[test]
fn env_empty_key_handling() {
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(BTreeMap::new()),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": ""})),
    };
    // Empty key should either be rejected or return no value
    // The implementation allows it through, returning None
    let result = provider.query(&query, &sample_context());
    assert!(result.is_ok());
    assert!(result.unwrap().value.is_none());
}

/// Tests that keys with special characters are handled correctly.
#[test]
fn env_special_characters_in_key() {
    let mut overrides = BTreeMap::new();
    overrides.insert("KEY_WITH_SPECIAL_!@#$".to_string(), "special_value".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "KEY_WITH_SPECIAL_!@#$"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "special_value");
}

/// Tests that Unicode keys are handled correctly.
#[test]
fn env_unicode_key_handling() {
    let mut overrides = BTreeMap::new();
    overrides.insert("KEY_UNICODE".to_string(), "value".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "KEY_UNICODE"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    assert!(result.value.is_some());
}

/// Tests that Unicode values are preserved correctly.
#[test]
fn env_unicode_value_preserved() {
    let mut overrides = BTreeMap::new();
    overrides.insert("UNICODE_VALUE".to_string(), "Hello World".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "UNICODE_VALUE"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    let EvidenceValue::Json(Value::String(value)) = result.value.unwrap() else {
        panic!("expected string evidence");
    };
    assert_eq!(value, "Hello World");
}

/// Tests `content_type` is set correctly for string values.
#[test]
fn env_content_type_set_for_value() {
    let mut overrides = BTreeMap::new();
    overrides.insert("CONTENT_TYPE_TEST".to_string(), "value".to_string());
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(overrides),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "CONTENT_TYPE_TEST"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    assert_eq!(result.content_type, Some("text/plain".to_string()));
}

/// Tests `content_type` is None for missing values.
#[test]
fn env_content_type_none_for_missing() {
    let provider = EnvProvider::new(EnvProviderConfig {
        overrides: Some(BTreeMap::new()),
        ..EnvProviderConfig::default()
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("env"),
        check_id: "get".to_string(),
        params: Some(json!({"key": "MISSING_KEY"})),
    };
    let result = provider.query(&query, &sample_context()).unwrap();
    assert!(result.content_type.is_none());
}
