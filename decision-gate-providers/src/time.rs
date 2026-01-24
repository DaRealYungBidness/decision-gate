// decision-gate-providers/src/time.rs
// ============================================================================
// Module: Time Evidence Provider
// Description: Evidence provider for trigger-time comparisons.
// Purpose: Expose deterministic time-based predicates without wall-clock access.
// Dependencies: decision-gate-core, serde_json, time
// ============================================================================

//! ## Overview
//! The time provider derives evidence from the trigger timestamp supplied in
//! the evidence context. It never reads wall-clock time directly to preserve
//! deterministic replay behavior.
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::TrustLane;
use serde::Deserialize;
use serde_json::Number;
use serde_json::Value;
use time::OffsetDateTime;

// ============================================================================
// SECTION: Configuration
// ============================================================================

/// Configuration for the time provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct TimeProviderConfig {
    /// Allow logical trigger timestamps for comparisons.
    pub allow_logical: bool,
}

impl Default for TimeProviderConfig {
    fn default() -> Self {
        Self {
            allow_logical: true,
        }
    }
}

// ============================================================================
// SECTION: Provider Implementation
// ============================================================================

/// Evidence provider for trigger-time predicates.
pub struct TimeProvider {
    /// Provider configuration, including logical timestamp policy.
    config: TimeProviderConfig,
}

impl TimeProvider {
    /// Creates a new time provider with the given configuration.
    #[must_use]
    pub const fn new(config: TimeProviderConfig) -> Self {
        Self {
            config,
        }
    }
}

impl EvidenceProvider for TimeProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let anchor = anchor_for_time(ctx.trigger_time);
        match query.predicate.as_str() {
            "now" => {
                let value = timestamp_value(ctx.trigger_time, self.config)?;
                Ok(EvidenceResult {
                    value: Some(EvidenceValue::Json(value)),
                    lane: TrustLane::Verified,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: Some(anchor),
                    signature: None,
                    content_type: Some("application/json".to_string()),
                })
            }
            "after" | "before" => {
                let threshold =
                    parse_threshold(query.params.as_ref(), ctx.trigger_time, self.config)?;
                let result = compare_time(ctx.trigger_time, threshold, query.predicate.as_str());
                Ok(EvidenceResult {
                    value: Some(EvidenceValue::Json(Value::Bool(result))),
                    lane: TrustLane::Verified,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: Some(anchor),
                    signature: None,
                    content_type: Some("application/json".to_string()),
                })
            }
            _ => Err(EvidenceError::Provider("unsupported time predicate".to_string())),
        }
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Builds an evidence anchor for the trigger timestamp.
fn anchor_for_time(timestamp: Timestamp) -> EvidenceAnchor {
    match timestamp {
        Timestamp::UnixMillis(value) => EvidenceAnchor {
            anchor_type: "trigger_time_unix_millis".to_string(),
            anchor_value: value.to_string(),
        },
        Timestamp::Logical(value) => EvidenceAnchor {
            anchor_type: "trigger_time_logical".to_string(),
            anchor_value: value.to_string(),
        },
    }
}

/// Converts a trigger timestamp into a JSON value, enforcing policy.
fn timestamp_value(
    timestamp: Timestamp,
    config: TimeProviderConfig,
) -> Result<Value, EvidenceError> {
    match timestamp {
        Timestamp::UnixMillis(value) => Ok(Value::Number(Number::from(value))),
        Timestamp::Logical(value) => {
            if !config.allow_logical {
                return Err(EvidenceError::Provider(
                    "logical timestamps are not permitted".to_string(),
                ));
            }
            Ok(Value::Number(Number::from(value)))
        }
    }
}

/// Parses the predicate threshold from query parameters.
fn parse_threshold(
    params: Option<&Value>,
    timestamp: Timestamp,
    config: TimeProviderConfig,
) -> Result<Timestamp, EvidenceError> {
    let params = params
        .ok_or_else(|| EvidenceError::Provider("time predicate requires params".to_string()))?;
    let Value::Object(map) = params else {
        return Err(EvidenceError::Provider("time params must be an object".to_string()));
    };
    let value = map
        .get("timestamp")
        .ok_or_else(|| EvidenceError::Provider("missing timestamp param".to_string()))?;
    match timestamp {
        Timestamp::UnixMillis(_) => parse_unix_millis(value),
        Timestamp::Logical(_) => {
            if !config.allow_logical {
                return Err(EvidenceError::Provider(
                    "logical timestamps are not permitted".to_string(),
                ));
            }
            parse_logical(value)
        }
    }
}

/// Parses a timestamp value expressed in Unix milliseconds or RFC3339.
fn parse_unix_millis(value: &Value) -> Result<Timestamp, EvidenceError> {
    match value {
        Value::Number(number) => {
            if let Some(raw) = number.as_i64() {
                return Ok(Timestamp::UnixMillis(raw));
            }
            if let Some(raw) = number.as_u64() {
                let Ok(raw) = i64::try_from(raw) else {
                    return Err(EvidenceError::Provider("timestamp exceeds i64 range".to_string()));
                };
                return Ok(Timestamp::UnixMillis(raw));
            }
            Err(EvidenceError::Provider("invalid numeric timestamp".to_string()))
        }
        Value::String(text) => parse_rfc3339(text),
        _ => Err(EvidenceError::Provider("timestamp must be number or string".to_string())),
    }
}

/// Parses an RFC3339 timestamp string into Unix milliseconds.
fn parse_rfc3339(value: &str) -> Result<Timestamp, EvidenceError> {
    let parsed = OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map_err(|_| EvidenceError::Provider("invalid rfc3339 timestamp".to_string()))?;
    let millis = parsed.unix_timestamp_nanos() / 1_000_000;
    let Ok(millis) = i64::try_from(millis) else {
        return Err(EvidenceError::Provider("timestamp exceeds i64 range".to_string()));
    };
    Ok(Timestamp::UnixMillis(millis))
}

/// Parses a logical timestamp value.
fn parse_logical(value: &Value) -> Result<Timestamp, EvidenceError> {
    let Value::Number(number) = value else {
        return Err(EvidenceError::Provider("logical timestamp must be numeric".to_string()));
    };
    let Some(raw) = number.as_u64() else {
        return Err(EvidenceError::Provider("logical timestamp must be unsigned".to_string()));
    };
    Ok(Timestamp::Logical(raw))
}

/// Compares the trigger timestamp against the threshold for a predicate.
fn compare_time(now: Timestamp, threshold: Timestamp, predicate: &str) -> bool {
    match (now, threshold) {
        (Timestamp::UnixMillis(now), Timestamp::UnixMillis(threshold)) => match predicate {
            "after" => now > threshold,
            "before" => now < threshold,
            _ => false,
        },
        (Timestamp::Logical(now), Timestamp::Logical(threshold)) => match predicate {
            "after" => now > threshold,
            "before" => now < threshold,
            _ => false,
        },
        _ => false,
    }
}
