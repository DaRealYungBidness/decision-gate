// decision-gate-config/tests/common/mod.rs
// =============================================================================
// Module: Config Test Helpers
// Description: Shared helpers for config validation tests.
// Purpose: Reduce duplication across integration tests for decision-gate-config.
// =============================================================================

#![allow(dead_code, reason = "Test helpers are selectively used across suites.")]

use decision_gate_config::DecisionGateConfig;
use decision_gate_config::RateLimitConfig;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerTransport;

/// Parses a TOML string into a `DecisionGateConfig` for tests.
pub fn config_from_toml(toml_str: &str) -> Result<DecisionGateConfig, toml::de::Error> {
    toml::from_str(toml_str)
}

/// Returns a minimal config with all defaults applied.
pub fn minimal_config() -> Result<DecisionGateConfig, toml::de::Error> {
    config_from_toml("")
}

/// Returns a minimal config with HTTP transport and the provided auth config.
pub fn config_with_auth(auth: ServerAuthConfig) -> Result<DecisionGateConfig, toml::de::Error> {
    let mut config = minimal_config()?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.auth = Some(auth);
    Ok(config)
}

/// Returns a minimal config with an HTTP transport and the provided rate limit.
pub fn config_with_rate_limit(
    rate_limit: RateLimitConfig,
) -> Result<DecisionGateConfig, toml::de::Error> {
    let mut config = minimal_config()?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.limits.rate_limit = Some(rate_limit);
    Ok(config)
}
