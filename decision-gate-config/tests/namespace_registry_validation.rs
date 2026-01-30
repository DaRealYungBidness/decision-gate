//! Namespace authority and registry validation tests for decision-gate-config.
// decision-gate-config/tests/namespace_registry_validation.rs
// =============================================================================
// Module: Namespace and Registry Validation Tests
// Description: Validate namespace authority, registry limits, and ACL rules.
// Purpose: Ensure namespace and schema registry settings fail closed.
// =============================================================================

use std::path::PathBuf;

use decision_gate_config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_config::ConfigError;
use decision_gate_config::NamespaceAuthorityMode;
use decision_gate_config::RegistryAclEffect;
use decision_gate_config::RegistryAclRule;
use decision_gate_config::SchemaRegistryType;

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
fn namespace_allow_default_requires_default_tenants() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.allow_default = true;
    config.namespace.default_tenants.clear();
    assert_invalid(
        config.validate(),
        "namespace.allow_default requires namespace.default_tenants",
    )?;
    Ok(())
}

#[test]
fn namespace_authority_assetcore_requires_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority.mode = NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore = None;
    assert_invalid(config.validate(), "namespace.authority.mode=assetcore_http requires")?;
    Ok(())
}

#[test]
fn namespace_authority_none_rejects_assetcore_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority.mode = NamespaceAuthorityMode::None;
    config.namespace.authority.assetcore = Some(AssetCoreNamespaceAuthorityConfig {
        base_url: "https://assetcore.example.com".to_string(),
        auth_token: None,
        connect_timeout_ms: 500,
        request_timeout_ms: 1_000,
    });
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore only allowed when mode=assetcore_http",
    )?;
    Ok(())
}

#[test]
fn assetcore_authority_rejects_empty_auth_token() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority.mode = NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore = Some(AssetCoreNamespaceAuthorityConfig {
        base_url: "https://assetcore.example.com".to_string(),
        auth_token: Some("  ".to_string()),
        connect_timeout_ms: 500,
        request_timeout_ms: 1_000,
    });
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore.auth_token must be non-empty",
    )?;
    Ok(())
}

#[test]
fn assetcore_authority_rejects_out_of_range_timeouts() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority.mode = NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore = Some(AssetCoreNamespaceAuthorityConfig {
        base_url: "https://assetcore.example.com".to_string(),
        auth_token: None,
        connect_timeout_ms: 50,
        request_timeout_ms: 1_000,
    });
    assert_invalid(config.validate(), "connect_timeout_ms must be between")?;
    Ok(())
}

#[test]
fn schema_registry_memory_rejects_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry.registry_type = SchemaRegistryType::Memory;
    config.schema_registry.path = Some(PathBuf::from("schema.db"));
    assert_invalid(config.validate(), "memory schema_registry must not set path")?;
    Ok(())
}

#[test]
fn schema_registry_sqlite_requires_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry.registry_type = SchemaRegistryType::Sqlite;
    config.schema_registry.path = None;
    assert_invalid(config.validate(), "sqlite schema_registry requires path")?;
    Ok(())
}

#[test]
fn schema_registry_rejects_zero_max_schema_bytes() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry.max_schema_bytes = 0;
    assert_invalid(config.validate(), "schema_registry max_schema_bytes out of range")?;
    Ok(())
}

#[test]
fn registry_acl_rules_reject_empty_subjects() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry.acl.rules = vec![RegistryAclRule {
        effect: RegistryAclEffect::Allow,
        actions: Vec::new(),
        tenants: Vec::new(),
        namespaces: Vec::new(),
        subjects: vec![String::new()],
        roles: Vec::new(),
        policy_classes: Vec::new(),
    }];
    assert_invalid(config.validate(), "schema_registry.acl.rules.subjects must be non-empty")?;
    Ok(())
}
