//! Config defaults and core validation tests for decision-gate-config.
// decision-gate-config/tests/config_defaults.rs
// =============================================================================
// Module: Config Defaults and Core Validation Tests
// Description: Validate default behavior and core config invariants.
// Purpose: Ensure minimal config is valid and critical invariants are enforced.
// =============================================================================

use decision_gate_config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_config::ConfigError;
use decision_gate_config::NamespaceAuthorityMode;
use decision_gate_config::PolicyConfig;
use decision_gate_config::ServerMode;
use decision_gate_config::policy::PolicyEngine;
use decision_gate_config::policy::StaticPolicyConfig;
use decision_gate_core::TrustLane;

mod common;

type TestResult = Result<(), String>;

fn assert_invalid(result: Result<(), ConfigError>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            let message = error.to_string();
            if message.contains(needle) {
                Ok(())
            } else {
                Err(format!("error {message} did not contain {needle}"))
            }
        }
        Ok(()) => Err("expected invalid config".to_string()),
    }
}

#[test]
fn default_config_validates() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn registry_acl_allow_local_only_defaults_to_false() -> TestResult {
    let config = common::minimal_config().map_err(|err| err.to_string())?;
    if config.schema_registry.acl.allow_local_only {
        return Err("schema_registry.acl.allow_local_only should default to false".to_string());
    }
    Ok(())
}

#[test]
fn validation_strict_requires_allow_permissive() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.validation.strict = false;
    config.validation.allow_permissive = false;
    assert_invalid(
        config.validate(),
        "validation.strict=false requires validation.allow_permissive=true",
    )?;
    Ok(())
}

#[test]
fn policy_static_requires_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.policy = PolicyConfig {
        engine: PolicyEngine::Static,
        static_policy: None,
    };
    assert_invalid(config.validate(), "policy.engine=static")?;
    Ok(())
}

#[test]
fn policy_non_static_rejects_static_block() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.policy = PolicyConfig {
        engine: PolicyEngine::PermitAll,
        static_policy: Some(StaticPolicyConfig::default()),
    };
    assert_invalid(config.validate(), "policy.static only allowed")?;
    Ok(())
}

#[test]
fn dev_permissive_rejected_with_assetcore_authority() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.dev.permissive = true;
    config.namespace.authority.mode = NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore = Some(AssetCoreNamespaceAuthorityConfig {
        base_url: "https://assetcore.example.com".to_string(),
        auth_token: None,
        connect_timeout_ms: 500,
        request_timeout_ms: 1_000,
    });
    assert_invalid(
        config.validate(),
        "dev.permissive not allowed when namespace.authority.mode=assetcore_http",
    )?;
    Ok(())
}

#[test]
fn server_mode_dev_permissive_sets_dev_flag() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.mode = ServerMode::DevPermissive;
    config.dev.permissive = false;
    config.validate().map_err(|err| err.to_string())?;
    if !config.dev.permissive {
        return Err("dev.permissive should be set by legacy mode".to_string());
    }
    Ok(())
}

#[test]
fn effective_trust_requirement_relaxes_in_dev_permissive() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.dev.permissive = true;
    let requirement = config.effective_trust_requirement();
    if requirement.min_lane != TrustLane::Asserted {
        return Err("expected TrustLane::Asserted under dev_permissive".to_string());
    }
    Ok(())
}
