//! Security validation tests for decision-gate-config.
// decision-gate-config/tests/security_validation.rs
// =============================================================================
// Module: Security Validation Tests
// Description: Comprehensive tests for security constraints and attack prevention.
// Purpose: Ensure path traversal, injection, and special character handling.
// =============================================================================

use std::path::PathBuf;

use decision_gate_config::ConfigError;
use decision_gate_config::ObjectStoreConfig;
use decision_gate_config::ObjectStoreProvider;
use decision_gate_config::ProviderConfig;
use decision_gate_config::ProviderTimeoutConfig;
use decision_gate_config::ProviderType;
use decision_gate_config::RunStateStoreConfig;
use decision_gate_config::RunStateStoreType;
use decision_gate_config::RunpackStorageConfig;
use decision_gate_config::ServerAuditConfig;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerAuthMode;
use decision_gate_config::ServerTlsConfig;
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
// SECTION: Path Traversal Prevention
// ============================================================================

#[test]
fn tls_cert_path_with_parent_directory_traversal() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = decision_gate_config::ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.tls = Some(ServerTlsConfig {
        cert_path: "../../../etc/passwd".to_string(),
        key_path: "key.pem".to_string(),
        client_ca_path: None,
        require_client_cert: false,
    });
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn tls_key_path_with_dot_dot_component() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = decision_gate_config::ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.tls = Some(ServerTlsConfig {
        cert_path: "cert.pem".to_string(),
        key_path: "path/../../../secret/key.pem".to_string(),
        client_ca_path: None,
        require_client_cert: false,
    });
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn tls_client_ca_path_with_traversal() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.transport = decision_gate_config::ServerTransport::Http;
    config.server.bind = Some("127.0.0.1:8080".to_string());
    config.server.tls = Some(ServerTlsConfig {
        cert_path: "cert.pem".to_string(),
        key_path: "key.pem".to_string(),
        client_ca_path: Some("../../ca.pem".to_string()),
        require_client_cert: false,
    });
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn audit_path_with_parent_directory() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some("../../../var/log/audit.log".to_string()),
        log_precheck_payloads: false,
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn store_path_with_traversal_attack() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: Some(PathBuf::from("../../etc/passwd")),
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions: None,
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn runpack_prefix_absolute_path() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my-bucket".to_string(),
        region: None,
        endpoint: None,
        prefix: Some("/absolute/path".to_string()),
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.prefix must be relative")?;
    Ok(())
}

#[test]
fn runpack_prefix_with_dot_dot() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my-bucket".to_string(),
        region: None,
        endpoint: None,
        prefix: Some("path/../other".to_string()),
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.prefix must be relative without traversal")?;
    Ok(())
}

#[test]
fn runpack_prefix_with_dot_segment() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my-bucket".to_string(),
        region: None,
        endpoint: None,
        prefix: Some("path/./other".to_string()),
        force_path_style: false,
        allow_http: false,
    }));
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn runpack_prefix_with_backslashes() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my-bucket".to_string(),
        region: None,
        endpoint: None,
        prefix: Some("path\\..\\other".to_string()),
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.prefix must not contain backslashes")?;
    Ok(())
}

#[test]
fn path_with_null_bytes() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some("audit\0.log".to_string()),
        log_precheck_payloads: false,
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Injection Prevention
// ============================================================================

#[test]
fn bearer_token_with_sql_injection_payload() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["'; DROP TABLE users--".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    // Should validate (SQL injection payloads are just strings)
    // The validation checks format, not content interpretation
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_name_with_shell_metacharacters_semicolon() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "provider;rm -rf /".to_string(),
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
    // Should validate (provider names are not executed as shell commands)
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_name_with_pipe() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "provider | cat /etc/passwd".to_string(),
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
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_url_with_crlf_injection() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("http://example.com\r\nHost: evil.com".to_string()),
        allow_insecure_http: true,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    // Should validate (URL parsing will fail at runtime, but config validation passes)
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_token_with_null_byte() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\0value".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    // Null bytes in strings are allowed (not executed as commands)
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Special Character Handling
// ============================================================================

#[test]
fn provider_name_with_forward_slash() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "provider/name".to_string(),
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
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_name_with_backslash() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "provider\\name".to_string(),
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
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn provider_name_with_colon() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "provider:name".to_string(),
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
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn bucket_name_with_invalid_s3_characters() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my_bucket!@#$%".to_string(),
        region: None,
        endpoint: None,
        prefix: None,
        force_path_style: false,
        allow_http: false,
    }));
    // Bucket name validation is handled by S3 at runtime, not in config
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_token_with_control_characters() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\x01\x02\x03".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    // Control characters are allowed in token values (not whitespace)
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Unicode Edge Cases
// ============================================================================

#[test]
fn provider_name_with_emoji() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "provider🚀".to_string(),
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
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_token_with_unicode() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["tøken-välue-日本語".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn path_with_utf8_multibyte() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.audit = ServerAuditConfig {
        enabled: true,
        path: Some("audit/日本語/log.json".to_string()),
        log_precheck_payloads: false,
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn field_with_unicode_whitespace_u00a0() -> TestResult {
    // U+00A0 is non-breaking space
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\u{00A0}value".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn field_with_unicode_whitespace_u2000() -> TestResult {
    // U+2000 is en quad
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\u{2000}value".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn string_with_very_long_grapheme_cluster() -> TestResult {
    // Combining characters can create very long grapheme clusters
    let mut base = "e".to_string();
    for _ in 0 .. 100 {
        base.push('\u{0301}'); // Combining acute accent
    }
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![base],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: HTTP/HTTPS Security
// ============================================================================

#[test]
fn provider_url_insecure_http_without_allow() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
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
fn provider_url_insecure_http_with_allow() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.providers = vec![ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("http://example.com/mcp".to_string()),
        allow_insecure_http: true,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }];
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn runpack_endpoint_insecure_http_without_allow() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my-bucket".to_string(),
        region: None,
        endpoint: Some("http://minio.local:9000".to_string()),
        prefix: None,
        force_path_style: false,
        allow_http: false,
    }));
    assert_invalid(config.validate(), "runpack_storage.endpoint uses http:// without allow_http")?;
    Ok(())
}

#[test]
fn runpack_endpoint_insecure_http_with_allow() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.runpack_storage = Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
        provider: ObjectStoreProvider::S3,
        bucket: "my-bucket".to_string(),
        region: None,
        endpoint: Some("http://minio.local:9000".to_string()),
        prefix: None,
        force_path_style: false,
        allow_http: true,
    }));
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}
