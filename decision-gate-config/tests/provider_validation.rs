//! Provider config validation tests for decision-gate-config.
// decision-gate-config/tests/provider_validation.rs
// =============================================================================
// Module: Provider Config Validation Tests
// Description: Validate provider configuration and timeout/auth constraints.
// Purpose: Ensure provider definitions remain safe and deterministic.
// =============================================================================

use std::path::PathBuf;

use decision_gate_config::ConfigError;
use decision_gate_config::ProviderAuthConfig;
use decision_gate_config::ProviderConfig;
use decision_gate_config::ProviderDiscoveryConfig;
use decision_gate_config::ProviderTimeoutConfig;
use decision_gate_config::ProviderType;
use serde::Deserialize;

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
fn provider_requires_name() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "   ".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    assert_invalid(config.validate(), "provider name is empty")?;
    Ok(())
}

#[test]
fn provider_mcp_requires_capabilities_and_transport() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    assert_invalid(config.validate(), "mcp provider requires command or url")?;
    Ok(())
}

#[test]
fn provider_mcp_rejects_insecure_http_without_allow() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("http://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    assert_invalid(config.validate(), "insecure http requires allow_insecure_http")?;
    Ok(())
}

#[test]
fn provider_builtin_rejects_capabilities_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "time".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    assert_invalid(config.validate(), "builtin provider does not accept capabilities_path")?;
    Ok(())
}

#[test]
fn provider_auth_rejects_empty_token() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["./provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: Some(ProviderAuthConfig {
            bearer_token: Some("   ".to_string()),
        }),
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    assert_invalid(config.validate(), "providers.auth.bearer_token must be non-empty")?;
    Ok(())
}

#[test]
fn provider_timeouts_reject_request_below_connect() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["./provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: 2_000,
            request_timeout_ms: 1_000,
        },
        config: None,
    }];
    assert_invalid(
        config.validate(),
        "providers.timeouts.request_timeout_ms must be >= connect_timeout_ms",
    )?;
    Ok(())
}

#[test]
fn provider_parse_config_rejects_invalid_payload() {
    #[allow(dead_code, reason = "Used for parse_config type coverage in tests.")]
    #[derive(Debug, Deserialize, Default)]
    struct ProviderSettings {
        enabled: bool,
    }

    let provider = ProviderConfig {
        name: "builtin".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: Some(toml::Value::Integer(5)),
    };

    let result = provider.parse_config::<ProviderSettings>();
    assert!(result.is_err(), "expected parse_config to reject invalid payload");
}

#[test]
fn provider_discovery_rejects_empty_entries() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.provider_discovery = ProviderDiscoveryConfig {
        allowlist: vec![String::new()],
        denylist: Vec::new(),
        max_response_bytes: 1024,
    };
    assert_invalid(config.validate(), "provider_discovery allow/deny entries must be non-empty")?;
    Ok(())
}

#[test]
fn provider_discovery_requires_positive_max_response_bytes() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.provider_discovery = ProviderDiscoveryConfig {
        allowlist: Vec::new(),
        denylist: Vec::new(),
        max_response_bytes: 0,
    };
    assert_invalid(config.validate(), "provider_discovery.max_response_bytes must be > 0")?;
    Ok(())
}
