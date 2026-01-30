//! Limits validation tests for decision-gate-config.
// decision-gate-config/tests/limits_validation.rs
// =============================================================================
// Module: Limits Validation Tests
// Description: Comprehensive tests for all MAX_*/MIN_* constant enforcement.
// Purpose: Ensure all numeric and size limits are properly enforced.
// =============================================================================

use decision_gate_config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_config::ConfigError;
use decision_gate_config::NamespaceAuthorityConfig;
use decision_gate_config::NamespaceAuthorityMode;
use decision_gate_config::PrincipalConfig;
use decision_gate_config::PrincipalRoleConfig;
use decision_gate_config::ProviderConfig;
use decision_gate_config::ProviderTimeoutConfig;
use decision_gate_config::ProviderType;
use decision_gate_config::RateLimitConfig;
use decision_gate_config::RegistryAclConfig;
use decision_gate_config::RegistryAclDefault;
use decision_gate_config::RegistryAclEffect;
use decision_gate_config::RegistryAclMode;
use decision_gate_config::RegistryAclRule;
use decision_gate_config::SchemaRegistryConfig;
use decision_gate_config::SchemaRegistryType;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerAuthMode;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;

mod common;

type TestResult = Result<(), String>;

// Test constants (from config.rs)
const MAX_AUTH_TOKENS: usize = 64;
const MAX_AUTH_TOKEN_LENGTH: usize = 256;
const MAX_AUTH_SUBJECT_LENGTH: usize = 512;
const MAX_AUTH_TOOL_RULES: usize = 128;
const MAX_PRINCIPAL_ROLES: usize = 128;
const MAX_REGISTRY_ACL_RULES: usize = 256;
const MIN_RATE_LIMIT_WINDOW_MS: u64 = 100;
const MAX_RATE_LIMIT_WINDOW_MS: u64 = 60_000;
const MAX_RATE_LIMIT_REQUESTS: u32 = 100_000;
const MAX_RATE_LIMIT_ENTRIES: usize = 65_536;
const MIN_PROVIDER_CONNECT_TIMEOUT_MS: u64 = 100;
const MAX_PROVIDER_CONNECT_TIMEOUT_MS: u64 = 10_000;
const MIN_PROVIDER_REQUEST_TIMEOUT_MS: u64 = 500;
const MAX_PROVIDER_REQUEST_TIMEOUT_MS: u64 = 30_000;
const MAX_SCHEMA_MAX_BYTES: usize = 10 * 1024 * 1024;
const MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS: u64 = 100;
const MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS: u64 = 10_000;
const MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS: u64 = 500;
const MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS: u64 = 30_000;

/// Assert that a validation result is an error containing a specific substring.
fn assert_invalid(result: Result<(), ConfigError>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            let message = error.to_string();
            if message.contains(needle) {
                Ok(())
            } else {
                Err(format!("error '{message}' did not contain '{needle}'"))
            }
        }
        Ok(()) => Err("expected invalid config".to_string()),
    }
}

// ============================================================================
// SECTION: Array Size Limits - Auth
// ============================================================================

#[test]
fn bearer_tokens_at_max_auth_tokens_64() -> TestResult {
    let tokens: Vec<String> = (0 .. MAX_AUTH_TOKENS).map(|i| format!("token{i}")).collect();
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: tokens,
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn bearer_tokens_exceeds_max_auth_tokens_65() -> TestResult {
    let tokens: Vec<String> = (0 .. MAX_AUTH_TOKENS + 1).map(|i| format!("token{i}")).collect();
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: tokens,
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "too many auth tokens")?;
    Ok(())
}

#[test]
fn mtls_subjects_at_max_auth_tokens_64() -> TestResult {
    let subjects: Vec<String> = (0 .. MAX_AUTH_TOKENS).map(|i| format!("CN=subject{i}")).collect();
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: subjects,
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn mtls_subjects_exceeds_max_auth_tokens_65() -> TestResult {
    let subjects: Vec<String> =
        (0 .. MAX_AUTH_TOKENS + 1).map(|i| format!("CN=subject{i}")).collect();
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: subjects,
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "too many mTLS subjects")?;
    Ok(())
}

#[test]
fn allowed_tools_at_max_auth_tool_rules_128() -> TestResult {
    let mut tools = Vec::new();
    for i in 0 .. MAX_AUTH_TOOL_RULES {
        if i % 2 == 0 {
            tools.push("precheck".to_string());
        } else {
            tools.push("scenario_next".to_string());
        }
    }
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: tools,
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn allowed_tools_exceeds_max_auth_tool_rules_129() -> TestResult {
    let mut tools = Vec::new();
    for i in 0 .. MAX_AUTH_TOOL_RULES + 1 {
        if i % 2 == 0 {
            tools.push("precheck".to_string());
        } else {
            tools.push("scenario_next".to_string());
        }
    }
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: tools,
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "too many tool allowlist entries")?;
    Ok(())
}

#[test]
fn principals_at_max_auth_tokens_64() -> TestResult {
    let principals: Vec<PrincipalConfig> = (0 .. MAX_AUTH_TOKENS)
        .map(|i| PrincipalConfig {
            subject: format!("user{i}@example.com"),
            policy_class: None,
            roles: Vec::new(),
        })
        .collect();
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals,
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn principals_exceeds_max_auth_tokens_65() -> TestResult {
    let principals: Vec<PrincipalConfig> = (0 .. MAX_AUTH_TOKENS + 1)
        .map(|i| PrincipalConfig {
            subject: format!("user{i}@example.com"),
            policy_class: None,
            roles: Vec::new(),
        })
        .collect();
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals,
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "too many principal mappings")?;
    Ok(())
}

#[test]
fn principal_roles_at_max_principal_roles_128() -> TestResult {
    let roles: Vec<PrincipalRoleConfig> = (0 .. MAX_PRINCIPAL_ROLES)
        .map(|i| PrincipalRoleConfig {
            name: format!("role{i}"),
            tenant_id: None,
            namespace_id: None,
        })
        .collect();
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles,
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn principal_roles_exceeds_max_principal_roles_129() -> TestResult {
    let roles: Vec<PrincipalRoleConfig> = (0 .. MAX_PRINCIPAL_ROLES + 1)
        .map(|i| PrincipalRoleConfig {
            name: format!("role{i}"),
            tenant_id: None,
            namespace_id: None,
        })
        .collect();
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles,
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.roles exceeds max entries")?;
    Ok(())
}

#[test]
fn registry_acl_rules_at_max_registry_acl_rules_256() -> TestResult {
    let rules: Vec<RegistryAclRule> = (0 .. MAX_REGISTRY_ACL_RULES)
        .map(|_| RegistryAclRule {
            effect: RegistryAclEffect::Allow,
            actions: Vec::new(),
            tenants: Vec::new(),
            namespaces: Vec::new(),
            subjects: Vec::new(),
            roles: Vec::new(),
            policy_classes: Vec::new(),
        })
        .collect();
    let acl = RegistryAclConfig {
        mode: RegistryAclMode::Custom,
        default: RegistryAclDefault::Deny,
        require_signing: false,
        rules,
    };
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry.acl = acl;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn registry_acl_rules_exceeds_max_registry_acl_rules_257() -> TestResult {
    let rules: Vec<RegistryAclRule> = (0 .. MAX_REGISTRY_ACL_RULES + 1)
        .map(|_| RegistryAclRule {
            effect: RegistryAclEffect::Allow,
            actions: Vec::new(),
            tenants: Vec::new(),
            namespaces: Vec::new(),
            subjects: Vec::new(),
            roles: Vec::new(),
            policy_classes: Vec::new(),
        })
        .collect();
    let acl = RegistryAclConfig {
        mode: RegistryAclMode::Custom,
        default: RegistryAclDefault::Deny,
        require_signing: false,
        rules,
    };
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry.acl = acl;
    assert_invalid(config.validate(), "schema_registry.acl.rules exceeds max entries")?;
    Ok(())
}

// ============================================================================
// SECTION: String Length Limits
// ============================================================================

#[test]
fn bearer_token_at_max_auth_token_length_256() -> TestResult {
    let token = "a".repeat(MAX_AUTH_TOKEN_LENGTH);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn bearer_token_exceeds_max_auth_token_length_257() -> TestResult {
    let token = "a".repeat(MAX_AUTH_TOKEN_LENGTH + 1);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth token too long")?;
    Ok(())
}

#[test]
fn mtls_subject_at_max_auth_subject_length_512() -> TestResult {
    let subject = "a".repeat(MAX_AUTH_SUBJECT_LENGTH);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec![subject],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn mtls_subject_exceeds_max_auth_subject_length_513() -> TestResult {
    let subject = "a".repeat(MAX_AUTH_SUBJECT_LENGTH + 1);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec![subject],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "mTLS subject too long")?;
    Ok(())
}

// ============================================================================
// SECTION: Rate Limit Bounds
// ============================================================================

#[test]
fn rate_limit_window_ms_at_min_100() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: MIN_RATE_LIMIT_WINDOW_MS,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_window_ms_below_min_99() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: MIN_RATE_LIMIT_WINDOW_MS - 1,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "rate_limit window_ms must be between 100 and 60000")?;
    Ok(())
}

#[test]
fn rate_limit_window_ms_at_max_60000() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: MAX_RATE_LIMIT_WINDOW_MS,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_window_ms_above_max_60001() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: MAX_RATE_LIMIT_WINDOW_MS + 1,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "rate_limit window_ms must be between 100 and 60000")?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_at_min_1() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 1,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_at_zero_rejected() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 0,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "rate_limit max_requests must be greater than zero")?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_at_max_100000() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: MAX_RATE_LIMIT_REQUESTS,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_exceeds_max_100001() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: MAX_RATE_LIMIT_REQUESTS + 1,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "rate_limit max_requests too large")?;
    Ok(())
}

#[test]
fn rate_limit_max_entries_at_min_1() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: 1000,
        max_entries: 1,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_entries_at_zero_rejected() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: 1000,
        max_entries: 0,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "rate_limit max_entries must be greater than zero")?;
    Ok(())
}

#[test]
fn rate_limit_max_entries_at_max_65536() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: 1000,
        max_entries: MAX_RATE_LIMIT_ENTRIES,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_entries_exceeds_max_65537() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: 1000,
        max_entries: MAX_RATE_LIMIT_ENTRIES + 1,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "rate_limit max_entries too large")?;
    Ok(())
}

// ============================================================================
// SECTION: Provider Timeout Bounds
// ============================================================================

#[test]
fn provider_connect_timeout_at_min_100() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MIN_PROVIDER_REQUEST_TIMEOUT_MS,
        },
        config: None,
    }];
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_connect_timeout_below_min_99() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MIN_PROVIDER_CONNECT_TIMEOUT_MS - 1,
            request_timeout_ms: MIN_PROVIDER_REQUEST_TIMEOUT_MS,
        },
        config: None,
    }];
    assert_invalid(
        config.validate(),
        "providers.timeouts.connect_timeout_ms must be between 100 and 10000 milliseconds",
    )?;
    Ok(())
}

#[test]
fn provider_connect_timeout_at_max_10000() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MAX_PROVIDER_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MAX_PROVIDER_REQUEST_TIMEOUT_MS,
        },
        config: None,
    }];
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_connect_timeout_above_max_10001() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MAX_PROVIDER_CONNECT_TIMEOUT_MS + 1,
            request_timeout_ms: MAX_PROVIDER_REQUEST_TIMEOUT_MS,
        },
        config: None,
    }];
    assert_invalid(
        config.validate(),
        "providers.timeouts.connect_timeout_ms must be between 100 and 10000 milliseconds",
    )?;
    Ok(())
}

#[test]
fn provider_request_timeout_at_min_500() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MIN_PROVIDER_REQUEST_TIMEOUT_MS,
        },
        config: None,
    }];
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_request_timeout_below_min_499() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MIN_PROVIDER_REQUEST_TIMEOUT_MS - 1,
        },
        config: None,
    }];
    assert_invalid(
        config.validate(),
        "providers.timeouts.request_timeout_ms must be between 500 and 30000 milliseconds",
    )?;
    Ok(())
}

#[test]
fn provider_request_timeout_at_max_30000() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MAX_PROVIDER_REQUEST_TIMEOUT_MS,
        },
        config: None,
    }];
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_request_timeout_above_max_30001() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: MIN_PROVIDER_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MAX_PROVIDER_REQUEST_TIMEOUT_MS + 1,
        },
        config: None,
    }];
    assert_invalid(
        config.validate(),
        "providers.timeouts.request_timeout_ms must be between 500 and 30000 milliseconds",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Namespace Authority Timeout Bounds
// ============================================================================

#[test]
fn namespace_auth_connect_timeout_at_min_100() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        }),
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn namespace_auth_connect_timeout_below_min_99() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS - 1,
            request_timeout_ms: MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        }),
    };
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore.connect_timeout_ms must be between 100 and 10000 \
         milliseconds",
    )?;
    Ok(())
}

#[test]
fn namespace_auth_connect_timeout_at_max_10000() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        }),
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn namespace_auth_connect_timeout_above_max_10001() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS + 1,
            request_timeout_ms: MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        }),
    };
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore.connect_timeout_ms must be between 100 and 10000 \
         milliseconds",
    )?;
    Ok(())
}

#[test]
fn namespace_auth_request_timeout_at_min_500() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        }),
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn namespace_auth_request_timeout_below_min_499() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS - 1,
        }),
    };
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore.request_timeout_ms must be between 500 and 30000 \
         milliseconds",
    )?;
    Ok(())
}

#[test]
fn namespace_auth_request_timeout_at_max_30000() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
        }),
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn namespace_auth_request_timeout_above_max_30001() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
            request_timeout_ms: MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS + 1,
        }),
    };
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore.request_timeout_ms must be between 500 and 30000 \
         milliseconds",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Schema Size Bounds
// ============================================================================

#[test]
fn schema_max_bytes_at_max_10mb() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry = SchemaRegistryConfig {
        registry_type: SchemaRegistryType::Memory,
        path: None,
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_schema_bytes: MAX_SCHEMA_MAX_BYTES,
        max_entries: None,
        acl: RegistryAclConfig {
            mode: RegistryAclMode::Builtin,
            default: RegistryAclDefault::Deny,
            require_signing: false,
            rules: Vec::new(),
        },
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn schema_max_bytes_exceeds_max_10mb_plus_1() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry = SchemaRegistryConfig {
        registry_type: SchemaRegistryType::Memory,
        path: None,
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_schema_bytes: MAX_SCHEMA_MAX_BYTES + 1,
        max_entries: None,
        acl: RegistryAclConfig {
            mode: RegistryAclMode::Builtin,
            default: RegistryAclDefault::Deny,
            require_signing: false,
            rules: Vec::new(),
        },
    };
    assert_invalid(config.validate(), "schema_registry max_schema_bytes out of range")?;
    Ok(())
}

#[test]
fn schema_max_bytes_at_min_1() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry = SchemaRegistryConfig {
        registry_type: SchemaRegistryType::Memory,
        path: None,
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_schema_bytes: 1,
        max_entries: None,
        acl: RegistryAclConfig {
            mode: RegistryAclMode::Builtin,
            default: RegistryAclDefault::Deny,
            require_signing: false,
            rules: Vec::new(),
        },
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn schema_max_bytes_at_zero_rejected() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.schema_registry = SchemaRegistryConfig {
        registry_type: SchemaRegistryType::Memory,
        path: None,
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_schema_bytes: 0,
        max_entries: None,
        acl: RegistryAclConfig {
            mode: RegistryAclMode::Builtin,
            default: RegistryAclDefault::Deny,
            require_signing: false,
            rules: Vec::new(),
        },
    };
    assert_invalid(config.validate(), "schema_registry max_schema_bytes out of range")?;
    Ok(())
}
