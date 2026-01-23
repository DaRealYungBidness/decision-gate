// decision-gate-cli/src/i18n.rs
// ============================================================================
// Module: CLI Internationalization Helpers
// Description: Provides message catalog and translation utilities for the CLI.
// Purpose: Centralize user-facing strings for future localization support.
// Dependencies: Standard library collections and formatting utilities.
// ============================================================================

//! ## Overview
//! The Decision Gate CLI stores user-facing strings in a small translation
//! catalog to enforce consistent messaging and to prepare for future locales.
//! All runtime output should be routed through the [`t!`](crate::t) macro.
//!
//! ## Invariants
//! - The catalog is initialized once and read-only thereafter.
//! - Missing keys fall back to the key itself to avoid panics.
//! - Placeholder substitutions preserve deterministic order.

use std::collections::HashMap;
use std::sync::OnceLock;

/// A formatted message argument captured by the [`macro@crate::t`] macro.
#[derive(Clone)]
pub struct MessageArg {
    /// The placeholder name used in message templates (e.g., `"path"`).
    pub key: &'static str,
    /// The formatted string value to substitute for this placeholder.
    pub value: String,
}

impl MessageArg {
    /// Constructs a new [`MessageArg`] from a key and displayable value.
    #[allow(clippy::needless_pass_by_value, reason = "ownership avoids extra lifetimes")]
    pub fn new(key: &'static str, value: impl ToString) -> Self {
        Self {
            key,
            value: value.to_string(),
        }
    }
}

/// Translates `key` using the English fallback catalog while substituting `args`.
#[must_use]
pub fn translate(key: &str, args: Vec<MessageArg>) -> String {
    let template = catalog().get(key).copied().unwrap_or(key);
    if args.is_empty() {
        return template.to_string();
    }

    let mut result = template.to_string();
    for arg in args {
        let placeholder = format!("{{{}}}", arg.key);
        result = result.replace(&placeholder, &arg.value);
    }
    result
}

/// Returns the static English catalog used by the CLI.
#[allow(clippy::too_many_lines, reason = "catalog kept centralized for auditability")]
fn catalog() -> &'static HashMap<&'static str, &'static str> {
    static CATALOG: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

    CATALOG.get_or_init(|| {
        HashMap::from([
            ("main.version", "decision-gate {version}"),
            (
                "serve.warn.local_only",
                "Warning: decision-gate is running in local-only, no-auth mode. Do not expose \
                 this service to the network.",
            ),
            (
                "serve.warn.transport_local_only",
                "Warning: HTTP/SSE transports are supported for loopback only until auth/policy \
                 enforcement is implemented.",
            ),
            ("output.stream.stdout", "stdout"),
            ("output.stream.stderr", "stderr"),
            ("output.stream.unknown", "output"),
            ("output.write_failed", "Failed to write to {stream}: {error}"),
            ("serve.config.load_failed", "Failed to load config: {error}"),
            ("serve.bind.parse_failed", "Invalid bind address {bind}: {error}"),
            (
                "serve.bind.non_loopback",
                "Refusing to bind to non-loopback address {bind} (auth/policy not configured).",
            ),
            ("serve.init_failed", "Failed to initialize MCP server: {error}"),
            ("serve.failed", "MCP server failed: {error}"),
            ("runpack.export.read_failed", "Failed to read {kind} file at {path}: {error}"),
            ("runpack.export.parse_failed", "Failed to parse {kind} JSON at {path}: {error}"),
            (
                "runpack.export.output_dir_failed",
                "Failed to create output directory {path}: {error}",
            ),
            ("runpack.export.sink_failed", "Failed to initialize runpack sink at {path}: {error}"),
            ("runpack.export.build_failed", "Failed to build runpack: {error}"),
            ("runpack.export.ok", "Runpack manifest written to {path}"),
            ("runpack.export.verification_status", "Verification status: {status}"),
            ("runpack.export.kind.spec", "scenario spec"),
            ("runpack.export.kind.state", "run state"),
            (
                "runpack.export.time.system_failed",
                "Failed to read system time for runpack generation: {error}",
            ),
            ("runpack.export.time.overflow", "System time is out of range for runpack generation."),
            ("runpack.verify.read_failed", "Failed to read runpack manifest at {path}: {error}"),
            ("runpack.verify.parse_failed", "Failed to parse runpack manifest at {path}: {error}"),
            ("runpack.verify.reader_failed", "Failed to open runpack directory {path}: {error}"),
            ("runpack.verify.failed", "Failed to verify runpack: {error}"),
            ("runpack.verify.status.pass", "pass"),
            ("runpack.verify.status.fail", "fail"),
            ("runpack.verify.md.header", "# Decision Gate Runpack Verification"),
            ("runpack.verify.md.status", "- Status: {status}"),
            ("runpack.verify.md.checked", "- Checked files: {count}"),
            ("runpack.verify.md.errors_header", "## Errors"),
            ("runpack.verify.md.error_line", "- {error}"),
            ("runpack.verify.md.no_errors", "- None"),
            ("authoring.read_failed", "Failed to read authoring input at {path}: {error}"),
            (
                "authoring.format.missing",
                "Unable to determine authoring format for {path}; specify --format.",
            ),
            ("authoring.parse_failed", "Failed to parse {format} input at {path}: {error}"),
            ("authoring.schema_failed", "Schema validation failed for {path}: {error}"),
            (
                "authoring.deserialize_failed",
                "Failed to deserialize ScenarioSpec from {path}: {error}",
            ),
            ("authoring.spec_failed", "ScenarioSpec validation failed for {path}: {error}"),
            (
                "authoring.canonicalize_failed",
                "Failed to canonicalize ScenarioSpec from {path}: {error}",
            ),
            (
                "authoring.normalize.write_failed",
                "Failed to write normalized output to {path}: {error}",
            ),
            ("authoring.normalize.ok", "Normalized scenario written to {path}"),
            (
                "authoring.validate.ok",
                "ScenarioSpec valid (scenario_id={scenario_id}, spec_hash={spec_hash})",
            ),
        ])
    })
}

/// Formats a localized message from a key and named arguments.
///
/// # Arguments
///
/// - `$key` must match a catalog entry.
/// - Named arguments are substituted into `{placeholder}` positions.
///
/// # Returns
///
/// A localized [`String`] with placeholders substituted.
#[macro_export]
macro_rules! t {
    ($key:literal $(, $name:ident = $value:expr )* $(,)?) => {{
        let args = ::std::vec![
            $(
                $crate::i18n::MessageArg::new(stringify!($name), &$value),
            )*
        ];
        $crate::i18n::translate($key, args)
    }};
}
