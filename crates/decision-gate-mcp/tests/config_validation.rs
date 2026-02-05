// crates/decision-gate-mcp/tests/config_validation.rs
// ============================================================================
// Module: Configuration Validation Tests
// Description: Tests for MCP config loading and validation.
// Purpose: Verify security constraints are enforced during config parsing.
// Dependencies: decision-gate-mcp
// ============================================================================

//! ## Overview
//! Tests configuration validation including loopback enforcement, path limits,
//! and provider configuration requirements.
//!
//! Security posture: Configuration is untrusted input - all limits must be enforced.
//! Threat model: TM-CFG-001 - Configuration injection or bypass.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

use std::path::PathBuf;

use decision_gate_core::TenantId;
use decision_gate_core::TrustLane;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::config::AnchorPolicyConfig;
use decision_gate_mcp::config::AnchorProviderConfig;
use decision_gate_mcp::config::DocsConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::NamespaceConfig;
use decision_gate_mcp::config::ObjectStoreConfig;
use decision_gate_mcp::config::ObjectStoreProvider;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::config::ProviderAuthConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RegistryAclConfig;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::RunpackStorageConfig;
use decision_gate_mcp::config::SchemaRegistryConfig;
use decision_gate_mcp::config::ServerAuditConfig;
use decision_gate_mcp::config::ServerAuthConfig;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::ServerFeedbackConfig;
use decision_gate_mcp::config::ServerLimitsConfig;
use decision_gate_mcp::config::ServerMode;
use decision_gate_mcp::config::ServerTlsConfig;
use decision_gate_mcp::config::ServerTlsTermination;
use decision_gate_mcp::config::ServerToolsConfig;
use decision_gate_mcp::config::ServerTransport;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::config::ValidationConfig;
use decision_gate_mcp::policy::DispatchTargetKind;
use decision_gate_mcp::policy::PolicyEffect;
use decision_gate_mcp::policy::PolicyEngine;
use decision_gate_mcp::policy::PolicyRule;
use decision_gate_mcp::policy::PolicyTargetSelector;
use decision_gate_mcp::policy::StaticPolicyConfig;
use tempfile::TempDir;

/// Validates a standalone server config via the public config validator.
fn validate_server_config(
    server: ServerConfig,
) -> Result<(), decision_gate_mcp::config::ConfigError> {
    let mut config = DecisionGateConfig {
        server,
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    config.validate()
}

/// Validates a standalone provider config via the public config validator.
fn validate_provider_config(
    provider: ProviderConfig,
) -> Result<(), decision_gate_mcp::config::ConfigError> {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: vec![provider],
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    config.validate()
}

/// Verifies static policy requires a static config block.
#[test]
fn policy_static_requires_config() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig {
            engine: PolicyEngine::Static,
            static_policy: None,
        },
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("policy.engine=static"));
}

/// Verifies static policy rules must include match criteria.
#[test]
fn policy_static_rejects_empty_rule() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig {
            engine: PolicyEngine::Static,
            static_policy: Some(StaticPolicyConfig {
                default: PolicyEffect::Permit,
                rules: vec![PolicyRule {
                    effect: PolicyEffect::Deny,
                    error_message: None,
                    target_kinds: Vec::new(),
                    targets: Vec::new(),
                    require_labels: Vec::new(),
                    forbid_labels: Vec::new(),
                    require_policy_tags: Vec::new(),
                    forbid_policy_tags: Vec::new(),
                    content_types: Vec::new(),
                    schema_ids: Vec::new(),
                    packet_ids: Vec::new(),
                    stage_ids: Vec::new(),
                    scenario_ids: Vec::new(),
                }],
            }),
        },
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("policy.rules"));
}

/// Verifies error rules require an error message.
#[test]
fn policy_static_error_requires_message() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig {
            engine: PolicyEngine::Static,
            static_policy: Some(StaticPolicyConfig {
                default: PolicyEffect::Permit,
                rules: vec![PolicyRule {
                    effect: PolicyEffect::Error,
                    error_message: None,
                    target_kinds: Vec::new(),
                    targets: Vec::new(),
                    require_labels: vec!["internal".to_string()],
                    forbid_labels: Vec::new(),
                    require_policy_tags: Vec::new(),
                    forbid_policy_tags: Vec::new(),
                    content_types: Vec::new(),
                    schema_ids: Vec::new(),
                    packet_ids: Vec::new(),
                    stage_ids: Vec::new(),
                    scenario_ids: Vec::new(),
                }],
            }),
        },
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("error_message"));
}

/// Verifies external targets cannot set `target_id`.
#[test]
fn policy_static_rejects_external_target_id() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig {
            engine: PolicyEngine::Static,
            static_policy: Some(StaticPolicyConfig {
                default: PolicyEffect::Deny,
                rules: vec![PolicyRule {
                    effect: PolicyEffect::Permit,
                    error_message: None,
                    target_kinds: Vec::new(),
                    targets: vec![PolicyTargetSelector {
                        target_kind: DispatchTargetKind::External,
                        target_id: Some("bad-target".to_string()),
                        system: Some("system-a".to_string()),
                        target: None,
                    }],
                    require_labels: Vec::new(),
                    forbid_labels: Vec::new(),
                    require_policy_tags: Vec::new(),
                    forbid_policy_tags: Vec::new(),
                    content_types: Vec::new(),
                    schema_ids: Vec::new(),
                    packet_ids: Vec::new(),
                    stage_ids: Vec::new(),
                    scenario_ids: Vec::new(),
                }],
            }),
        },
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("target_id"));
}

/// Verifies non-external targets cannot set external selector fields.
#[test]
fn policy_static_rejects_agent_with_system() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig {
            engine: PolicyEngine::Static,
            static_policy: Some(StaticPolicyConfig {
                default: PolicyEffect::Deny,
                rules: vec![PolicyRule {
                    effect: PolicyEffect::Permit,
                    error_message: None,
                    target_kinds: Vec::new(),
                    targets: vec![PolicyTargetSelector {
                        target_kind: DispatchTargetKind::Agent,
                        target_id: Some("agent-1".to_string()),
                        system: Some("system-a".to_string()),
                        target: None,
                    }],
                    require_labels: Vec::new(),
                    forbid_labels: Vec::new(),
                    require_policy_tags: Vec::new(),
                    forbid_policy_tags: Vec::new(),
                    content_types: Vec::new(),
                    schema_ids: Vec::new(),
                    packet_ids: Vec::new(),
                    stage_ids: Vec::new(),
                    scenario_ids: Vec::new(),
                }],
            }),
        },
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("non-external"));
}

// ============================================================================
// SECTION: Server Config Validation Tests
// ============================================================================

/// Verifies stdio transport requires no bind address.
#[test]
fn server_stdio_no_bind_required() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: None,
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies `max_body_bytes` must be non-zero.
#[test]
fn server_max_body_bytes_zero_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: None,
        max_body_bytes: 0,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("max_body_bytes"));
}

/// Verifies HTTP transport requires bind address.
#[test]
fn server_http_requires_bind() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: None,
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("bind address"));
}

/// Verifies SSE transport requires bind address.
#[test]
fn server_sse_requires_bind() {
    let config = ServerConfig {
        transport: ServerTransport::Sse,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: None,
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("bind address"));
}

/// Verifies HTTP transport allows loopback bind.
#[test]
fn server_http_loopback_allowed() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    assert!(validate_server_config(config).is_ok());
}

// ============================================================================
// SECTION: Docs + Tool Visibility Config Tests
// ============================================================================

#[test]
fn server_tools_unknown_tool_rejected() {
    let config = ServerConfig {
        tools: ServerToolsConfig {
            allowlist: vec!["not_a_tool".to_string()],
            ..ServerToolsConfig::default()
        },
        ..ServerConfig::default()
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("unknown tool"));
}

#[test]
fn server_tools_empty_entry_rejected() {
    let config = ServerConfig {
        tools: ServerToolsConfig {
            denylist: vec![" ".to_string()],
            ..ServerToolsConfig::default()
        },
        ..ServerConfig::default()
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("non-empty"));
}

#[test]
fn docs_max_doc_bytes_zero_rejected() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,
        source_modified_at: None,
    };
    config.docs.max_doc_bytes = 0;
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("docs.max_doc_bytes"));
}

#[test]
fn docs_max_sections_too_high_rejected() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,
        source_modified_at: None,
    };
    config.docs.max_sections = 99;
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("docs.max_sections"));
}

#[test]
fn docs_extra_paths_too_many_rejected() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,
        source_modified_at: None,
    };
    config.docs.extra_paths = (0 .. 65).map(|idx| format!("doc-{idx}.md")).collect();
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("docs.extra_paths"));
}

/// Verifies HTTP transport allows IPv6 loopback.
#[test]
fn server_http_ipv6_loopback_allowed() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("[::1]:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies HTTP transport rejects non-loopback bind.
#[test]
fn server_http_non_loopback_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("0.0.0.0:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("loopback"));
}

/// Verifies HTTP transport rejects external IP bind.
#[test]
fn server_http_external_ip_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("192.168.1.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies invalid bind address format rejected.
#[test]
fn server_invalid_bind_format_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("not-an-address".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("invalid bind"));
}

/// Verifies empty bind address rejected.
#[test]
fn server_empty_bind_rejected() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("   ".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies non-loopback bind is allowed with bearer auth configured.
#[test]
fn server_http_non_loopback_allowed_with_bearer_auth() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("0.0.0.0:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token-1".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: Vec::new(),
        }),
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    assert!(validate_server_config(config).is_ok());
}

/// Verifies stdio transport rejects bearer auth mode.
#[test]
fn server_stdio_rejects_bearer_auth() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: None,
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token-1".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: Vec::new(),
        }),
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies bearer auth requires at least one token.
#[test]
fn server_auth_bearer_requires_token() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: Vec::new(),
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: Vec::new(),
        }),
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies tool allowlist rejects unknown tools.
#[test]
fn server_auth_rejects_unknown_tool_in_allowlist() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::BearerToken,
            bearer_tokens: vec!["token-1".to_string()],
            mtls_subjects: Vec::new(),
            allowed_tools: vec!["invalid_tool".to_string()],
            principals: Vec::new(),
        }),
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies mTLS auth requires at least one subject.
#[test]
fn server_auth_mtls_requires_subjects() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: Some(ServerAuthConfig {
            mode: ServerAuthMode::Mtls,
            bearer_tokens: Vec::new(),
            mtls_subjects: Vec::new(),
            allowed_tools: Vec::new(),
            principals: Vec::new(),
        }),
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies `max_inflight` must be non-zero.
#[test]
fn server_limits_rejects_zero_inflight() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig {
            max_inflight: 0,
            rate_limit: None,
        },
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies rate limit requires `max_requests`.
#[test]
fn server_rate_limit_rejects_zero_requests() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig {
            max_inflight: 64,
            rate_limit: Some(decision_gate_mcp::config::RateLimitConfig {
                max_requests: 0,
                window_ms: 1_000,
                max_entries: 8,
            }),
        },
        auth: None,
        tls: None,
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies TLS config requires non-empty paths.
#[test]
fn server_tls_rejects_empty_paths() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: Some(ServerTlsConfig {
            cert_path: "   ".to_string(),
            key_path: String::new(),
            client_ca_path: None,
            require_client_cert: true,
        }),
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies stdio transport rejects TLS configuration.
#[test]
fn server_stdio_rejects_tls() {
    let config = ServerConfig {
        transport: ServerTransport::Stdio,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: None,
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: Some(ServerTlsConfig {
            cert_path: "cert.pem".to_string(),
            key_path: "key.pem".to_string(),
            client_ca_path: None,
            require_client_cert: true,
        }),
        audit: ServerAuditConfig::default(),
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

/// Verifies audit path rejects empty values.
#[test]
fn server_audit_rejects_empty_path() {
    let config = ServerConfig {
        transport: ServerTransport::Http,
        mode: ServerMode::Strict,
        tls_termination: ServerTlsTermination::Server,
        bind: Some("127.0.0.1:8080".to_string()),
        max_body_bytes: 1024 * 1024,
        limits: ServerLimitsConfig::default(),
        auth: None,
        tls: None,
        audit: ServerAuditConfig {
            enabled: true,
            path: Some("   ".to_string()),
            log_precheck_payloads: false,
        },
        feedback: ServerFeedbackConfig::default(),
        tools: ServerToolsConfig::default(),
    };
    let result = validate_server_config(config);
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Provider Config Validation Tests
// ============================================================================

/// Verifies builtin provider with name is valid.
#[test]
fn provider_builtin_valid() {
    let config = ProviderConfig {
        name: "time".to_string(),
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
    };
    assert!(validate_provider_config(config).is_ok());
}

/// Verifies empty provider name rejected.
#[test]
fn provider_empty_name_rejected() {
    let config = ProviderConfig {
        name: String::new(),
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("name"));
}

/// Verifies whitespace-only provider name rejected.
#[test]
fn provider_whitespace_name_rejected() {
    let config = ProviderConfig {
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
}

/// Verifies MCP provider requires command or URL.
#[test]
fn provider_mcp_requires_command_or_url() {
    let config = ProviderConfig {
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("command or url"));
}

/// Verifies builtin provider rejects url.
#[test]
fn provider_builtin_rejects_url() {
    let config = ProviderConfig {
        name: "time".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("builtin provider does not accept url"));
}

/// Verifies builtin provider rejects `allow_insecure_http`.
#[test]
fn provider_builtin_rejects_allow_insecure_http() {
    let config = ProviderConfig {
        name: "time".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: true,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("builtin provider does not accept allow_insecure_http"));
}

/// Verifies builtin provider rejects auth block.
#[test]
fn provider_builtin_rejects_auth() {
    let config = ProviderConfig {
        name: "time".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: Some(ProviderAuthConfig {
            bearer_token: Some("token".to_string()),
        }),
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("builtin provider does not accept auth"));
}

/// Verifies MCP provider with command is valid.
#[test]
fn provider_mcp_with_command_valid() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["./provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    assert!(validate_provider_config(config).is_ok());
}

/// Verifies MCP provider with HTTPS URL is valid.
#[test]
fn provider_mcp_with_https_url_valid() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    assert!(validate_provider_config(config).is_ok());
}

/// Verifies provider auth rejects empty bearer tokens.
#[test]
fn provider_auth_rejects_empty_bearer_token() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["./provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: Some(ProviderAuthConfig {
            bearer_token: Some("  ".to_string()),
        }),
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("providers.auth.bearer_token"));
}

/// Verifies provider timeouts reject out-of-range connect values.
#[test]
fn provider_timeouts_reject_connect_out_of_range() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig {
            connect_timeout_ms: 50,
            request_timeout_ms: 1_000,
        },
        config: None,
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("connect_timeout_ms"));
}

/// Verifies provider timeouts require request timeout >= connect timeout.
#[test]
fn provider_timeouts_reject_request_below_connect() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("https://example.com/mcp".to_string()),
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("request_timeout_ms"));
}

// ============================================================================
// SECTION: Schema Registry Validation Tests
// ============================================================================

/// Verifies memory `schema_registry` rejects a path.
#[test]
fn schema_registry_memory_rejects_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig {
            registry_type: decision_gate_mcp::config::SchemaRegistryType::Memory,
            path: Some(PathBuf::from("schema.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_schema_bytes: SchemaRegistryConfig::default().max_schema_bytes,
            max_entries: None,
            acl: RegistryAclConfig::default(),
        },
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
}

/// Verifies sqlite `schema_registry` requires a path.
#[test]
fn schema_registry_sqlite_requires_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig {
            registry_type: decision_gate_mcp::config::SchemaRegistryType::Sqlite,
            path: None,
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_schema_bytes: SchemaRegistryConfig::default().max_schema_bytes,
            max_entries: None,
            acl: RegistryAclConfig::default(),
        },
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("schema_registry"));
}

// ============================================================================
// SECTION: Validation Config Tests
// ============================================================================

#[test]
fn validation_strict_disabled_requires_allow_permissive() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig {
            strict: false,
            allow_permissive: false,
            ..ValidationConfig::default()
        },
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("allow_permissive"));
}

// ============================================================================
// SECTION: Server Mode + Namespace Policy Tests
// ============================================================================

#[test]
fn dev_permissive_forces_asserted_trust_lane() {
    let config = DecisionGateConfig {
        server: ServerConfig {
            mode: ServerMode::DevPermissive,
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig {
            min_lane: TrustLane::Verified,
            ..TrustConfig::default()
        },
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    assert_eq!(config.effective_trust_requirement().min_lane, TrustLane::Asserted);
}

#[test]
fn strict_mode_uses_configured_trust_lane() {
    let config = DecisionGateConfig {
        server: ServerConfig {
            mode: ServerMode::Strict,
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig {
            min_lane: TrustLane::Asserted,
            ..TrustConfig::default()
        },
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    assert_eq!(config.effective_trust_requirement().min_lane, TrustLane::Asserted);
}

#[test]
fn dev_permissive_does_not_override_default_namespace() {
    let config = DecisionGateConfig {
        server: ServerConfig {
            mode: ServerMode::DevPermissive,
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig {
            allow_default: false,
            ..NamespaceConfig::default()
        },
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    assert!(!config.allow_default_namespace());
}

#[test]
fn strict_mode_requires_explicit_default_namespace_allow() {
    let mut config = DecisionGateConfig {
        server: ServerConfig {
            mode: ServerMode::Strict,
            tools: ServerToolsConfig::default(),
            ..ServerConfig::default()
        },
        namespace: NamespaceConfig {
            allow_default: false,
            ..NamespaceConfig::default()
        },
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    assert!(!config.allow_default_namespace());
    config.namespace.allow_default = true;
    config.namespace.default_tenants = vec![TenantId::from_raw(100).expect("nonzero tenantid")];
    assert!(config.allow_default_namespace());
}

#[test]
fn allow_default_namespace_requires_default_tenants() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig {
            allow_default: true,
            ..NamespaceConfig::default()
        },
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let error = config.validate().unwrap_err();
    assert!(
        error.to_string().contains("namespace.allow_default requires namespace.default_tenants")
    );
}

/// Verifies `schema_registry` rejects zero `max_schema_bytes`.
#[test]
fn schema_registry_rejects_zero_max_schema_bytes() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig {
            max_schema_bytes: 0,
            ..SchemaRegistryConfig::default()
        },
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
}

/// Verifies `schema_registry` rejects zero `max_entries`.
#[test]
fn schema_registry_rejects_zero_max_entries() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig {
            max_entries: Some(0),
            ..SchemaRegistryConfig::default()
        },
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Run State Store Validation Tests
// ============================================================================

/// Verifies sqlite `run_state_store` requires a path.
#[test]
fn run_state_store_sqlite_requires_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Sqlite,
            path: None,
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: None,
        },
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("run_state_store"));
}

/// Verifies memory `run_state_store` rejects a path.
#[test]
fn run_state_store_memory_rejects_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Memory,
            path: Some(PathBuf::from("store.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: None,
        },
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
}

/// Verifies sqlite `run_state_store` accepts a valid path.
#[test]
fn run_state_store_sqlite_accepts_path() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Sqlite,
            path: Some(PathBuf::from("store.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: Some(10),
        },
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_ok());
}

/// Verifies sqlite `run_state_store` rejects `max_versions` of zero.
#[test]
fn run_state_store_sqlite_rejects_zero_retention() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig {
            store_type: decision_gate_mcp::config::RunStateStoreType::Sqlite,
            path: Some(PathBuf::from("store.db")),
            busy_timeout_ms: 5_000,
            journal_mode: decision_gate_store_sqlite::SqliteStoreMode::Wal,
            sync_mode: decision_gate_store_sqlite::SqliteSyncMode::Full,
            max_versions: Some(0),
        },
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
}

// ============================================================================
// SECTION: Config Load Validation Tests
// ============================================================================

/// Verifies loading rejects MCP providers missing `capabilities_path`.
#[test]
fn config_load_rejects_mcp_without_capabilities_path() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("decision-gate.toml");
    let config = r#"
[server]
transport = "stdio"

[[providers]]
name = "echo"
type = "mcp"
command = ["echo-provider"]
"#;
    std::fs::write(&config_path, config.as_bytes()).unwrap();

    let result = DecisionGateConfig::load(Some(&config_path));
    let err = result.expect_err("expected missing capabilities_path rejection");
    assert!(err.to_string().contains("capabilities_path"));
}

/// Verifies loading accepts MCP providers with `capabilities_path`.
#[test]
fn config_load_accepts_mcp_with_capabilities_path() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    std::fs::write(&contract_path, "{}").unwrap();
    let config_path = temp.path().join("decision-gate.toml");
    let contract_path = contract_path.to_string_lossy().replace('\\', "/");
    let config = format!(
        r#"
[server]
transport = "stdio"

[[providers]]
name = "echo"
type = "mcp"
command = ["echo-provider"]
capabilities_path = "{contract_path}"
"#,
    );
    std::fs::write(&config_path, config.as_bytes()).unwrap();

    let result = DecisionGateConfig::load(Some(&config_path));
    assert!(result.is_ok());
}

/// Verifies MCP provider rejects HTTP without `allow_insecure` flag.
#[test]
fn provider_mcp_http_rejected_without_flag() {
    let config = ProviderConfig {
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
    };
    let result = validate_provider_config(config);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("insecure http"));
}

/// Verifies MCP provider allows HTTP with `allow_insecure` flag.
#[test]
fn provider_mcp_http_allowed_with_flag() {
    let config = ProviderConfig {
        name: "external".to_string(),
        provider_type: ProviderType::Mcp,
        command: Vec::new(),
        url: Some("http://localhost:8080/mcp".to_string()),
        allow_insecure_http: true,
        capabilities_path: Some(PathBuf::from("provider.json")),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    assert!(validate_provider_config(config).is_ok());
}

// ============================================================================
// SECTION: Default Value Tests
// ============================================================================

/// Verifies default server config uses stdio.
#[test]
fn default_server_is_stdio() {
    let config = ServerConfig::default();
    assert_eq!(config.transport, ServerTransport::Stdio);
}

/// Verifies default max body bytes is 1MB.
#[test]
fn default_max_body_bytes_is_1mb() {
    let config = ServerConfig::default();
    assert_eq!(config.max_body_bytes, 1024 * 1024);
}

/// Verifies default evidence policy redacts raw values.
#[test]
fn default_evidence_policy_redacts() {
    let config = EvidencePolicyConfig::default();
    assert!(!config.allow_raw_values);
    assert!(config.require_provider_opt_in);
}

// ============================================================================
// SECTION: Namespace Authority Validation Tests
// ============================================================================

/// Verifies assetcore authority mode requires an assetcore config block.
#[test]
fn namespace_authority_assetcore_requires_config() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    config.namespace.authority.mode =
        decision_gate_mcp::config::NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore = None;
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("namespace.authority.mode=assetcore_http"));
}

/// Verifies assetcore config is rejected when authority mode is none.
#[test]
fn namespace_authority_none_rejects_assetcore_config() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    config.namespace.authority.mode = decision_gate_mcp::config::NamespaceAuthorityMode::None;
    config.namespace.authority.assetcore =
        Some(decision_gate_mcp::config::AssetCoreNamespaceAuthorityConfig {
            base_url: "http://127.0.0.1:9000".to_string(),
            auth_token: None,
            connect_timeout_ms: 500,
            request_timeout_ms: 1_000,
        });
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("namespace.authority.assetcore"));
}

/// Verifies assetcore auth token rejects empty values.
#[test]
fn namespace_authority_assetcore_rejects_empty_token() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    config.namespace.authority.mode =
        decision_gate_mcp::config::NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore =
        Some(decision_gate_mcp::config::AssetCoreNamespaceAuthorityConfig {
            base_url: "https://assetcore.example.com".to_string(),
            auth_token: Some(" ".to_string()),
            connect_timeout_ms: 500,
            request_timeout_ms: 1_000,
        });
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("namespace.authority.assetcore.auth_token"));
}

/// Verifies dev-permissive is rejected when using assetcore namespace authority.
#[test]
fn dev_permissive_rejected_with_assetcore_authority() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig {
            permissive: true,
            ..decision_gate_mcp::config::DevConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    config.namespace.authority.mode =
        decision_gate_mcp::config::NamespaceAuthorityMode::AssetcoreHttp;
    config.namespace.authority.assetcore =
        Some(decision_gate_mcp::config::AssetCoreNamespaceAuthorityConfig {
            base_url: "http://127.0.0.1:9000".to_string(),
            auth_token: None,
            connect_timeout_ms: 500,
            request_timeout_ms: 1_000,
        });
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("dev.permissive not allowed when namespace.authority.mode=assetcore_http")
    );
}

// ============================================================================
// SECTION: Anchor Policy Validation Tests
// ============================================================================

/// Verifies anchor policy requires at least one required field per provider.
#[test]
fn anchors_require_required_fields() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig {
            providers: vec![AnchorProviderConfig {
                provider_id: "assetcore_read".to_string(),
                anchor_type: "assetcore.anchor_set".to_string(),
                required_fields: Vec::new(),
            }],
        },
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("anchors.providers.required_fields"));
}

// ========================================================================
// SECTION: Provider Discovery Validation Tests
// ========================================================================

#[test]
fn provider_discovery_rejects_empty_entries() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig {
            allowlist: vec![String::new()],
            denylist: Vec::new(),
            max_response_bytes: 1024,
        },
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("provider_discovery"));
}

#[test]
fn provider_discovery_rejects_zero_max_bytes() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig {
            allowlist: Vec::new(),
            denylist: Vec::new(),
            max_response_bytes: 0,
        },
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("provider_discovery.max_response_bytes"));
}

#[test]
fn runpack_storage_object_store_requires_bucket() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: " ".to_string(),
            region: None,
            endpoint: None,
            prefix: None,
            force_path_style: false,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    let err = config.validate().expect_err("invalid bucket");
    assert!(err.to_string().contains("bucket"));
}

#[test]
fn runpack_storage_object_store_rejects_http_without_allow() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: "runpacks".to_string(),
            region: None,
            endpoint: Some("http://localhost:9000".to_string()),
            prefix: None,
            force_path_style: true,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    let err = config.validate().expect_err("http endpoint without allow_http");
    assert!(err.to_string().contains("allow_http"));
}

#[test]
fn runpack_storage_object_store_accepts_https_endpoint() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: "runpacks".to_string(),
            region: None,
            endpoint: Some("https://s3.example.com".to_string()),
            prefix: Some("dg/runpacks".to_string()),
            force_path_style: false,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    config.validate().expect("valid object store config");
}

#[test]
fn runpack_storage_object_store_rejects_prefix_with_backslash() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: "runpacks".to_string(),
            region: None,
            endpoint: Some("https://s3.example.com".to_string()),
            prefix: Some("bad\\prefix".to_string()),
            force_path_style: false,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    let err = config.validate().expect_err("prefix contains backslash");
    assert!(err.to_string().contains("backslashes"));
}

#[test]
fn runpack_storage_object_store_rejects_prefix_traversal() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: "runpacks".to_string(),
            region: None,
            endpoint: Some("https://s3.example.com".to_string()),
            prefix: Some("../escape".to_string()),
            force_path_style: false,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    let err = config.validate().expect_err("prefix traversal");
    assert!(err.to_string().contains("traversal") || err.to_string().contains("segment"));
}

#[test]
fn runpack_storage_object_store_rejects_absolute_prefix() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: "runpacks".to_string(),
            region: None,
            endpoint: Some("https://s3.example.com".to_string()),
            prefix: Some("/absolute".to_string()),
            force_path_style: false,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    let err = config.validate().expect_err("absolute prefix");
    assert!(err.to_string().contains("relative"));
}

#[test]
fn runpack_storage_object_store_rejects_empty_prefix() {
    let mut config = DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: NamespaceConfig::default(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        docs: DocsConfig::default(),
        runpack_storage: Some(RunpackStorageConfig::ObjectStore(ObjectStoreConfig {
            provider: ObjectStoreProvider::S3,
            bucket: "runpacks".to_string(),
            region: None,
            endpoint: Some("https://s3.example.com".to_string()),
            prefix: Some("   ".to_string()),
            force_path_style: false,
            allow_http: false,
        })),
        source_modified_at: None,
    };
    let err = config.validate().expect_err("empty prefix");
    assert!(err.to_string().contains("prefix must be non-empty"));
}
