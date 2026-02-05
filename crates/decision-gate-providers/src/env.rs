// crates/decision-gate-providers/src/env.rs
// ============================================================================
// Module: Environment Evidence Provider
// Description: Evidence provider for environment variable lookups.
// Purpose: Expose deterministic access to process environment state.
// Dependencies: decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! The environment provider resolves values from the process environment. It
//! enforces explicit allowlist and denylist rules plus hard size limits to
//! preserve fail-closed behavior.
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::TrustLane;
use serde::Deserialize;
use serde_json::Value;

// ============================================================================
// SECTION: Configuration
// ============================================================================

/// Configuration for the environment provider.
///
/// # Invariants
/// - `denylist` overrides `allowlist` when both are present.
/// - `max_value_bytes` and `max_key_bytes` are enforced as hard upper bounds.
/// - `overrides` take precedence over process environment reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EnvProviderConfig {
    /// Optional allowlist of environment variable keys.
    pub allowlist: Option<BTreeSet<String>>,
    /// Explicit denylist of environment variable keys.
    pub denylist: BTreeSet<String>,
    /// Maximum bytes allowed for a single environment value.
    pub max_value_bytes: usize,
    /// Maximum bytes allowed for a single environment key.
    pub max_key_bytes: usize,
    /// Optional override map used for deterministic lookups.
    pub overrides: Option<BTreeMap<String, String>>,
}

impl Default for EnvProviderConfig {
    fn default() -> Self {
        Self {
            allowlist: None,
            denylist: BTreeSet::new(),
            max_value_bytes: 64 * 1024,
            max_key_bytes: 255,
            overrides: None,
        }
    }
}

// ============================================================================
// SECTION: Provider Implementation
// ============================================================================

/// Evidence provider for environment variables.
///
/// # Invariants
/// - Supports only the `get` check id.
/// - Applies allowlist/denylist policy before any lookup.
/// - Enforces key/value size limits and fails closed on violations.
pub struct EnvProvider {
    /// Provider configuration, including policy and size limits.
    config: EnvProviderConfig,
}

impl EnvProvider {
    /// Creates a new environment provider with the given configuration.
    #[must_use]
    pub const fn new(config: EnvProviderConfig) -> Self {
        Self {
            config,
        }
    }
}

impl EvidenceProvider for EnvProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        if query.check_id.as_str() != "get" {
            return Err(EvidenceError::Provider("unsupported env check".to_string()));
        }

        let key = extract_key(query.params.as_ref())?;
        if key.len() > self.config.max_key_bytes {
            return Err(EvidenceError::Provider("env key exceeds limit".to_string()));
        }
        if !is_key_allowed(&self.config, key) {
            return Err(EvidenceError::Provider("env key blocked by policy".to_string()));
        }

        if let Some(overrides) = &self.config.overrides {
            return overrides.get(key).map_or_else(
                || Ok(empty_result(key)),
                |value| build_value_result(key, value.clone(), self.config.max_value_bytes),
            );
        }

        std::env::var(key).map_or_else(
            |_| Ok(empty_result(key)),
            |value| build_value_result(key, value, self.config.max_value_bytes),
        )
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Extracts the env key from the query parameters.
fn extract_key(params: Option<&Value>) -> Result<&str, EvidenceError> {
    let params =
        params.ok_or_else(|| EvidenceError::Provider("env check requires params".to_string()))?;
    let Value::Object(map) = params else {
        return Err(EvidenceError::Provider("env params must be an object".to_string()));
    };
    let Value::String(key) =
        map.get("key").ok_or_else(|| EvidenceError::Provider("missing env key".to_string()))?
    else {
        return Err(EvidenceError::Provider("env key must be a string".to_string()));
    };
    Ok(key)
}

/// Validates the key against allowlist/denylist policy.
fn is_key_allowed(config: &EnvProviderConfig, key: &str) -> bool {
    if config.denylist.contains(key) {
        return false;
    }
    if let Some(allowlist) = &config.allowlist {
        return allowlist.contains(key);
    }
    true
}

/// Builds a populated evidence result, enforcing value size limits.
fn build_value_result(
    key: &str,
    value: String,
    max_value_bytes: usize,
) -> Result<EvidenceResult, EvidenceError> {
    if value.len() > max_value_bytes {
        return Err(EvidenceError::Provider("env value exceeds limit".to_string()));
    }
    Ok(EvidenceResult {
        value: Some(EvidenceValue::Json(Value::String(value))),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: Some(EvidenceAnchor {
            anchor_type: "env".to_string(),
            anchor_value: key.to_string(),
        }),
        signature: None,
        content_type: Some("text/plain".to_string()),
    })
}

/// Builds an empty evidence result for a missing key.
fn empty_result(key: &str) -> EvidenceResult {
    EvidenceResult {
        value: None,
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: Some(EvidenceAnchor {
            anchor_type: "env".to_string(),
            anchor_value: key.to_string(),
        }),
        signature: None,
        content_type: None,
    }
}
