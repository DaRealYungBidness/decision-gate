//! Cross-field validation tests for decision-gate-config.
// decision-gate-config/tests/cross_field_validation.rs
// =============================================================================
// Module: Cross-Field Validation Tests
// Description: Comprehensive tests for conditional logic and multi-field constraints.
// Purpose: Ensure cross-field dependencies and constraints are enforced.
// =============================================================================

use std::num::NonZeroU64;
use std::path::PathBuf;

use decision_gate_config::AssetCoreNamespaceAuthorityConfig;
use decision_gate_config::ConfigError;
use decision_gate_config::DevConfig;
use decision_gate_config::DevPermissiveScope;
use decision_gate_config::NamespaceAuthorityConfig;
use decision_gate_config::NamespaceAuthorityMode;
use decision_gate_config::NamespaceConfig;
use decision_gate_config::PolicyConfig;
use decision_gate_config::PolicyEngine;
use decision_gate_config::ProviderConfig;
use decision_gate_config::ProviderTimeoutConfig;
use decision_gate_config::ProviderType;
use decision_gate_config::RunStateStoreConfig;
use decision_gate_config::RunStateStoreType;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerAuthMode;
use decision_gate_config::ServerTlsConfig;
use decision_gate_config::ServerTransport;
use decision_gate_config::ValidationConfig;
use decision_gate_config::ValidationProfile;
use decision_gate_config::policy::PolicyEffect;
use decision_gate_config::policy::StaticPolicyConfig;
use decision_gate_core::TenantId;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;

mod common;

type TestResult = Result<(), String>;

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
// SECTION: Conditional Requirements
// ============================================================================

#[test]
fn validation_strict_false_requires_allow_permissive() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.validation = ValidationConfig {
        strict: false,
        profile: ValidationProfile::StrictCoreV1,
        allow_permissive: false,
        enable_lexicographic: false,
        enable_deep_equals: false,
    };
    assert_invalid(
        config.validate(),
        "validation.strict=false requires validation.allow_permissive=true",
    )?;
    Ok(())
}

#[test]
fn validation_strict_false_with_allow_permissive() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.validation = ValidationConfig {
        strict: false,
        profile: ValidationProfile::StrictCoreV1,
        allow_permissive: true,
        enable_lexicographic: false,
        enable_deep_equals: false,
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn namespace_allow_default_requires_default_tenants() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace = NamespaceConfig {
        allow_default: true,
        default_tenants: Vec::new(),
        authority: NamespaceAuthorityConfig {
            mode: NamespaceAuthorityMode::None,
            assetcore: None,
        },
    };
    assert_invalid(
        config.validate(),
        "namespace.allow_default requires namespace.default_tenants",
    )?;
    Ok(())
}

#[test]
fn namespace_allow_default_with_default_tenants() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace = NamespaceConfig {
        allow_default: true,
        default_tenants: vec![TenantId::new(NonZeroU64::MIN)],
        authority: NamespaceAuthorityConfig {
            mode: NamespaceAuthorityMode::None,
            assetcore: None,
        },
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn dev_permissive_rejected_with_assetcore_authority() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.dev = DevConfig {
        permissive: true,
        permissive_scope: DevPermissiveScope::AssertedEvidenceOnly,
        permissive_ttl_days: None,
        permissive_warn: true,
        permissive_exempt_providers: Vec::new(),
    };
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: 1000,
            request_timeout_ms: 5000,
        }),
    };
    assert_invalid(
        config.validate(),
        "dev.permissive not allowed when namespace.authority.mode=assetcore_http",
    )?;
    Ok(())
}

#[test]
fn assetcore_http_mode_requires_assetcore_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::AssetcoreHttp,
        assetcore: None,
    };
    assert_invalid(
        config.validate(),
        "namespace.authority.mode=assetcore_http requires namespace.authority.assetcore",
    )?;
    Ok(())
}

#[test]
fn bearer_token_mode_requires_bearer_tokens() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "bearer_token auth requires bearer_tokens")?;
    Ok(())
}

#[test]
fn mtls_mode_requires_mtls_subjects() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "mtls auth requires mtls_subjects")?;
    Ok(())
}

#[test]
fn static_policy_engine_requires_static_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.policy = PolicyConfig {
        engine: PolicyEngine::Static,
        static_policy: None,
    };
    assert_invalid(config.validate(), "policy.engine=static requires policy.static")?;
    Ok(())
}

// ============================================================================
// SECTION: Mutually Exclusive Configs
// ============================================================================

#[test]
fn permit_all_engine_rejects_static_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.policy = PolicyConfig {
        engine: PolicyEngine::PermitAll,
        static_policy: Some(StaticPolicyConfig {
            default: PolicyEffect::Deny,
            rules: Vec::new(),
        }),
    };
    assert_invalid(config.validate(), "policy.static only allowed when engine=static")?;
    Ok(())
}

#[test]
fn deny_all_engine_rejects_static_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.policy = PolicyConfig {
        engine: PolicyEngine::DenyAll,
        static_policy: Some(StaticPolicyConfig {
            default: PolicyEffect::Deny,
            rules: Vec::new(),
        }),
    };
    assert_invalid(config.validate(), "policy.static only allowed when engine=static")?;
    Ok(())
}

#[test]
fn namespace_authority_none_rejects_assetcore_config() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.namespace.authority = NamespaceAuthorityConfig {
        mode: NamespaceAuthorityMode::None,
        assetcore: Some(AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: None,
            connect_timeout_ms: 1000,
            request_timeout_ms: 5000,
        }),
    };
    assert_invalid(
        config.validate(),
        "namespace.authority.assetcore only allowed when mode=assetcore_http",
    )?;
    Ok(())
}

// ============================================================================
// SECTION: Multi-Field Constraints
// ============================================================================

#[test]
fn request_timeout_must_exceed_connect_timeout() -> TestResult {
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
            connect_timeout_ms: 2000,
            request_timeout_ms: 1000,
        },
        config: None,
    }];
    assert_invalid(config.validate(), "request_timeout_ms must be >= connect_timeout_ms")?;
    Ok(())
}

#[test]
fn request_timeout_equal_to_connect_timeout() -> TestResult {
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
            connect_timeout_ms: 1000,
            request_timeout_ms: 1000,
        },
        config: None,
    }];
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn http_transport_requires_bind() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = None;
    assert_invalid(config.validate(), "http/sse transport requires bind address")?;
    Ok(())
}

#[test]
fn sse_transport_requires_bind() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Sse;
    config.server.bind = None;
    assert_invalid(config.validate(), "http/sse transport requires bind address")?;
    Ok(())
}

#[test]
fn non_loopback_bind_requires_auth() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Http;
    config.server.bind = Some("0.0.0.0:8080".to_string());
    config.server.auth = None;
    assert_invalid(config.validate(), "non-loopback bind disallowed without auth policy")?;
    Ok(())
}

#[test]
fn stdio_transport_rejects_non_local_auth() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Stdio;
    config.server.auth = Some(ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    });
    assert_invalid(config.validate(), "stdio transport only supports local_only auth")?;
    Ok(())
}

#[test]
fn stdio_transport_rejects_tls() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = ServerTransport::Stdio;
    config.server.tls = Some(ServerTlsConfig {
        cert_path: "cert.pem".to_string(),
        key_path: "key.pem".to_string(),
        client_ca_path: None,
        require_client_cert: false,
    });
    assert_invalid(config.validate(), "stdio transport does not support tls")?;
    Ok(())
}

// ============================================================================
// SECTION: Type-Specific Validation
// ============================================================================

#[test]
fn memory_store_rejects_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Memory,
        path: Some(PathBuf::from("store.db")),
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions: None,
    };
    assert_invalid(config.validate(), "memory run_state_store must not set path")?;
    Ok(())
}

#[test]
fn sqlite_store_requires_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: None,
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions: None,
    };
    assert_invalid(config.validate(), "sqlite run_state_store requires path")?;
    Ok(())
}

#[test]
fn builtin_provider_rejects_capabilities_path() -> TestResult {
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
fn mcp_provider_requires_capabilities_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["./provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    assert_invalid(config.validate(), "mcp provider requires capabilities_path")?;
    Ok(())
}

#[test]
fn mcp_provider_requires_command_or_url() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
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
    assert_invalid(config.validate(), "mcp provider requires command or url")?;
    Ok(())
}

#[test]
fn http_provider_url_insecure_requires_allow() -> TestResult {
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
