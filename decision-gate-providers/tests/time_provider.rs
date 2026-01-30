// decision-gate-providers/tests/time_provider.rs
// ============================================================================
// Module: Time Provider Tests
// Description: Comprehensive tests for trigger-time evidence provider.
// Purpose: Validate deterministic time checks and fail-closed behavior.
// Dependencies: decision-gate-providers, decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! Tests the time provider for:
//! - Happy path: now, after, before checks
//! - Boundary enforcement: logical timestamp policy
//! - Error handling: invalid parameters, unsupported checks
//! - Edge cases: timestamp overflow, RFC3339 parsing, mixed types
//!
//! Security posture: Time checks must derive from trigger context only,
//! never wall-clock time, to preserve deterministic replay.
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

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderId;
use decision_gate_core::Timestamp;
use decision_gate_providers::TimeProvider;
use decision_gate_providers::TimeProviderConfig;
use serde_json::Value;
use serde_json::json;

use crate::common::sample_context_unix_millis;
use crate::common::sample_context_with_time;

// ============================================================================
// SECTION: Happy Path Tests - Now Check
// ============================================================================

/// Tests that the time provider returns trigger time for "now" with Unix millis.
#[test]
fn time_provider_returns_trigger_time_for_now() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_unix_millis(1234);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_i64(), Some(1234));
}

/// Tests that "now" returns logical timestamp when logical is allowed.
#[test]
fn time_provider_now_returns_logical_timestamp() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: true,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_with_time(Timestamp::Logical(42));
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_u64(), Some(42));
}

/// Tests that evidence anchor is set correctly for trigger time.
#[test]
fn time_provider_sets_evidence_anchor_unix() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_unix_millis(9999);
    let result = provider.query(&query, &context).unwrap();
    let anchor = result.evidence_anchor.unwrap();
    assert_eq!(anchor.anchor_type, "trigger_time_unix_millis");
    assert_eq!(anchor.anchor_value, "9999");
}

/// Tests that evidence anchor is set correctly for logical timestamps.
#[test]
fn time_provider_sets_evidence_anchor_logical() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: true,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_with_time(Timestamp::Logical(100));
    let result = provider.query(&query, &context).unwrap();
    let anchor = result.evidence_anchor.unwrap();
    assert_eq!(anchor.anchor_type, "trigger_time_logical");
    assert_eq!(anchor.anchor_value, "100");
}

// ============================================================================
// SECTION: Happy Path Tests - After Check
// ============================================================================

/// Tests that "after" returns true when trigger time is after threshold.
#[test]
fn time_provider_after_comparison_true() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": 1000})),
    };
    let context = sample_context_unix_millis(1500);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value);
}

/// Tests that "after" returns false when trigger time is before threshold.
#[test]
fn time_provider_after_comparison_false() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": 2000})),
    };
    let context = sample_context_unix_millis(1500);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(!value);
}

/// Tests that "after" returns false when times are equal.
#[test]
fn time_provider_after_comparison_equal() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": 1000})),
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(!value); // Equal is not "after"
}

// ============================================================================
// SECTION: Happy Path Tests - Before Check
// ============================================================================

/// Tests that "before" returns true when trigger time is before threshold.
#[test]
fn time_provider_before_comparison_true() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "before".to_string(),
        params: Some(json!({"timestamp": 2000})),
    };
    let context = sample_context_unix_millis(1500);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value);
}

/// Tests that "before" returns false when trigger time is after threshold.
#[test]
fn time_provider_before_comparison_false() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "before".to_string(),
        params: Some(json!({"timestamp": 1000})),
    };
    let context = sample_context_unix_millis(1500);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(!value);
}

/// Tests that "before" returns false when times are equal.
#[test]
fn time_provider_before_comparison_equal() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "before".to_string(),
        params: Some(json!({"timestamp": 1000})),
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(!value); // Equal is not "before"
}

// ============================================================================
// SECTION: Logical Timestamp Tests
// ============================================================================

/// Tests that "after" works with logical timestamps.
#[test]
fn time_provider_after_logical_timestamps() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: true,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": 10})),
    };
    let context = sample_context_with_time(Timestamp::Logical(15));
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value);
}

/// Tests that "before" works with logical timestamps.
#[test]
fn time_provider_before_logical_timestamps() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: true,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "before".to_string(),
        params: Some(json!({"timestamp": 20})),
    };
    let context = sample_context_with_time(Timestamp::Logical(15));
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value);
}

// ============================================================================
// SECTION: Boundary Enforcement - Logical Disabled
// ============================================================================

/// Tests that "now" rejects logical timestamps when disabled.
///
/// Security: Disabling logical timestamps enforces wall-clock semantics.
#[test]
fn time_logical_disabled_rejects_now_with_logical() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: false,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_with_time(Timestamp::Logical(42));
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("logical timestamps are not permitted"));
}

/// Tests that "after" rejects logical timestamps when disabled.
#[test]
fn time_logical_disabled_rejects_after_with_logical() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: false,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": 10})),
    };
    let context = sample_context_with_time(Timestamp::Logical(15));
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("logical timestamps are not permitted"));
}

// ============================================================================
// SECTION: RFC3339 Timestamp Parsing
// ============================================================================

/// Tests that RFC3339 timestamps are parsed correctly for "after".
#[test]
fn time_rfc3339_parsing_after() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    // 2024-01-01T00:00:00Z = 1704067200000 ms
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": "2024-01-01T00:00:00Z"})),
    };
    // Use a time after 2024-01-01
    let context = sample_context_unix_millis(1_704_067_200_001);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value);
}

/// Tests that RFC3339 timestamps are parsed correctly for "before".
#[test]
fn time_rfc3339_parsing_before() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "before".to_string(),
        params: Some(json!({"timestamp": "2024-01-01T00:00:00Z"})),
    };
    // Use a time before 2024-01-01
    let context = sample_context_unix_millis(1_704_067_199_999);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value);
}

/// Tests that invalid RFC3339 timestamps are rejected.
#[test]
fn time_rfc3339_invalid_format_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": "not-a-timestamp"})),
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("invalid rfc3339"));
}

// ============================================================================
// SECTION: Error Path Tests - Invalid Parameters
// ============================================================================

/// Tests that unsupported checks are rejected.
#[test]
fn time_unsupported_check_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "unknown".to_string(),
        params: None,
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("unsupported"));
}

/// Tests that missing params for after/before are rejected.
#[test]
fn time_after_missing_params_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: None,
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("requires params"));
}

/// Tests that non-object params are rejected.
#[test]
fn time_params_not_object_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!("not_an_object")),
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be an object"));
}

/// Tests that missing timestamp param is rejected.
#[test]
fn time_missing_timestamp_param_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"other": "value"})),
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("missing timestamp"));
}

/// Tests that invalid timestamp types are rejected.
#[test]
fn time_timestamp_invalid_type_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": true})),
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be number or string"));
}

/// Tests that negative logical timestamps are rejected.
#[test]
fn time_logical_timestamp_negative_rejected() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: true,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": -5})),
    };
    let context = sample_context_with_time(Timestamp::Logical(10));
    let result = provider.query(&query, &context);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:?}").contains("must be unsigned"));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests handling of negative Unix timestamps (pre-epoch).
#[test]
fn time_negative_unix_timestamp_handling() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "after".to_string(),
        params: Some(json!({"timestamp": -1000})),
    };
    let context = sample_context_unix_millis(0);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Bool(value)) = result.value.unwrap() else {
        panic!("expected boolean evidence");
    };
    assert!(value); // 0 is after -1000
}

/// Tests `content_type` is set correctly.
#[test]
fn time_content_type_set() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_unix_millis(1000);
    let result = provider.query(&query, &context).unwrap();
    assert_eq!(result.content_type, Some("application/json".to_string()));
}

/// Tests that very large Unix timestamps don't overflow.
#[test]
fn time_large_unix_timestamp_handling() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    // Year 3000 approximately
    let context = sample_context_unix_millis(32_503_680_000_000);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_i64(), Some(32_503_680_000_000));
}

/// Tests that zero timestamp is handled correctly.
#[test]
fn time_zero_timestamp_handling() {
    let provider = TimeProvider::new(TimeProviderConfig::default());
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_unix_millis(0);
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_i64(), Some(0));
}

/// Tests that zero logical timestamp is handled correctly.
#[test]
fn time_zero_logical_timestamp_handling() {
    let provider = TimeProvider::new(TimeProviderConfig {
        allow_logical: true,
    });
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: None,
    };
    let context = sample_context_with_time(Timestamp::Logical(0));
    let result = provider.query(&query, &context).unwrap();
    let EvidenceValue::Json(Value::Number(number)) = result.value.unwrap() else {
        panic!("expected numeric evidence");
    };
    assert_eq!(number.as_u64(), Some(0));
}
