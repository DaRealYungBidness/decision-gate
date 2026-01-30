// decision-gate-config/src/schema.rs
// ============================================================================
// Module: Config Schemas
// Description: JSON schema builders for decision-gate.toml.
// Purpose: Provide canonical validation schema for config artifacts.
// Dependencies: serde_json
// ============================================================================

//! ## Overview
//! This module defines the JSON Schema for Decision Gate configuration.
//! The schema is generated from the canonical config model and is used by
//! tooling, docs, and validation pipelines.
//!
//! Security posture: schemas gate untrusted inputs; see
//! `Docs/security/threat_model.md`.

use decision_gate_core::ToolName;
use serde_json::Value;
use serde_json::json;

use crate::config::MAX_AUTH_SUBJECT_LENGTH;
use crate::config::MAX_AUTH_TOKEN_LENGTH;
use crate::config::MAX_AUTH_TOKENS;
use crate::config::MAX_AUTH_TOOL_RULES;
use crate::config::MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS;
use crate::config::MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS;
use crate::config::MAX_PRINCIPAL_ROLES;
use crate::config::MAX_PROVIDER_CONNECT_TIMEOUT_MS;
use crate::config::MAX_PROVIDER_REQUEST_TIMEOUT_MS;
use crate::config::MAX_RATE_LIMIT_ENTRIES;
use crate::config::MAX_RATE_LIMIT_REQUESTS;
use crate::config::MAX_RATE_LIMIT_WINDOW_MS;
use crate::config::MAX_REGISTRY_ACL_RULES;
use crate::config::MAX_SCHEMA_MAX_BYTES;
use crate::config::MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS;
use crate::config::MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS;
use crate::config::MIN_PROVIDER_CONNECT_TIMEOUT_MS;
use crate::config::MIN_PROVIDER_REQUEST_TIMEOUT_MS;
use crate::config::MIN_RATE_LIMIT_WINDOW_MS;
use crate::config::default_audit_enabled;
use crate::config::default_dev_permissive_exempt_providers;
use crate::config::default_dev_permissive_warn;
use crate::config::default_max_body_bytes;
use crate::config::default_max_inflight;
use crate::config::default_provider_connect_timeout_ms;
use crate::config::default_provider_discovery_max_bytes;
use crate::config::default_provider_request_timeout_ms;
use crate::config::default_rate_limit_max_entries;
use crate::config::default_rate_limit_max_requests;
use crate::config::default_rate_limit_window_ms;
use crate::config::default_require_provider_opt_in;
use crate::config::default_schema_max_bytes;
use crate::config::default_store_busy_timeout_ms;
use crate::config::default_tls_require_client_cert;
use crate::config::default_validation_strict;

/// Returns the JSON schema for `decision-gate.toml`.
#[must_use]
pub fn config_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "decision-gate://contract/schemas/config.schema.json",
        "title": "Decision Gate MCP Configuration",
        "description": "Configuration for the Decision Gate MCP server and providers.",
        "type": "object",
        "properties": {
            "server": server_config_schema(),
            "namespace": namespace_config_schema(),
            "dev": dev_config_schema(),
            "trust": trust_config_schema(),
            "evidence": evidence_policy_schema(),
            "anchors": anchor_policy_schema(),
            "provider_discovery": provider_discovery_config_schema(),
            "validation": validation_config_schema(),
            "policy": policy_config_schema(),
            "run_state_store": run_state_store_schema(),
            "schema_registry": schema_registry_config_schema(),
            "providers": {
                "type": "array",
                "items": provider_config_schema(),
                "default": []
            },
            "runpack_storage": {
                "oneOf": [
                    { "type": "null" },
                    runpack_storage_schema()
                ],
                "default": null
            }
        },
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Server Configuration
// ============================================================================

/// Schema for the server configuration section.
fn server_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "transport": {
                "type": "string",
                "enum": ["stdio", "http", "sse"],
                "default": "stdio",
                "description": "Transport protocol for MCP."
            },
            "mode": {
                "type": "string",
                "enum": ["strict", "dev_permissive"],
                "default": "strict",
                "description": "Operational mode for MCP (dev_permissive is legacy)."
            },
            "bind": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Bind address for HTTP/SSE transport.")
                ],
                "default": null
            },
            "max_body_bytes": {
                "type": "integer",
                "minimum": 1,
                "default": default_max_body_bytes(),
                "description": "Maximum JSON-RPC request size in bytes."
            },
            "limits": server_limits_schema(),
            "auth": nullable_schema(&server_auth_schema()),
            "tls": nullable_schema(&server_tls_schema()),
            "audit": server_audit_schema()
        },
        "allOf": [
            {
                "if": {
                    "properties": {
                        "transport": { "enum": ["http", "sse"] }
                    }
                },
                "then": {
                    "required": ["bind"],
                    "properties": {
                        "bind": schema_for_non_empty_string("Bind address for HTTP/SSE transport.")
                    }
                }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for server limits.
fn server_limits_schema() -> Value {
    json!({
        "type": "object",
        "description": "Request limits for MCP server.",
        "properties": {
            "max_inflight": {
                "type": "integer",
                "minimum": 1,
                "default": default_max_inflight(),
                "description": "Maximum concurrent MCP requests."
            },
            "rate_limit": {
                "oneOf": [
                    { "type": "null" },
                    rate_limit_schema()
                ],
                "default": null,
                "description": "Optional rate limit configuration."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for rate limit settings.
fn rate_limit_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "max_requests": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAX_RATE_LIMIT_REQUESTS,
                "default": default_rate_limit_max_requests(),
                "description": "Maximum requests per rate limit window."
            },
            "window_ms": {
                "type": "integer",
                "minimum": MIN_RATE_LIMIT_WINDOW_MS,
                "maximum": MAX_RATE_LIMIT_WINDOW_MS,
                "default": default_rate_limit_window_ms(),
                "description": "Rate limit window in milliseconds."
            },
            "max_entries": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAX_RATE_LIMIT_ENTRIES,
                "default": default_rate_limit_max_entries(),
                "description": "Maximum distinct rate limit entries."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for inbound authentication configuration.
fn server_auth_schema() -> Value {
    json!({
        "type": "object",
        "description": "Inbound authentication configuration for MCP tool calls.",
        "properties": {
            "mode": {
                "type": "string",
                "enum": ["local_only", "bearer_token", "mtls"],
                "default": "local_only",
                "description": "Inbound auth mode for MCP tool calls."
            },
            "bearer_tokens": {
                "type": "array",
                "items": schema_for_bearer_token("Bearer token value."),
                "maxItems": MAX_AUTH_TOKENS,
                "default": [],
                "description": "Allowed bearer tokens."
            },
            "mtls_subjects": {
                "type": "array",
                "items": schema_for_mtls_subject("mTLS subject string."),
                "maxItems": MAX_AUTH_TOKENS,
                "default": [],
                "description": "Allowed mTLS subjects (via trusted proxy header)."
            },
            "allowed_tools": {
                "type": "array",
                "items": tool_name_schema(),
                "maxItems": MAX_AUTH_TOOL_RULES,
                "default": [],
                "description": "Optional tool allowlist for inbound calls."
            },
            "principals": {
                "type": "array",
                "items": principal_schema(),
                "maxItems": MAX_AUTH_TOKENS,
                "default": [],
                "description": "Optional principal-to-role mappings."
            }
        },
        "allOf": [
            {
                "if": { "properties": { "mode": { "const": "bearer_token" } } },
                "then": { "required": ["bearer_tokens"], "properties": { "bearer_tokens": { "minItems": 1 } } }
            },
            {
                "if": { "properties": { "mode": { "const": "mtls" } } },
                "then": { "required": ["mtls_subjects"], "properties": { "mtls_subjects": { "minItems": 1 } } }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for principal mappings in auth configuration.
fn principal_schema() -> Value {
    json!({
        "type": "object",
        "required": ["subject"],
        "properties": {
            "subject": schema_for_non_empty_string("Principal identifier (subject or token fingerprint)."),
            "policy_class": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Policy class label.")
                ],
                "default": null
            },
            "roles": {
                "type": "array",
                "items": principal_role_schema(),
                "maxItems": MAX_PRINCIPAL_ROLES,
                "default": [],
                "description": "Role bindings for this principal."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for role bindings attached to principals.
fn principal_role_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": schema_for_non_empty_string("Role name."),
            "tenant_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_numeric_identifier("Tenant identifier scope.")
                ],
                "default": null
            },
            "namespace_id": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_numeric_identifier("Namespace identifier scope.")
                ],
                "default": null
            }
        },
        "additionalProperties": false
    })
}

/// Schema for TLS configuration settings.
fn server_tls_schema() -> Value {
    json!({
        "type": "object",
        "description": "TLS configuration for HTTP/SSE transports.",
        "properties": {
            "cert_path": schema_for_non_empty_string("Server TLS certificate (PEM)."),
            "key_path": schema_for_non_empty_string("Server TLS private key (PEM)."),
            "client_ca_path": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Optional client CA bundle for mTLS.")
                ],
                "default": null
            },
            "require_client_cert": {
                "type": "boolean",
                "default": default_tls_require_client_cert(),
                "description": "Require client certificate for mTLS."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for audit logging configuration.
fn server_audit_schema() -> Value {
    json!({
        "type": "object",
        "description": "Structured audit logging configuration.",
        "properties": {
            "enabled": {
                "type": "boolean",
                "default": default_audit_enabled(),
                "description": "Enable structured audit logging (JSON lines)."
            },
            "path": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Audit log path (JSON lines).")
                ],
                "default": null
            },
            "log_precheck_payloads": {
                "type": "boolean",
                "default": false,
                "description": "Log raw precheck payloads (explicit opt-in)."
            }
        },
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Dev / Namespace / Trust
// ============================================================================

/// Schema for development overrides.
fn dev_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Development-mode overrides (explicit opt-in).",
        "properties": {
            "permissive": {
                "type": "boolean",
                "default": false,
                "description": "Enable dev-permissive mode (explicit opt-in)."
            },
            "permissive_scope": {
                "type": "string",
                "enum": ["asserted_evidence_only"],
                "default": "asserted_evidence_only",
                "description": "Dev-permissive scope selection."
            },
            "permissive_ttl_days": {
                "oneOf": [
                    { "type": "null" },
                    { "type": "integer", "minimum": 1 }
                ],
                "default": null,
                "description": "Optional TTL for dev-permissive warnings (days)."
            },
            "permissive_warn": {
                "type": "boolean",
                "default": default_dev_permissive_warn(),
                "description": "Emit warnings when dev-permissive enabled/expired."
            },
            "permissive_exempt_providers": {
                "type": "array",
                "items": schema_for_non_empty_string("Provider ID exempt from dev-permissive."),
                "default": default_dev_permissive_exempt_providers(),
                "description": "Providers exempt from dev-permissive relaxations."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for namespace configuration.
fn namespace_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Namespace policy configuration.",
        "properties": {
            "allow_default": {
                "type": "boolean",
                "default": false,
                "description": "Allow the default namespace ID (1)."
            },
            "default_tenants": {
                "type": "array",
                "items": schema_for_numeric_identifier("Tenant identifier allowed for default namespace."),
                "default": [],
                "description": "Tenant allowlist required when allow_default is true."
            },
            "authority": namespace_authority_schema()
        },
        "additionalProperties": false
    })
}

/// Schema for namespace authority configuration.
fn namespace_authority_schema() -> Value {
    json!({
        "type": "object",
        "description": "Namespace authority backend selection.",
        "properties": {
            "mode": {
                "type": "string",
                "enum": ["none", "assetcore_http"],
                "default": "none",
                "description": "Namespace authority backend selection."
            },
            "assetcore": {
                "oneOf": [
                    { "type": "null" },
                    assetcore_authority_schema()
                ]
            }
        },
        "allOf": [
            {
                "if": { "properties": { "mode": { "const": "assetcore_http" } } },
                "then": { "required": ["assetcore"] }
            },
            {
                "if": { "properties": { "mode": { "const": "none" } } },
                "then": { "properties": { "assetcore": { "type": "null" } } }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for Asset Core authority settings.
fn assetcore_authority_schema() -> Value {
    json!({
        "type": "object",
        "description": "Asset Core namespace authority settings.",
        "required": ["base_url"],
        "properties": {
            "base_url": schema_for_non_empty_string("Asset Core write-daemon base URL."),
            "auth_token": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Optional bearer token for namespace lookup.")
                ],
                "default": null
            },
            "connect_timeout_ms": {
                "type": "integer",
                "minimum": MIN_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
                "maximum": MAX_NAMESPACE_AUTH_CONNECT_TIMEOUT_MS,
                "default": crate::config::default_namespace_auth_connect_timeout_ms(),
                "description": "HTTP connect timeout (ms)."
            },
            "request_timeout_ms": {
                "type": "integer",
                "minimum": MIN_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
                "maximum": MAX_NAMESPACE_AUTH_REQUEST_TIMEOUT_MS,
                "default": crate::config::default_namespace_auth_request_timeout_ms(),
                "description": "HTTP request timeout (ms)."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for trust policy defaults.
fn trust_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Trust policy defaults for providers.",
        "properties": {
            "default_policy": trust_policy_schema(),
            "min_lane": trust_lane_schema()
        },
        "additionalProperties": false
    })
}

/// Schema for trust policy variants.
fn trust_policy_schema() -> Value {
    json!({
        "description": "Default trust policy for providers.",
        "oneOf": [
            { "type": "string", "enum": ["audit"], "default": "audit" },
            {
                "type": "object",
                "required": ["require_signature"],
                "properties": {
                    "require_signature": {
                        "type": "object",
                        "required": ["keys"],
                        "properties": {
                            "keys": schema_for_non_empty_string_array("Signature key identifiers.")
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            }
        ]
    })
}

/// Schema for trust lane selection.
fn trust_lane_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["verified", "asserted"],
        "default": "verified",
        "description": "Minimum evidence trust lane accepted."
    })
}

// ============================================================================
// SECTION: Evidence / Anchors / Discovery
// ============================================================================

/// Schema for evidence disclosure defaults.
fn evidence_policy_schema() -> Value {
    json!({
        "type": "object",
        "description": "Evidence disclosure policy defaults.",
        "properties": {
            "allow_raw_values": {
                "type": "boolean",
                "default": false,
                "description": "Allow raw evidence values to be disclosed."
            },
            "require_provider_opt_in": {
                "type": "boolean",
                "default": default_require_provider_opt_in(),
                "description": "Require provider opt-in for raw disclosure."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for provider discovery configuration.
fn provider_discovery_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Provider discovery allow/deny policy.",
        "properties": {
            "allowlist": {
                "type": "array",
                "items": schema_for_non_empty_string("Provider identifier allowed for disclosure."),
                "default": [],
                "description": "Optional allowlist for provider disclosure."
            },
            "denylist": {
                "type": "array",
                "items": schema_for_non_empty_string("Provider identifier denied for disclosure."),
                "default": [],
                "description": "Provider identifiers denied for disclosure."
            },
            "max_response_bytes": {
                "type": "integer",
                "minimum": 1,
                "default": default_provider_discovery_max_bytes(),
                "description": "Maximum response size for provider discovery tools."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for evidence anchor policy.
fn anchor_policy_schema() -> Value {
    json!({
        "type": "object",
        "description": "Evidence anchor policy configuration.",
        "properties": {
            "providers": {
                "type": "array",
                "items": anchor_provider_schema(),
                "default": [],
                "description": "Provider-specific anchor requirements."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for anchor provider requirements.
fn anchor_provider_schema() -> Value {
    json!({
        "type": "object",
        "required": ["provider_id", "anchor_type", "required_fields"],
        "properties": {
            "provider_id": schema_for_non_empty_string("Provider identifier requiring anchors."),
            "anchor_type": schema_for_non_empty_string("Anchor type identifier expected in results."),
            "required_fields": schema_for_required_string_array("Required fields in anchor_value.")
        },
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Validation / Policy
// ============================================================================

/// Schema for comparator validation configuration.
fn validation_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Comparator validation configuration.",
        "properties": {
            "strict": {
                "type": "boolean",
                "default": default_validation_strict(),
                "description": "Enforce strict comparator validation."
            },
            "profile": {
                "type": "string",
                "enum": ["strict_core_v1"],
                "default": "strict_core_v1",
                "description": "Strict comparator profile identifier."
            },
            "allow_permissive": {
                "type": "boolean",
                "default": false,
                "description": "Explicit opt-in for permissive validation."
            },
            "enable_lexicographic": {
                "type": "boolean",
                "default": false,
                "description": "Enable lexicographic comparators (opt-in per schema)."
            },
            "enable_deep_equals": {
                "type": "boolean",
                "default": false,
                "description": "Enable deep equality comparators (opt-in per schema)."
            }
        },
        "allOf": [
            {
                "if": { "properties": { "strict": { "const": false } } },
                "then": { "properties": { "allow_permissive": { "const": true } } }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for policy engine configuration.
fn policy_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Dispatch policy engine configuration.",
        "properties": {
            "engine": policy_engine_schema(),
            "static": {
                "oneOf": [
                    { "type": "null" },
                    static_policy_schema()
                ],
                "default": null
            }
        },
        "allOf": [
            {
                "if": { "properties": { "engine": { "const": "static" } } },
                "then": { "required": ["static"] }
            },
            {
                "if": { "properties": { "engine": { "enum": ["permit_all", "deny_all"] } } },
                "then": { "properties": { "static": { "type": "null" } } }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for policy engine selector values.
fn policy_engine_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["permit_all", "deny_all", "static"],
        "default": "permit_all",
        "description": "Dispatch policy engine selection."
    })
}

/// Schema for static policy configuration.
fn static_policy_schema() -> Value {
    json!({
        "type": "object",
        "description": "Static dispatch policy rules.",
        "properties": {
            "default": {
                "type": "string",
                "enum": ["permit", "deny"],
                "default": "deny",
                "description": "Default decision when no rules match."
            },
            "rules": {
                "type": "array",
                "items": policy_rule_schema(),
                "default": [],
                "description": "Ordered list of static policy rules."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for static policy rules.
fn policy_rule_schema() -> Value {
    json!({
        "type": "object",
        "required": ["effect"],
        "properties": {
            "effect": {
                "type": "string",
                "enum": ["permit", "deny", "error"],
                "description": "Rule effect."
            },
            "error_message": schema_for_string("Error message when effect is 'error'."),
            "target_kinds": {
                "type": "array",
                "items": dispatch_target_kind_schema(),
                "default": [],
                "description": "Target kinds that may receive the packet."
            },
            "targets": {
                "type": "array",
                "items": policy_target_schema(),
                "default": [],
                "description": "Specific target selectors."
            },
            "require_labels": schema_for_string_array("Visibility labels required to match."),
            "forbid_labels": schema_for_string_array("Visibility labels that block a match."),
            "require_policy_tags": schema_for_string_array("Policy tags required to match."),
            "forbid_policy_tags": schema_for_string_array("Policy tags that block a match."),
            "content_types": schema_for_string_array("Allowed content types."),
            "schema_ids": schema_for_string_array("Allowed schema identifiers."),
            "packet_ids": schema_for_string_array("Allowed packet identifiers."),
            "stage_ids": schema_for_string_array("Allowed stage identifiers."),
            "scenario_ids": schema_for_string_array("Allowed scenario identifiers.")
        },
        "allOf": [
            {
                "if": { "properties": { "effect": { "const": "error" } } },
                "then": { "required": ["error_message"] }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for dispatch target kinds.
fn dispatch_target_kind_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["agent", "session", "external", "channel"],
        "description": "Dispatch target kind."
    })
}

/// Schema for policy target selectors.
fn policy_target_schema() -> Value {
    json!({
        "type": "object",
        "required": ["target_kind"],
        "properties": {
            "target_kind": dispatch_target_kind_schema(),
            "target_id": schema_for_string("Target identifier for agent/session/channel."),
            "system": schema_for_string("External system name."),
            "target": schema_for_string("External target identifier.")
        },
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Stores / Registry
// ============================================================================

/// Schema for run state store settings.
fn run_state_store_schema() -> Value {
    json!({
        "type": "object",
        "description": "Run state store configuration.",
        "properties": {
            "type": {
                "type": "string",
                "enum": ["memory", "sqlite"],
                "default": "memory",
                "description": "Run state store backend selection."
            },
            "path": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("SQLite database path.")
                ],
                "default": null
            },
            "busy_timeout_ms": {
                "type": "integer",
                "minimum": 0,
                "default": default_store_busy_timeout_ms(),
                "description": "SQLite busy timeout (ms)."
            },
            "journal_mode": {
                "type": "string",
                "enum": ["wal", "delete"],
                "default": "wal",
                "description": "SQLite journal mode."
            },
            "sync_mode": {
                "type": "string",
                "enum": ["full", "normal"],
                "default": "full",
                "description": "SQLite sync mode."
            },
            "max_versions": {
                "oneOf": [
                    { "type": "null" },
                    { "type": "integer", "minimum": 1 }
                ],
                "default": null,
                "description": "Optional max versions retained per run."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for schema registry configuration.
fn schema_registry_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Schema registry configuration.",
        "properties": {
            "type": {
                "type": "string",
                "enum": ["memory", "sqlite"],
                "default": "memory",
                "description": "Schema registry backend selection."
            },
            "path": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("SQLite database path.")
                ],
                "default": null
            },
            "busy_timeout_ms": {
                "type": "integer",
                "minimum": 0,
                "default": default_store_busy_timeout_ms(),
                "description": "SQLite busy timeout (ms)."
            },
            "journal_mode": {
                "type": "string",
                "enum": ["wal", "delete"],
                "default": "wal",
                "description": "SQLite journal mode."
            },
            "sync_mode": {
                "type": "string",
                "enum": ["full", "normal"],
                "default": "full",
                "description": "SQLite sync mode."
            },
            "max_schema_bytes": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAX_SCHEMA_MAX_BYTES,
                "default": default_schema_max_bytes(),
                "description": "Maximum schema payload size in bytes."
            },
            "max_entries": {
                "oneOf": [
                    { "type": "null" },
                    { "type": "integer", "minimum": 1 }
                ],
                "default": null,
                "description": "Optional max schemas per tenant + namespace."
            },
            "acl": schema_registry_acl_schema()
        },
        "additionalProperties": false
    })
}

/// Schema for schema registry ACL configuration.
fn schema_registry_acl_schema() -> Value {
    json!({
        "type": "object",
        "description": "Schema registry ACL configuration.",
        "properties": {
            "mode": {
                "type": "string",
                "enum": ["builtin", "custom"],
                "default": "builtin",
                "description": "Built-in role rules or custom ACL rules."
            },
            "default": {
                "type": "string",
                "enum": ["deny", "allow"],
                "default": "deny",
                "description": "Default decision when no rules match (custom only)."
            },
            "require_signing": {
                "type": "boolean",
                "default": false,
                "description": "Require schema signing metadata on writes."
            },
            "rules": {
                "type": "array",
                "items": schema_registry_acl_rule_schema(),
                "maxItems": MAX_REGISTRY_ACL_RULES,
                "default": [],
                "description": "Custom ACL rules (mode = custom)."
            }
        },
        "additionalProperties": false
    })
}

/// Schema for schema registry ACL rules.
fn schema_registry_acl_rule_schema() -> Value {
    json!({
        "type": "object",
        "required": ["effect"],
        "properties": {
            "effect": {
                "type": "string",
                "enum": ["allow", "deny"],
                "description": "Rule effect."
            },
            "actions": {
                "type": "array",
                "items": {
                    "type": "string",
                    "enum": ["register", "list", "get"]
                },
                "default": [],
                "description": "Registry actions covered by the rule."
            },
            "tenants": {
                "type": "array",
                "items": schema_for_numeric_identifier("Tenant identifier."),
                "description": "Tenant identifier scope."
            },
            "namespaces": {
                "type": "array",
                "items": schema_for_numeric_identifier("Namespace identifier."),
                "description": "Namespace identifier scope."
            },
            "subjects": {
                "type": "array",
                "items": schema_for_non_empty_string("Principal subject."),
                "description": "Principal subjects in scope."
            },
            "roles": {
                "type": "array",
                "items": schema_for_non_empty_string("Role name."),
                "description": "Role names in scope."
            },
            "policy_classes": {
                "type": "array",
                "items": schema_for_non_empty_string("Policy class label."),
                "description": "Policy class labels in scope."
            }
        },
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Providers
// ============================================================================

/// Schema for provider registry entries.
fn provider_config_schema() -> Value {
    json!({
        "type": "object",
        "description": "Provider configuration entry.",
        "required": ["name", "type"],
        "properties": {
            "name": schema_for_non_empty_string("Provider identifier."),
            "type": {
                "type": "string",
                "enum": ["builtin", "mcp"],
                "description": "Provider kind."
            },
            "command": {
                "type": "array",
                "items": { "type": "string" },
                "default": []
            },
            "url": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Provider HTTP URL.")
                ],
                "default": null
            },
            "allow_insecure_http": {
                "type": "boolean",
                "default": false,
                "description": "Allow http:// URLs for MCP providers."
            },
            "capabilities_path": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Path to provider capability contract JSON.")
                ],
                "default": null
            },
            "auth": {
                "oneOf": [
                    { "type": "null" },
                    provider_auth_schema()
                ],
                "default": null
            },
            "trust": {
                "oneOf": [
                    { "type": "null" },
                    trust_policy_schema()
                ],
                "default": null
            },
            "allow_raw": {
                "type": "boolean",
                "default": false,
                "description": "Allow raw evidence disclosure for this provider."
            },
            "timeouts": provider_timeouts_schema(),
            "config": schema_for_json_value("Provider-specific config blob.")
        },
        "allOf": [
            {
                "if": {
                    "properties": {
                        "type": { "const": "mcp" }
                    }
                },
                "then": {
                    "anyOf": [
                        { "required": ["command"] },
                        { "required": ["url"] }
                    ],
                    "required": ["capabilities_path"]
                }
            },
            {
                "if": {
                    "properties": {
                        "url": { "type": "string", "pattern": "^http://" }
                    }
                },
                "then": {
                    "properties": { "allow_insecure_http": { "const": true } }
                }
            }
        ],
        "additionalProperties": false
    })
}

/// Schema for provider authentication settings.
fn provider_auth_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "bearer_token": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Bearer token for MCP providers.")
                ],
                "default": null
            }
        },
        "additionalProperties": false
    })
}

/// Schema for provider timeout overrides.
fn provider_timeouts_schema() -> Value {
    json!({
        "type": "object",
        "description": "HTTP timeout overrides for MCP providers.",
        "properties": {
            "connect_timeout_ms": {
                "type": "integer",
                "minimum": MIN_PROVIDER_CONNECT_TIMEOUT_MS,
                "maximum": MAX_PROVIDER_CONNECT_TIMEOUT_MS,
                "default": default_provider_connect_timeout_ms(),
                "description": "TCP/TLS connect timeout (ms)."
            },
            "request_timeout_ms": {
                "type": "integer",
                "minimum": MIN_PROVIDER_REQUEST_TIMEOUT_MS,
                "maximum": MAX_PROVIDER_REQUEST_TIMEOUT_MS,
                "default": default_provider_request_timeout_ms(),
                "description": "Total request timeout (ms)."
            }
        },
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Runpack Storage
// ============================================================================

/// Schema for runpack storage configuration.
fn runpack_storage_schema() -> Value {
    json!({
        "type": "object",
        "description": "Runpack storage configuration.",
        "required": ["type", "provider", "bucket"],
        "properties": {
            "type": {
                "type": "string",
                "enum": ["object_store"],
                "description": "Runpack storage backend selection."
            },
            "provider": {
                "type": "string",
                "enum": ["s3"],
                "description": "Object-store provider."
            },
            "bucket": schema_for_non_empty_string("Bucket name for runpack storage."),
            "region": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Optional S3 region override.")
                ],
                "default": null
            },
            "endpoint": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Optional S3-compatible endpoint.")
                ],
                "default": null
            },
            "prefix": {
                "oneOf": [
                    { "type": "null" },
                    schema_for_non_empty_string("Optional key prefix inside the bucket.")
                ],
                "default": null
            },
            "force_path_style": {
                "type": "boolean",
                "default": false,
                "description": "Force path-style addressing (S3-compatible)."
            },
            "allow_http": {
                "type": "boolean",
                "default": false,
                "description": "Allow non-TLS endpoints (explicit opt-in)."
            }
        },
        "allOf": [
            {
                "if": {
                    "properties": {
                        "endpoint": { "type": "string", "pattern": "^http://" }
                    }
                },
                "then": {
                    "properties": { "allow_http": { "const": true } }
                }
            }
        ],
        "additionalProperties": false
    })
}

// ============================================================================
// SECTION: Schema Helpers
// ============================================================================

/// Schema for a required, non-empty string.
fn schema_for_non_empty_string(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description
    })
}

/// Schema for a bearer token string.
fn schema_for_bearer_token(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_AUTH_TOKEN_LENGTH,
        "description": description
    })
}

/// Schema for an mTLS subject string.
fn schema_for_mtls_subject(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_AUTH_SUBJECT_LENGTH,
        "description": description
    })
}

/// Schema for an arbitrary string.
fn schema_for_string(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

/// Schema for a positive numeric identifier.
fn schema_for_numeric_identifier(description: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "description": description
    })
}

/// Schema for a string array (empty allowed).
fn schema_for_string_array(description: &str) -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" },
        "description": description
    })
}

/// Schema for an array of non-empty strings.
fn schema_for_non_empty_string_array(description: &str) -> Value {
    json!({
        "type": "array",
        "items": schema_for_non_empty_string(description),
        "description": description
    })
}

/// Schema for a required, non-empty string array.
fn schema_for_required_string_array(description: &str) -> Value {
    json!({
        "type": "array",
        "items": schema_for_non_empty_string(description),
        "minItems": 1,
        "description": description
    })
}

/// Schema for arbitrary JSON values.
fn schema_for_json_value(description: &str) -> Value {
    json!({
        "type": ["null", "boolean", "number", "string", "array", "object"],
        "description": description
    })
}

/// Wraps a schema in a nullable `oneOf` construct.
fn nullable_schema(schema: &Value) -> Value {
    json!({
        "oneOf": [
            { "type": "null" },
            schema
        ],
        "default": null
    })
}

/// Schema for known tool names.
fn tool_name_schema() -> Value {
    let names: Vec<&str> = ToolName::all().iter().map(|tool| tool.as_str()).collect();
    json!({
        "type": "string",
        "enum": names
    })
}
