// decision-gate-config/src/docs.rs
// ============================================================================
// Module: Config Docs Generator
// Description: Markdown generator for decision-gate.toml documentation.
// Purpose: Keep config docs in sync with schema and validation.
// Dependencies: serde_json, std
// ============================================================================

//! ## Overview
//! Generates `Docs/configuration/decision-gate.toml.md` from the canonical
//! configuration schema. This output is deterministic and used by the website.
//!
//! Security posture: docs must reflect fail-closed defaults; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

use crate::schema::config_schema;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Default output path for generated configuration docs.
const DOCS_PATH: &str = "Docs/configuration/decision-gate.toml.md";

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Errors raised when generating or verifying config docs.
#[derive(Debug, Error)]
pub enum DocsError {
    /// IO failure while writing docs.
    #[error("docs io error: {0}")]
    Io(String),
    /// Schema traversal or rendering error.
    #[error("docs schema error: {0}")]
    Schema(String),
    /// Generated docs do not match the committed file.
    #[error("docs drift: {0}")]
    Drift(String),
}

// ============================================================================
// SECTION: Public API
// ============================================================================

/// Generates the configuration markdown documentation.
///
/// # Errors
///
/// Returns [`DocsError`] when schema traversal fails.
pub fn config_docs_markdown() -> Result<String, DocsError> {
    let schema = config_schema();
    let mut out = String::new();

    out.push_str("<!--\n");
    out.push_str("Docs/configuration/decision-gate.toml.md\n");
    out.push_str("============================================================================\n");
    out.push_str("Document: Decision Gate MCP Configuration\n");
    out.push_str("Description: Reference for decision-gate.toml configuration fields.\n");
    out.push_str("Purpose: Document server, trust, evidence, and provider settings.\n");
    out.push_str("Generated: This file is auto-generated; do not edit manually.\n");
    out.push_str("============================================================================\n");
    out.push_str("-->\n\n");

    out.push_str("# decision-gate.toml Configuration\n\n");
    out.push_str("## Overview\n\n");
    out.push_str("`decision-gate.toml` configures the MCP server, trust policies, evidence\n");
    out.push_str("disclosure defaults, and provider registry. All inputs are validated and\n");
    out.push_str("fail closed on errors.\n\n");

    out.push_str("## Top-Level Sections\n\n");

    let sections = build_sections();
    for section in sections {
        out.push_str("### ");
        out.push_str(section.heading);
        out.push_str("\n\n");
        if !section.description.is_empty() {
            out.push_str(section.description);
            out.push_str("\n\n");
        }
        let table = render_table(&schema, &section).map_err(DocsError::Schema)?;
        out.push_str(&table);
        if let Some(extra) = section.extra {
            out.push('\n');
            out.push_str(extra);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("## Built-In Provider Config\n\n");
    out.push_str("Built-in providers accept optional `config` blocks:\n\n");
    out.push_str("- `time`:\n  - `allow_logical` (bool, default true)\n");
    out.push_str(
        "- `env`:\n  - `allowlist` (array)\n  - `denylist` (array)\n  - `max_value_bytes` \
         (integer)\n  - `max_key_bytes` (integer)\n  - `overrides` (table)\n",
    );
    out.push_str(
        "- `json`:\n  - `root` (string)\n  - `root_id` (string)\n  - `max_bytes` (integer)\n  - \
         `allow_yaml` (bool)\n",
    );
    out.push_str(
        "- `http`:\n  - `allow_http` (bool)\n  - `timeout_ms` (integer)\n  - `max_response_bytes` \
         (integer)\n  - `allowed_hosts` (array)\n  - `user_agent` (string)\n  - `hash_algorithm` \
         (string)\n",
    );

    Ok(out)
}

/// Writes the generated docs to the standard location.
///
/// # Errors
///
/// Returns [`DocsError`] when file output fails.
pub fn write_config_docs(path: Option<&Path>) -> Result<(), DocsError> {
    let path = path.unwrap_or_else(|| Path::new(DOCS_PATH));
    let content = config_docs_markdown()?;
    fs::write(path, content.as_bytes()).map_err(|err| DocsError::Io(err.to_string()))
}

/// Verifies the on-disk docs match the generated output.
///
/// # Errors
///
/// Returns [`DocsError`] when the docs drift.
pub fn verify_config_docs(path: Option<&Path>) -> Result<(), DocsError> {
    let path = path.unwrap_or_else(|| Path::new(DOCS_PATH));
    let content = config_docs_markdown()?;
    let existing = fs::read_to_string(path).map_err(|err| DocsError::Io(err.to_string()))?;
    if existing != content {
        return Err(DocsError::Drift(format!("docs mismatch: {}", path.display())));
    }
    Ok(())
}

// ============================================================================
// SECTION: Section Specs
// ============================================================================

/// Specification for one rendered documentation section.
#[derive(Clone)]
struct SectionSpec {
    /// Section heading, including TOML table name.
    heading: &'static str,
    /// Section description displayed beneath the heading.
    description: &'static str,
    /// Schema traversal path used to resolve the section.
    path: &'static [SchemaPath],
    /// Ordered field list rendered in the docs table.
    fields: &'static [&'static str],
    /// Whether to include a "Required" column.
    include_required: bool,
    /// Default values that override schema defaults for docs.
    default_overrides: &'static [FieldOverride],
    /// Optional additional text appended after the table.
    extra: Option<&'static str>,
}

/// Overrides for schema defaults shown in docs tables.
#[derive(Clone, Copy)]
struct FieldOverride {
    /// Field name to override.
    field: &'static str,
    /// Replacement default value string.
    default_value: &'static str,
}

/// Path segment for resolving nested schema properties.
#[derive(Clone, Copy)]
enum SchemaPath {
    /// Descend into an object property.
    Property(&'static str),
    /// Descend into an array items schema.
    Items,
}

// ============================================================================
// SECTION: Section Registry
// ============================================================================

/// Builds the ordered list of configuration sections to render.
#[allow(
    clippy::too_many_lines,
    reason = "Keeping the full section list inline makes the config table spec auditable."
)]
fn build_sections() -> Vec<SectionSpec> {
    vec![
        SectionSpec {
            heading: "[server]",
            description: "Server transport, auth, limits, and audit settings.",
            path: &[SchemaPath::Property("server")],
            fields: &[
                "transport",
                "mode",
                "tls_termination",
                "bind",
                "max_body_bytes",
                "limits",
                "auth",
                "tls",
                "audit",
                "feedback",
                "tools",
            ],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "limits", default_value: "{ max_inflight = 256 }" },
                FieldOverride { field: "auth", default_value: "null" },
                FieldOverride { field: "tls", default_value: "null" },
                FieldOverride { field: "audit", default_value: "{ enabled = true }" },
                FieldOverride {
                    field: "tools",
                    default_value: "{ mode = \"filter\", allowlist = [], denylist = [] }",
                },
            ],
            extra: Some(
                "HTTP/SSE require `bind`; non-loopback requires explicit CLI opt-in plus TLS \
or `tls_termination = \"upstream\"` + non-local auth.",
            ),
        },
        SectionSpec {
            heading: "[server.auth]",
            description: "Inbound authn/authz for MCP tool calls.",
            path: &[SchemaPath::Property("server"), SchemaPath::Property("auth")],
            fields: &["mode", "bearer_tokens", "mtls_subjects", "allowed_tools", "principals"],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "bearer_tokens", default_value: "[]" },
                FieldOverride { field: "mtls_subjects", default_value: "[]" },
                FieldOverride { field: "allowed_tools", default_value: "[]" },
                FieldOverride { field: "principals", default_value: "[]" },
            ],
            extra: Some(
                "Bearer token example:\n\n```toml\n[server.auth]\nmode = \"bearer_token\"\nbearer_tokens = [\"token-1\", \"token-2\"]\nallowed_tools = [\"scenario_define\", \"scenario_start\", \"scenario_next\"]\n```\n\nmTLS subject example (via trusted proxy header):\n\n```toml\n[server.auth]\nmode = \"mtls\"\nmtls_subjects = [\"CN=decision-gate-client,O=Example Corp\"]\n```\n\nWhen using `mtls` mode, the server expects the `x-decision-gate-client-subject` header from a trusted TLS-terminating proxy.\n\nPrincipal mapping example (registry ACL):\n\n```toml\n[[server.auth.principals]]\nsubject = \"loopback\"\npolicy_class = \"prod\"\n\n[[server.auth.principals.roles]]\nname = \"TenantAdmin\"\ntenant_id = 1\nnamespace_id = 1\n```\n\nBuilt-in registry ACL expects `policy_class` values like `prod`, `project`, or `scratch` (case-insensitive). Unknown values are treated as `prod`.",
            ),
        },
        SectionSpec {
            heading: "[server.audit]",
            description: "Structured audit logging configuration.",
            path: &[SchemaPath::Property("server"), SchemaPath::Property("audit")],
            fields: &["enabled", "path", "log_precheck_payloads"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "path", default_value: "null" }],
            extra: None,
        },
        SectionSpec {
            heading: "[server.feedback]",
            description: "Feedback disclosure controls for tool responses.",
            path: &[SchemaPath::Property("server"), SchemaPath::Property("feedback")],
            fields: &["scenario_next"],
            include_required: false,
            default_overrides: &[FieldOverride {
                field: "scenario_next",
                default_value: "{ default = \"summary\", local_only_default = \"trace\", max = \"trace\" }",
            }],
            extra: Some(
                "Feedback levels: `summary` (unmet gates only), `trace` (gate + condition status), `evidence` (includes evidence records, subject to disclosure policy).",
            ),
        },
        SectionSpec {
            heading: "[server.feedback.scenario_next]",
            description: "Feedback policy for scenario_next responses.",
            path: &[
                SchemaPath::Property("server"),
                SchemaPath::Property("feedback"),
                SchemaPath::Property("scenario_next"),
            ],
            fields: &[
                "default",
                "local_only_default",
                "max",
                "trace_subjects",
                "trace_roles",
                "evidence_subjects",
                "evidence_roles",
            ],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "trace_roles", default_value: "[]" },
                FieldOverride { field: "evidence_subjects", default_value: "[]" },
                FieldOverride { field: "evidence_roles", default_value: "[]" },
            ],
            extra: Some(
                "Local-only defaults apply to loopback/stdio. Subjects and roles are resolved from `server.auth.principals`.",
            ),
        },
        SectionSpec {
            heading: "[server.tools]",
            description: "Tool visibility configuration for tools/list output.",
            path: &[SchemaPath::Property("server"), SchemaPath::Property("tools")],
            fields: &["mode", "allowlist", "denylist"],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "allowlist", default_value: "[]" },
                FieldOverride { field: "denylist", default_value: "[]" },
            ],
            extra: Some(
                "Visibility is separate from auth: hidden tools are omitted from tools/list and treated as unknown when called.",
            ),
        },
        SectionSpec {
            heading: "[server.limits]",
            description: "Request concurrency and rate limits.",
            path: &[SchemaPath::Property("server"), SchemaPath::Property("limits")],
            fields: &["max_inflight", "rate_limit"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "rate_limit", default_value: "null" }],
            extra: None,
        },
        SectionSpec {
            heading: "[server.limits.rate_limit]",
            description: "Optional token-bucket style rate limit configuration.",
            path: &[
                SchemaPath::Property("server"),
                SchemaPath::Property("limits"),
                SchemaPath::Property("rate_limit"),
            ],
            fields: &["max_requests", "window_ms", "max_entries"],
            include_required: false,
            default_overrides: &[],
            extra: None,
        },
        SectionSpec {
            heading: "[server.tls]",
            description: "TLS configuration for HTTP/SSE transports.",
            path: &[SchemaPath::Property("server"), SchemaPath::Property("tls")],
            fields: &["cert_path", "key_path", "client_ca_path", "require_client_cert"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "client_ca_path", default_value: "null" }],
            extra: None,
        },
        SectionSpec {
            heading: "[dev]",
            description: "Explicit dev-permissive overrides (opt-in only).",
            path: &[SchemaPath::Property("dev")],
            fields: &[
                "permissive",
                "permissive_scope",
                "permissive_ttl_days",
                "permissive_warn",
                "permissive_exempt_providers",
            ],
            include_required: false,
            default_overrides: &[FieldOverride {
                field: "permissive_ttl_days",
                default_value: "null",
            }],
            extra: Some(
                "Dev-permissive is rejected when `namespace.authority.mode = \"assetcore_http\"`.",
            ),
        },
        SectionSpec {
            heading: "[namespace]",
            description: "Namespace allowlist and authority selection.",
            path: &[SchemaPath::Property("namespace")],
            fields: &["allow_default", "default_tenants", "authority"],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "default_tenants", default_value: "[]" },
                FieldOverride { field: "authority", default_value: "{ mode = \"none\" }" },
            ],
            extra: None,
        },
        SectionSpec {
            heading: "[namespace.authority]",
            description: "Namespace authority backend configuration.",
            path: &[SchemaPath::Property("namespace"), SchemaPath::Property("authority")],
            fields: &["mode", "assetcore"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "assetcore", default_value: "null" }],
            extra: None,
        },
        SectionSpec {
            heading: "[namespace.authority.assetcore]",
            description: "Asset Core namespace authority settings.",
            path: &[
                SchemaPath::Property("namespace"),
                SchemaPath::Property("authority"),
                SchemaPath::Property("assetcore"),
            ],
            fields: &["base_url", "auth_token", "connect_timeout_ms", "request_timeout_ms"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "auth_token", default_value: "null" }],
            extra: Some(
                "Asset Core authority example:\n\n```toml\n[namespace.authority]\nmode = \"assetcore_http\"\n\n[namespace.authority.assetcore]\nbase_url = \"http://127.0.0.1:9001\"\nauth_token = \"token\"\nconnect_timeout_ms = 500\nrequest_timeout_ms = 2000\n```",
            ),
        },
        SectionSpec {
            heading: "[trust]",
            description: "Trust lane defaults and provider signature enforcement.",
            path: &[SchemaPath::Property("trust")],
            fields: &["default_policy", "min_lane"],
            include_required: false,
            default_overrides: &[],
            extra: Some(
                "`require_signature` form:\n\n```toml\n[trust]\ndefault_policy = { require_signature = { keys = [\"key1.pub\"] } }\n```",
            ),
        },
        SectionSpec {
            heading: "[evidence]",
            description: "Evidence disclosure policy defaults.",
            path: &[SchemaPath::Property("evidence")],
            fields: &["allow_raw_values", "require_provider_opt_in"],
            include_required: false,
            default_overrides: &[],
            extra: None,
        },
        SectionSpec {
            heading: "[provider_discovery]",
            description: "Provider contract/schema disclosure controls.",
            path: &[SchemaPath::Property("provider_discovery")],
            fields: &["allowlist", "denylist", "max_response_bytes"],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "allowlist", default_value: "[]" },
                FieldOverride { field: "denylist", default_value: "[]" },
            ],
            extra: None,
        },
        SectionSpec {
            heading: "[anchors]",
            description: "Evidence anchor policy configuration.",
            path: &[SchemaPath::Property("anchors")],
            fields: &["providers"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "providers", default_value: "[]" }],
            extra: None,
        },
        SectionSpec {
            heading: "[[anchors.providers]]",
            description: "Provider-specific anchor requirements.",
            path: &[
                SchemaPath::Property("anchors"),
                SchemaPath::Property("providers"),
                SchemaPath::Items,
            ],
            fields: &["provider_id", "anchor_type", "required_fields"],
            include_required: true,
            default_overrides: &[],
            extra: Some(
                "Anchor policy example (Asset Core):\n\n```toml\n[anchors]\n[[anchors.providers]]\nprovider_id = \"assetcore_read\"\nanchor_type = \"assetcore.anchor_set\"\nrequired_fields = [\"assetcore.namespace_id\", \"assetcore.commit_id\", \"assetcore.world_seq\"]\n```",
            ),
        },
        SectionSpec {
            heading: "[policy]",
            description: "Dispatch policy engine selection.",
            path: &[SchemaPath::Property("policy")],
            fields: &["engine", "static"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "static", default_value: "null" }],
            extra: Some(
                "Static policy example:\n\n```toml\n[policy]\nengine = \"static\"\n\n[policy.static]\ndefault = \"deny\"\n\n[[policy.static.rules]]\neffect = \"permit\"\ntarget_kinds = [\"agent\"]\nrequire_labels = [\"public\"]\n```",
            ),
        },
        SectionSpec {
            heading: "[policy.static]",
            description: "Static dispatch policy rules.",
            path: &[SchemaPath::Property("policy"), SchemaPath::Property("static")],
            fields: &["default", "rules"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "rules", default_value: "[]" }],
            extra: None,
        },
        SectionSpec {
            heading: "[[policy.static.rules]]",
            description: "Static policy rule fields.",
            path: &[
                SchemaPath::Property("policy"),
                SchemaPath::Property("static"),
                SchemaPath::Property("rules"),
                SchemaPath::Items,
            ],
            fields: &[
                "effect",
                "error_message",
                "target_kinds",
                "targets",
                "require_labels",
                "forbid_labels",
                "require_policy_tags",
                "forbid_policy_tags",
                "content_types",
                "schema_ids",
                "packet_ids",
                "stage_ids",
                "scenario_ids",
            ],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "error_message", default_value: "null" },
                FieldOverride { field: "target_kinds", default_value: "[]" },
                FieldOverride { field: "targets", default_value: "[]" },
                FieldOverride { field: "require_labels", default_value: "[]" },
                FieldOverride { field: "forbid_labels", default_value: "[]" },
                FieldOverride { field: "require_policy_tags", default_value: "[]" },
                FieldOverride { field: "forbid_policy_tags", default_value: "[]" },
                FieldOverride { field: "content_types", default_value: "[]" },
                FieldOverride { field: "schema_ids", default_value: "[]" },
                FieldOverride { field: "packet_ids", default_value: "[]" },
                FieldOverride { field: "stage_ids", default_value: "[]" },
                FieldOverride { field: "scenario_ids", default_value: "[]" },
            ],
            extra: Some(
                "Target selector fields (`policy.static.rules.targets`):\n\n| Field | Type | Notes |\n| --- | --- | --- |\n| `target_kind` | \"agent\" \\| \"session\" \\| \"external\" \\| \"channel\" | Target kind. |\n| `target_id` | string | Agent/session/channel identifier. |\n| `system` | string | External system name (external only). |\n| `target` | string | External target identifier (external only). |",
            ),
        },
        SectionSpec {
            heading: "[validation]",
            description: "Comparator validation policy for scenarios and prechecks.",
            path: &[SchemaPath::Property("validation")],
            fields: &[
                "strict",
                "profile",
                "allow_permissive",
                "enable_lexicographic",
                "enable_deep_equals",
            ],
            include_required: false,
            default_overrides: &[],
            extra: Some(
                "Strict validation (default):\n\n```toml\n[validation]\nstrict = true\nprofile = \"strict_core_v1\"\n```\n\nPermissive validation (explicit opt-in):\n\n```toml\n[validation]\nstrict = false\nallow_permissive = true\n```\n\nOptional comparator families:\n\n```toml\n[validation]\nenable_lexicographic = true\nenable_deep_equals = true\n```",
            ),
        },
        SectionSpec {
            heading: "[runpack_storage]",
            description: "Runpack storage configuration.",
            path: &[SchemaPath::Property("runpack_storage")],
            fields: &[
                "type",
                "provider",
                "bucket",
                "region",
                "endpoint",
                "prefix",
                "force_path_style",
                "allow_http",
            ],
            include_required: true,
            default_overrides: &[
                FieldOverride { field: "region", default_value: "null" },
                FieldOverride { field: "endpoint", default_value: "null" },
                FieldOverride { field: "prefix", default_value: "null" },
            ],
            extra: None,
        },
        SectionSpec {
            heading: "[run_state_store]",
            description: "Run state persistence settings.",
            path: &[SchemaPath::Property("run_state_store")],
            fields: &[
                "type",
                "path",
                "busy_timeout_ms",
                "journal_mode",
                "sync_mode",
                "max_versions",
            ],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "path", default_value: "null" },
                FieldOverride { field: "max_versions", default_value: "null" },
            ],
            extra: Some(
                "SQLite example:\n\n```toml\n[run_state_store]\ntype = \"sqlite\"\npath = \"decision-gate.db\"\njournal_mode = \"wal\"\nsync_mode = \"full\"\nbusy_timeout_ms = 5000\nmax_versions = 1000\n```",
            ),
        },
        SectionSpec {
            heading: "[schema_registry]",
            description: "Schema registry persistence and limits.",
            path: &[SchemaPath::Property("schema_registry")],
            fields: &[
                "type",
                "path",
                "busy_timeout_ms",
                "journal_mode",
                "sync_mode",
                "max_schema_bytes",
                "max_entries",
                "acl",
            ],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "path", default_value: "null" },
                FieldOverride { field: "max_entries", default_value: "null" },
                FieldOverride { field: "acl", default_value: "{ mode = \"builtin\" }" },
            ],
            extra: None,
        },
        SectionSpec {
            heading: "[schema_registry.acl]",
            description: "Schema registry ACL configuration.",
            path: &[SchemaPath::Property("schema_registry"), SchemaPath::Property("acl")],
            fields: &["mode", "default", "allow_local_only", "require_signing", "rules"],
            include_required: false,
            default_overrides: &[FieldOverride { field: "rules", default_value: "[]" }],
            extra: Some(
                "Built-in ACL relies on `server.auth.principals` for role and policy_class resolution. Without principals, registry access defaults to deny unless `allow_local_only` is enabled (loopback/stdio only). Enable `allow_local_only` for dev-only convenience; it bypasses principal mapping for local-only callers.\n\nCustom ACL example:\n\n```toml\n[schema_registry.acl]\nmode = \"custom\"\ndefault = \"deny\"\n\n[[schema_registry.acl.rules]]\neffect = \"allow\"\nactions = [\"register\", \"list\", \"get\"]\ntenants = [1]\nnamespaces = [1]\nroles = [\"TenantAdmin\", \"NamespaceAdmin\"]\n```",
            ),
        },
        SectionSpec {
            heading: "[[schema_registry.acl.rules]]",
            description: "Custom ACL rule fields.",
            path: &[
                SchemaPath::Property("schema_registry"),
                SchemaPath::Property("acl"),
                SchemaPath::Property("rules"),
                SchemaPath::Items,
            ],
            fields: &[
                "effect",
                "actions",
                "tenants",
                "namespaces",
                "subjects",
                "roles",
                "policy_classes",
            ],
            include_required: false,
            default_overrides: &[
                FieldOverride { field: "actions", default_value: "[]" },
                FieldOverride { field: "tenants", default_value: "[]" },
                FieldOverride { field: "namespaces", default_value: "[]" },
                FieldOverride { field: "subjects", default_value: "[]" },
                FieldOverride { field: "roles", default_value: "[]" },
                FieldOverride { field: "policy_classes", default_value: "[]" },
            ],
            extra: None,
        },
        SectionSpec {
            heading: "[docs]",
            description: "Documentation search and resources configuration.",
            path: &[SchemaPath::Property("docs")],
            fields: &[
                "enabled",
                "enable_search",
                "enable_resources",
                "include_default_docs",
                "extra_paths",
                "max_doc_bytes",
                "max_total_bytes",
                "max_docs",
                "max_sections",
            ],
            include_required: false,
            default_overrides: &[FieldOverride { field: "extra_paths", default_value: "[]" }],
            extra: Some(
                "Docs search and resources are deterministic and local-only by default. Use extra_paths to ingest local markdown files or directories.",
            ),
        },
        SectionSpec {
            heading: "[[providers]]",
            description: "Provider entries register built-in or MCP providers.",
            path: &[SchemaPath::Property("providers"), SchemaPath::Items],
            fields: &[
                "name",
                "type",
                "command",
                "url",
                "allow_insecure_http",
                "capabilities_path",
                "auth",
                "trust",
                "allow_raw",
                "timeouts",
                "config",
            ],
            include_required: true,
            default_overrides: &[
                FieldOverride { field: "command", default_value: "[]" },
                FieldOverride { field: "url", default_value: "null" },
                FieldOverride { field: "allow_insecure_http", default_value: "false" },
                FieldOverride { field: "capabilities_path", default_value: "null" },
                FieldOverride { field: "auth", default_value: "null" },
                FieldOverride { field: "trust", default_value: "null" },
                FieldOverride { field: "allow_raw", default_value: "false" },
                FieldOverride {
                    field: "timeouts",
                    default_value: "{ connect_timeout_ms = 2000, request_timeout_ms = 10000 }",
                },
                FieldOverride { field: "config", default_value: "null" },
            ],
            extra: Some(
                "`auth` form:\n\n```toml\nauth = { bearer_token = \"token\" }\n```\n\n`trust` override form:\n\n```toml\ntrust = { require_signature = { keys = [\"provider.pub\"] } }\n```\n\n`capabilities_path` example for MCP providers:\n\n```toml\n[[providers]]\nname = \"mongo\"\ntype = \"mcp\"\ncommand = [\"mongo-provider\", \"--stdio\"]\ncapabilities_path = \"contracts/mongo_provider.json\"\n```\n\n`timeouts` form (HTTP MCP providers):\n\n```toml\ntimeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }\n```\n\nHTTP provider example with timeouts:\n\n```toml\n[[providers]]\nname = \"ci\"\ntype = \"mcp\"\nurl = \"https://ci.example.com/rpc\"\ncapabilities_path = \"contracts/ci_provider.json\"\ntimeouts = { connect_timeout_ms = 2000, request_timeout_ms = 10000 }\n```\n\nTimeout constraints:\n\n- `connect_timeout_ms` must be between 100 and 10000.\n- `request_timeout_ms` must be between 500 and 30000 and >= `connect_timeout_ms`.",
            ),
        },
        SectionSpec {
            heading: "[providers.timeouts]",
            description: "Timeout overrides for HTTP MCP providers.",
            path: &[
                SchemaPath::Property("providers"),
                SchemaPath::Items,
                SchemaPath::Property("timeouts"),
            ],
            fields: &["connect_timeout_ms", "request_timeout_ms"],
            include_required: false,
            default_overrides: &[],
            extra: None,
        },
    ]
}

// ============================================================================
// SECTION: Rendering Helpers
// ============================================================================

/// Renders the markdown table for a configuration section.
fn render_table(schema: &Value, section: &SectionSpec) -> Result<String, String> {
    let section_schema = schema_at(schema, section.path)?;
    let props = section_schema
        .get("properties")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "schema properties missing".to_string())?;

    let mut seen = BTreeSet::new();
    for field in section.fields {
        if !props.contains_key(*field) {
            return Err(format!("missing field in schema: {field}"));
        }
        seen.insert(*field);
    }
    for key in props.keys() {
        if !seen.contains(key.as_str()) {
            return Err(format!("field not documented: {key}"));
        }
    }

    let required = section_schema
        .get("required")
        .and_then(|value| value.as_array())
        .map(|arr| arr.iter().filter_map(|val| val.as_str()).collect::<Vec<&str>>())
        .unwrap_or_default();

    let overrides = overrides_map(section.default_overrides);

    let mut table = String::new();
    if section.include_required {
        table.push_str("| Field | Type | Required | Default | Notes |\n");
        table.push_str("| --- | --- | --- | --- | --- |\n");
    } else {
        table.push_str("| Field | Type | Default | Notes |\n");
        table.push_str("| --- | --- | --- | --- |\n");
    }

    for field in section.fields {
        let raw_schema =
            props.get(*field).ok_or_else(|| format!("missing field schema: {field}"))?;
        let prop_schema = unwrap_nullable(raw_schema);
        let field_type = format_schema_type(prop_schema);
        let default_value = overrides
            .get(*field)
            .map(|value| (*value).to_string())
            .or_else(|| raw_schema.get("default").map(format_default_value))
            .or_else(|| prop_schema.get("default").map(format_default_value))
            .unwrap_or_else(|| "n/a".to_string());
        let notes = raw_schema
            .get("description")
            .and_then(|value| value.as_str())
            .or_else(|| prop_schema.get("description").and_then(|value| value.as_str()))
            .unwrap_or("");

        if section.include_required {
            let required_value = if required.contains(field) { "yes" } else { "no" };
            let _ = writeln!(
                &mut table,
                "| `{field}` | {field_type} | {required_value} | {default_value} | {notes} |"
            );
        } else {
            let _ =
                writeln!(&mut table, "| `{field}` | {field_type} | {default_value} | {notes} |");
        }
    }

    Ok(table)
}

/// Builds a lookup table for default overrides.
fn overrides_map(overrides: &[FieldOverride]) -> BTreeMap<&str, &str> {
    let mut map = BTreeMap::new();
    for override_entry in overrides {
        map.insert(override_entry.field, override_entry.default_value);
    }
    map
}

/// Resolves a schema node by walking a path of properties/items.
fn schema_at<'a>(schema: &'a Value, path: &[SchemaPath]) -> Result<&'a Value, String> {
    let mut current = schema;
    for segment in path {
        current = match segment {
            SchemaPath::Property(name) => {
                let props = current
                    .get("properties")
                    .and_then(|value| value.as_object())
                    .ok_or_else(|| format!("properties missing while seeking {name}"))?;
                let prop = props.get(*name).ok_or_else(|| format!("property not found: {name}"))?;
                unwrap_nullable(prop)
            }
            SchemaPath::Items => current
                .get("items")
                .map(unwrap_nullable)
                .ok_or_else(|| "array items missing".to_string())?,
        };
    }
    Ok(current)
}

/// Returns the non-null branch of a nullable `oneOf` schema.
fn unwrap_nullable(schema: &Value) -> &Value {
    if let Some(one_of) = schema.get("oneOf").and_then(|val| val.as_array())
        && one_of.len() == 2
        && let Some(other) =
            one_of.iter().find(|item| item.get("type").and_then(|val| val.as_str()) != Some("null"))
    {
        return other;
    }
    schema
}

/// Formats a schema type for markdown tables.
fn format_schema_type(schema: &Value) -> String {
    let raw = format_schema_type_raw(schema);
    escape_table_cell(&raw)
}

/// Formats a schema type without markdown escaping.
fn format_schema_type_raw(schema: &Value) -> String {
    if let Some(one_of) = schema.get("oneOf").and_then(|val| val.as_array()) {
        let mut types = one_of
            .iter()
            .filter(|item| item.get("type").and_then(|val| val.as_str()) != Some("null"))
            .map(format_schema_type_raw)
            .collect::<Vec<String>>();
        if types.len() == 1 {
            let mut only = types.remove(0);
            only.push_str(" | null");
            return only;
        }
    }
    if let Some(enum_vals) = schema.get("enum").and_then(|val| val.as_array()) {
        let items = enum_vals.iter().map(format_enum_value).collect::<Vec<String>>();
        return items.join(" | ");
    }
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            return match type_str {
                "string" => "string".to_string(),
                "integer" => "integer".to_string(),
                "number" => "number".to_string(),
                "boolean" => "bool".to_string(),
                "array" => "array".to_string(),
                "object" => "table".to_string(),
                _ => type_str.to_string(),
            };
        }
        if let Some(type_arr) = type_val.as_array() {
            let types = type_arr.iter().filter_map(|val| val.as_str()).collect::<Vec<&str>>();
            if types.len() > 2 {
                return "json".to_string();
            }
            return types.join(" | ");
        }
    }
    "unknown".to_string()
}

/// Escapes pipe characters for markdown table cells.
fn escape_table_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

/// Formats enum values as TOML-compatible strings.
fn format_enum_value(value: &Value) -> String {
    value.as_str().map_or_else(|| value.to_string(), |text| format!("\"{text}\""))
}

/// Formats schema defaults for display in docs.
fn format_default_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(val) => val.to_string(),
        Value::Number(val) => val.to_string(),
        Value::String(val) => val.clone(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                let items = arr.iter().map(format_enum_value).collect::<Vec<String>>();
                format!("[{}]", items.join(", "))
            }
        }
        Value::Object(_) => "{...}".to_string(),
    }
}
