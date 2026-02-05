//! Authentication config validation tests for decision-gate-config.
// crates/decision-gate-config/tests/auth_validation.rs
// =============================================================================
// Module: Authentication Config Validation Tests
// Description: Comprehensive tests for auth constraints and limits.
// Purpose: Ensure auth validation is fail-closed and enforces all limits.
// =============================================================================

use std::num::NonZeroU64;

use decision_gate_config::ConfigError;
use decision_gate_config::PrincipalConfig;
use decision_gate_config::PrincipalRoleConfig;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerAuthMode;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;

mod common;

type TestResult = Result<(), String>;

// Test constants (from config.rs)
const MAX_AUTH_TOKENS: usize = 64;
const MAX_AUTH_TOKEN_LENGTH: usize = 256;
const MAX_AUTH_SUBJECT_LENGTH: usize = 512;
const MAX_AUTH_TOOL_RULES: usize = 128;
const MAX_PRINCIPAL_ROLES: usize = 128;

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
// SECTION: Bearer Token Constraints
// ============================================================================

#[test]
fn auth_bearer_token_at_max_length_256() -> TestResult {
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
fn auth_bearer_token_exceeds_max_length_257() -> TestResult {
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
fn auth_bearer_token_empty_string() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![String::new()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth token must be non-empty")?;
    Ok(())
}

#[test]
fn auth_bearer_token_whitespace_only() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["   ".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth token must be non-empty")?;
    Ok(())
}

#[test]
fn auth_bearer_token_with_leading_whitespace() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![" token".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth token must not contain whitespace")?;
    Ok(())
}

#[test]
fn auth_bearer_token_with_trailing_whitespace() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token ".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth token must not contain whitespace")?;
    Ok(())
}

#[test]
fn auth_bearer_token_with_internal_whitespace() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["to ken".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_bearer_token_with_newline() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\nvalue".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_bearer_token_with_tab() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\tvalue".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Bearer Tokens Array Limits
// ============================================================================

#[test]
fn auth_bearer_tokens_array_at_max_64() -> TestResult {
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
fn auth_bearer_tokens_array_exceeds_max_65() -> TestResult {
    let tokens: Vec<String> = (0 ..= MAX_AUTH_TOKENS).map(|i| format!("token{i}")).collect();
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
fn auth_bearer_tokens_empty_array_local_only_mode() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_bearer_tokens_empty_array_bearer_mode() -> TestResult {
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

// ============================================================================
// SECTION: mTLS Subject Constraints
// ============================================================================

#[test]
fn auth_mtls_subject_at_max_length_512() -> TestResult {
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
fn auth_mtls_subject_exceeds_max_length_513() -> TestResult {
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

#[test]
fn auth_mtls_subject_empty_string() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec![String::new()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "mTLS subject must be non-empty")?;
    Ok(())
}

#[test]
fn auth_mtls_subject_whitespace_only() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec!["   ".to_string()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "mTLS subject must be non-empty")?;
    Ok(())
}

#[test]
fn auth_mtls_subject_with_whitespace_allowed() -> TestResult {
    // mTLS subjects can contain whitespace (unlike bearer tokens)
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec!["CN=Test User, OU=Engineering".to_string()],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: mTLS Subjects Array Limits
// ============================================================================

#[test]
fn auth_mtls_subjects_array_at_max_64() -> TestResult {
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
fn auth_mtls_subjects_array_exceeds_max_65() -> TestResult {
    let subjects: Vec<String> = (0 ..= MAX_AUTH_TOKENS).map(|i| format!("CN=subject{i}")).collect();
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
fn auth_mtls_subjects_empty_array_mtls_mode() -> TestResult {
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

// ============================================================================
// SECTION: Tool Allowlist Constraints
// ============================================================================

#[test]
fn auth_allowed_tools_valid_tool_names() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: vec!["precheck".to_string(), "scenario_next".to_string()],
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn auth_allowed_tools_invalid_tool_name() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: vec!["invalid.tool.name".to_string()],
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "unknown tool in allowlist")?;
    Ok(())
}

#[test]
fn auth_allowed_tools_array_at_max_128() -> TestResult {
    // Repeat valid tool names to reach the limit
    let mut tools = Vec::new();
    for i in 0 .. MAX_AUTH_TOOL_RULES {
        // Alternate between valid tool names
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
fn auth_allowed_tools_array_exceeds_max_129() -> TestResult {
    let mut tools = Vec::new();
    for i in 0 ..= MAX_AUTH_TOOL_RULES {
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
fn auth_allowed_tools_empty_array() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Principal Validation
// ============================================================================

#[test]
fn auth_principal_subject_valid() -> TestResult {
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: Vec::new(),
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
fn auth_principal_subject_empty() -> TestResult {
    let principal = PrincipalConfig {
        subject: String::new(),
        policy_class: None,
        roles: Vec::new(),
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.subject must be non-empty")?;
    Ok(())
}

#[test]
fn auth_principal_subject_whitespace() -> TestResult {
    let principal = PrincipalConfig {
        subject: "   ".to_string(),
        policy_class: None,
        roles: Vec::new(),
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.subject must be non-empty")?;
    Ok(())
}

#[test]
fn auth_principal_policy_class_valid() -> TestResult {
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: Some("production".to_string()),
        roles: Vec::new(),
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
fn auth_principal_policy_class_empty() -> TestResult {
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: Some(String::new()),
        roles: Vec::new(),
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.policy_class must be non-empty")?;
    Ok(())
}

#[test]
fn auth_principal_policy_class_whitespace() -> TestResult {
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: Some("   ".to_string()),
        roles: Vec::new(),
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.policy_class must be non-empty")?;
    Ok(())
}

// ============================================================================
// SECTION: Principal Roles Constraints
// ============================================================================

#[test]
fn auth_principal_roles_at_max_128() -> TestResult {
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
fn auth_principal_roles_exceeds_max_129() -> TestResult {
    let roles: Vec<PrincipalRoleConfig> = (0 ..= MAX_PRINCIPAL_ROLES)
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

// ============================================================================
// SECTION: Principals Array Limits
// ============================================================================

#[test]
fn auth_principals_array_at_max_64() -> TestResult {
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
fn auth_principals_array_exceeds_max_65() -> TestResult {
    let principals: Vec<PrincipalConfig> = (0 ..= MAX_AUTH_TOKENS)
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

// ============================================================================
// SECTION: Role Validation
// ============================================================================

#[test]
fn auth_role_name_valid() -> TestResult {
    let role = PrincipalRoleConfig {
        name: "NamespaceAdmin".to_string(),
        tenant_id: None,
        namespace_id: None,
    };
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: vec![role],
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
fn auth_role_name_empty() -> TestResult {
    let role = PrincipalRoleConfig {
        name: String::new(),
        tenant_id: None,
        namespace_id: None,
    };
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: vec![role],
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.roles.name must be non-empty")?;
    Ok(())
}

#[test]
fn auth_role_name_whitespace() -> TestResult {
    let role = PrincipalRoleConfig {
        name: "   ".to_string(),
        tenant_id: None,
        namespace_id: None,
    };
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: vec![role],
    };
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: vec![principal],
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth.principals.roles.name must be non-empty")?;
    Ok(())
}

#[test]
fn auth_role_with_tenant_id_only() -> TestResult {
    let role = PrincipalRoleConfig {
        name: "TenantAdmin".to_string(),
        tenant_id: Some(TenantId::new(NonZeroU64::MIN)),
        namespace_id: None,
    };
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: vec![role],
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
fn auth_role_with_namespace_id_only() -> TestResult {
    let role = PrincipalRoleConfig {
        name: "NamespaceAdmin".to_string(),
        tenant_id: None,
        namespace_id: Some(NamespaceId::new(NonZeroU64::MIN)),
    };
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: vec![role],
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
fn auth_role_with_both_tenant_and_namespace() -> TestResult {
    let role = PrincipalRoleConfig {
        name: "ScopedAdmin".to_string(),
        tenant_id: Some(TenantId::new(NonZeroU64::MIN)),
        namespace_id: Some(NamespaceId::new(NonZeroU64::new(2).unwrap_or(NonZeroU64::MIN))),
    };
    let principal = PrincipalConfig {
        subject: "user@example.com".to_string(),
        policy_class: None,
        roles: vec![role],
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

// ============================================================================
// SECTION: Auth Mode Cross-Field Validation
// ============================================================================

#[test]
fn auth_mode_bearer_token_requires_tokens() -> TestResult {
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
fn auth_mode_mtls_requires_subjects() -> TestResult {
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
fn auth_mode_local_only_allows_empty_arrays() -> TestResult {
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}
