// crates/decision-gate-cli/src/i18n.rs
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
//! - Missing keys fall back to English and then to the key itself.
//! - Placeholder substitutions preserve deterministic order.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::HashMap;
use std::sync::OnceLock;

// ============================================================================
// SECTION: Types
// ============================================================================

/// Supported CLI locales.
///
/// # Invariants
/// - Variants are stable for CLI parsing and catalog lookup.
/// - [`Locale::En`] is the default fallback locale.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Locale {
    /// English (default).
    En,
    /// Catalan.
    Ca,
}

impl Locale {
    /// Returns the canonical locale label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ca => "ca",
        }
    }

    /// Attempts to parse a locale value (case-insensitive, tolerant of region tags).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let normalized = value.to_ascii_lowercase();
        let lang = normalized.split(['-', '_']).next().unwrap_or("");
        match lang {
            "en" => Some(Self::En),
            "ca" => Some(Self::Ca),
            _ => None,
        }
    }
}

/// Ordered list of supported CLI locales.
///
/// # Invariants
/// - Ordering is stable for deterministic presentation.
pub const SUPPORTED_LOCALES: &[Locale] = &[Locale::En, Locale::Ca];

/// A formatted message argument captured by the [`macro@crate::t`] macro.
///
/// # Invariants
/// - `key` matches a placeholder name without braces (for example, `path`).
/// - `value` is preformatted and should be safe for display.
#[derive(Clone)]
pub struct MessageArg {
    /// The placeholder name used in message templates (e.g., `"path"`).
    pub key: &'static str,
    /// The formatted string value to substitute for this placeholder.
    pub value: String,
}

impl MessageArg {
    /// Constructs a new [`MessageArg`] from a key and displayable value.
    pub fn new(key: &'static str, value: impl Into<String>) -> Self {
        Self {
            key,
            value: value.into(),
        }
    }
}

// ============================================================================
// SECTION: Locale Selection
// ============================================================================

/// Global locale selection for CLI output.
static CURRENT_LOCALE: OnceLock<Locale> = OnceLock::new();

/// Sets the CLI locale. Only the first call wins.
pub fn set_locale(locale: Locale) {
    let _ = CURRENT_LOCALE.set(locale);
}

/// Returns the current CLI locale (defaults to English).
#[must_use]
pub fn current_locale() -> Locale {
    CURRENT_LOCALE.get().copied().unwrap_or(Locale::En)
}

// ============================================================================
// SECTION: Catalog
// ============================================================================

/// Static English catalog entries loaded into the localized message bundle.
const CATALOG_EN: &[(&str, &str)] = &[
    ("main.version", "decision-gate {version}"),
    (
        "serve.warn.local_only_auth",
        "Info: running in local-only mode (stdio + loopback HTTP/SSE only). Configure server.auth \
         for bearer_token or mtls when exposing beyond localhost.",
    ),
    (
        "serve.warn.loopback_only_transport",
        "Info: HTTP/SSE is bound to loopback only. Use --allow-non-loopback or {env}=1 with TLS \
         (or tls_termination=upstream) + auth to expose it.",
    ),
    ("output.stream.stdout", "stdout"),
    ("output.stream.stderr", "stderr"),
    ("output.stream.unknown", "output"),
    ("output.write_failed", "Failed to write to {stream}: {error}"),
    (
        "input.read_too_large",
        "Refusing to read {kind} at {path} because it is {size} bytes (limit {limit}).",
    ),
    ("config.load_failed", "Failed to load config: {error}"),
    ("config.validate.ok", "Config valid."),
    ("serve.config.load_failed", "Failed to load config: {error}"),
    ("serve.bind.parse_failed", "Invalid bind address {bind}: {error}"),
    (
        "serve.bind.non_loopback_opt_in",
        "Refusing to bind to non-loopback address {bind}. Set --allow-non-loopback or {env}=1 to \
         opt in.",
    ),
    (
        "serve.bind.non_loopback_auth_required",
        "Refusing to bind to {bind}: server.auth.mode must be bearer_token or mtls for \
         non-loopback.",
    ),
    (
        "serve.bind.non_loopback_tls_required",
        "Refusing to bind to {bind}: configure server.tls or set server.tls_termination=upstream.",
    ),
    (
        "serve.bind.non_loopback_mtls_client_ca_required",
        "Refusing to bind to {bind}: mTLS requires tls.client_ca_path.",
    ),
    (
        "serve.bind.non_loopback_mtls_client_cert_required",
        "Refusing to bind to {bind}: mTLS requires tls.require_client_cert=true.",
    ),
    (
        "serve.bind.allow_env_invalid",
        "Invalid value for {env}: {value}. Expected true/false/1/0/yes/no/on/off.",
    ),
    ("serve.warn.network.header", "SECURITY WARNING: Decision Gate is exposed on the network."),
    ("serve.warn.network.bind", "Bind: {bind}"),
    ("serve.warn.network.auth", "Auth mode: {mode}"),
    ("serve.warn.network.tls", "TLS: {tls}"),
    ("serve.warn.network.audit", "Audit logging: {status}"),
    ("serve.warn.network.rate_limit", "Rate limiting: {status}"),
    (
        "serve.warn.network.footer",
        "Verify firewall rules and credentials; this exposure is intentional.",
    ),
    ("serve.warn.network.enabled", "enabled"),
    ("serve.warn.network.disabled", "disabled"),
    (
        "serve.warn.network.tls_enabled",
        "enabled (client cert {client_cert}, client CA {client_ca})",
    ),
    ("serve.warn.network.tls_disabled", "disabled"),
    ("serve.warn.network.tls_upstream", "upstream (terminated by proxy/ingress)"),
    ("serve.warn.network.required", "required"),
    ("serve.warn.network.not_required", "not required"),
    ("serve.warn.network.present", "present"),
    ("serve.warn.network.missing", "missing"),
    ("serve.init_failed", "Failed to initialize MCP server: {error}"),
    ("serve.failed", "MCP server failed: {error}"),
    ("runpack.export.read_failed", "Failed to read {kind} file at {path}: {error}"),
    ("runpack.export.parse_failed", "Failed to parse {kind} JSON at {path}: {error}"),
    ("runpack.export.spec_failed", "ScenarioSpec validation failed for {path}: {error}"),
    ("runpack.export.output_dir_failed", "Failed to create output directory {path}: {error}"),
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
    (
        "runpack.export.time.negative",
        "generated_at must be a non-negative unix timestamp in milliseconds.",
    ),
    ("runpack.verify.read_failed", "Failed to read runpack manifest at {path}: {error}"),
    ("runpack.verify.parse_failed", "Failed to parse runpack manifest at {path}: {error}"),
    ("runpack.verify.reader_failed", "Failed to open runpack directory {path}: {error}"),
    ("runpack.verify.failed", "Failed to verify runpack: {error}"),
    ("runpack.verify.kind.manifest", "runpack manifest"),
    ("runpack.verify.status.pass", "pass"),
    ("runpack.verify.status.fail", "fail"),
    ("runpack.verify.md.header", "# Decision Gate Runpack Verification"),
    ("runpack.verify.md.status", "- Status: {status}"),
    ("runpack.verify.md.checked", "- Checked files: {count}"),
    ("runpack.verify.md.errors_header", "## Errors"),
    ("runpack.verify.md.error_line", "- {error}"),
    ("runpack.verify.md.no_errors", "- None"),
    (
        "runpack.pretty.output_dir_failed",
        "Failed to create pretty output directory {path}: {error}",
    ),
    ("runpack.pretty.manifest_name_missing", "Runpack manifest path is missing a filename: {path}"),
    (
        "runpack.pretty.manifest_render_failed",
        "Failed to render runpack manifest JSON for {path}: {error}",
    ),
    ("runpack.pretty.reader_failed", "Failed to open runpack directory {path}: {error}"),
    ("runpack.pretty.sink_failed", "Failed to initialize pretty output directory {path}: {error}"),
    ("runpack.pretty.read_failed", "Failed to read runpack artifact {path}: {error}"),
    ("runpack.pretty.parse_failed", "Failed to parse JSON artifact {path}: {error}"),
    ("runpack.pretty.render_failed", "Failed to render JSON artifact {path}: {error}"),
    ("runpack.pretty.write_failed", "Failed to write pretty artifact {path}: {error}"),
    ("runpack.pretty.ok", "Pretty runpack written to {path} (json: {json}, skipped: {skipped})"),
    ("authoring.read_failed", "Failed to read authoring input at {path}: {error}"),
    ("authoring.kind.input", "authoring input"),
    (
        "authoring.format.missing",
        "Unable to determine authoring format for {path}; specify --format.",
    ),
    ("authoring.parse_failed", "Failed to parse {format} input at {path}: {error}"),
    ("authoring.schema_failed", "Schema validation failed for {path}: {error}"),
    ("authoring.deserialize_failed", "Failed to deserialize ScenarioSpec from {path}: {error}"),
    ("authoring.spec_failed", "ScenarioSpec validation failed for {path}: {error}"),
    ("authoring.canonicalize_failed", "Failed to canonicalize ScenarioSpec from {path}: {error}"),
    (
        "authoring.size_limit_exceeded",
        "Authoring input at {path} exceeds size limit ({size} > {limit}).",
    ),
    (
        "authoring.depth_limit_exceeded",
        "Authoring input at {path} exceeds depth limit ({depth} > {limit}).",
    ),
    (
        "authoring.canonical_too_large",
        "Canonical JSON for {path} exceeds size limit ({size} > {limit}).",
    ),
    ("authoring.normalize.write_failed", "Failed to write normalized output to {path}: {error}"),
    ("authoring.normalize.ok", "Normalized scenario written to {path}"),
    (
        "authoring.validate.ok",
        "ScenarioSpec valid (scenario_id={scenario_id}, spec_hash={spec_hash})",
    ),
    ("interop.kind.spec", "scenario spec"),
    ("interop.kind.run_config", "run config"),
    ("interop.kind.trigger", "trigger event"),
    ("interop.read_failed", "Failed to read {kind} file at {path}: {error}"),
    ("interop.parse_failed", "Failed to parse {kind} JSON at {path}: {error}"),
    ("interop.spec_failed", "ScenarioSpec validation failed for {path}: {error}"),
    ("interop.input_invalid", "Interop input validation failed: {error}"),
    ("interop.execution_failed", "Interop execution failed: {error}"),
    ("interop.report.serialize_failed", "Failed to serialize interop report: {error}"),
    ("interop.report.write_failed", "Failed to write interop report to {path}: {error}"),
    (
        "interop.expect_status_mismatch",
        "Interop status mismatch (expected {expected}, actual {actual}).",
    ),
    (
        "interop.timestamp.conflict",
        "Both {label}_unix_ms and {label}_logical were provided; choose one.",
    ),
    ("interop.timestamp.negative", "{label}_unix_ms must be non-negative."),
    ("interop.status.active", "active"),
    ("interop.status.completed", "completed"),
    ("interop.status.failed", "failed"),
    ("provider.discovery.failed", "Provider discovery failed: {error}"),
    ("provider.discovery.denied", "Provider discovery denied for {provider}."),
    (
        "provider.discovery.serialize_failed",
        "Failed to serialize provider discovery output: {error}",
    ),
    ("provider.list.header", "Providers:"),
    ("provider.list.checks.none", "none"),
    ("provider.list.entry", "- {provider} ({transport}) checks: {checks}"),
    ("schema.invalid_id", "Invalid {field} value: {value}. Must be >= 1."),
    ("mcp.client.failed", "MCP request failed: {error}"),
    ("mcp.client.config_failed", "MCP client configuration failed: {error}"),
    ("mcp.client.input_read_failed", "Failed to read MCP input {path}: {error}"),
    ("mcp.client.input_parse_failed", "Failed to parse MCP input: {error}"),
    ("mcp.client.invalid_stdio_env", "Invalid stdio env var: {value}. Expected KEY=VALUE."),
    ("mcp.client.auth_profile_missing", "Auth profile not found: {profile}"),
    ("mcp.client.auth_config_read_failed", "Failed to read auth config at {path}: {error}"),
    ("mcp.client.auth_config_too_large", "Auth config file too large: {path}."),
    ("mcp.client.auth_config_parse_failed", "Failed to parse auth config: {error}"),
    ("mcp.client.schema_registry_missing", "Scenario schema registry missing $id."),
    ("mcp.client.schema_registry_failed", "Schema registry build failed: {error}"),
    ("mcp.client.schema_unknown_tool", "Unknown tool for schema validation: {tool}"),
    ("mcp.client.schema_compile_failed", "Schema compilation failed: {error}"),
    ("mcp.client.schema_validation_failed", "Schema validation failed for {tool}: {error}"),
    ("mcp.client.schema_lock_failed", "Schema validator lock failed."),
    ("mcp.client.json_failed", "Failed to render JSON output: {error}"),
    ("contract.generate.failed", "Contract generation failed: {error}"),
    ("contract.check.failed", "Contract verification failed: {error}"),
    ("sdk.generate.failed", "SDK generation failed: {error}"),
    ("sdk.check.failed", "SDK verification failed: {error}"),
    ("sdk.check.drift", "SDK drift detected for {path}."),
    ("sdk.io.failed", "SDK I/O failed: {error}"),
    ("output.signature.key_required", "Signature output requires --signing-key."),
    ("output.signature.out_required", "Signing key provided without --signature-out."),
    ("output.artifact.serialize_failed", "Failed to serialize {kind} artifact: {error}"),
    ("output.artifact.write_failed", "Failed to write {kind} artifact to {path}: {error}"),
    ("output.signature.key_read_failed", "Failed to read signing key at {path}: {error}"),
    ("output.signature.key_kind", "signing key"),
    ("output.signature.key_invalid", "Signing key must be 32 bytes (raw or base64)."),
    ("runpack.storage.missing", "runpack_storage is not configured in the config file."),
    ("runpack.storage.init_failed", "Failed to initialize runpack storage backend: {error}"),
    ("runpack.storage.upload_failed", "Failed to upload runpack to storage: {error}"),
    ("runpack.export.storage_ok", "Runpack stored at {uri}"),
    ("store.config.unsupported_backend", "run_state_store must be sqlite for store commands."),
    ("store.config.missing_path", "sqlite run_state_store requires path."),
    ("store.open_failed", "Failed to open sqlite store: {error}"),
    ("store.list.failed", "Failed to list runs: {error}"),
    ("store.get.failed", "Failed to load run state: {error}"),
    ("store.get.not_found", "Run not found: {run_id}"),
    ("store.export.write_failed", "Failed to write run state to {path}: {error}"),
    ("store.export.ok", "Run state written to {path}"),
    ("store.verify.failed", "Failed to verify run state: {error}"),
    ("store.verify.version_missing", "Run version not found: {version}"),
    ("store.verify.no_versions", "No run state versions found."),
    ("store.verify.hash_algorithm_invalid", "Unsupported hash algorithm: {value}"),
    ("store.prune.keep_invalid", "keep must be >= 1."),
    ("store.prune.failed", "Failed to prune run state versions: {error}"),
    ("store.list.header", "Stored runs:"),
    ("store.list.none", "No runs found."),
    (
        "store.list.entry",
        "- tenant={tenant_id} namespace={namespace_id} run={run_id} version={version} \
         saved_at={saved_at}",
    ),
    ("store.verify.header", "Run state verification:"),
    ("store.verify.summary", "- Status: {status} (version {version}, saved_at {saved_at})"),
    ("store.verify.status.pass", "pass"),
    ("store.verify.status.fail", "fail"),
    ("store.verify.hash", "- {label}: {value}"),
    ("store.verify.hash.stored", "stored"),
    ("store.verify.hash.computed", "computed"),
    ("store.verify.bytes", "- State bytes: {bytes}"),
    ("store.prune.summary", "Run {run_id}: keep {keep}, pruned {pruned} (dry_run={dry_run})"),
    ("broker.input.kind.resolve", "broker resolve input"),
    ("broker.input.kind.dispatch", "broker dispatch input"),
    ("broker.input.read_failed", "Failed to read {kind} at {path}: {error}"),
    ("broker.input.parse_failed", "Failed to parse {kind} JSON at {path}: {error}"),
    ("broker.resolve.failed", "Failed to resolve broker payload: {error}"),
    (
        "broker.resolve.content_type_mismatch",
        "Content type mismatch (expected {expected}, got {actual})",
    ),
    ("broker.resolve.unsupported_scheme", "Unsupported URI scheme: {scheme}"),
    ("broker.http.init_failed", "Failed to initialize broker HTTP source: {error}"),
    ("broker.resolve.json_parse_failed", "Failed to parse JSON payload: {error}"),
    ("broker.resolve.hash_failed", "Failed to hash payload: {error}"),
    ("broker.resolve.hash_mismatch", "Payload hash mismatch (expected {expected}, got {actual})"),
    ("broker.resolve.content_type.unknown", "unknown"),
    ("broker.resolve.header", "Broker resolve result:"),
    ("broker.resolve.uri", "URI: {uri}"),
    ("broker.resolve.content_type", "Content type: {content_type}"),
    ("broker.resolve.hash", "Content hash: {value}"),
    ("broker.resolve.bytes", "Payload bytes: {bytes}"),
    ("broker.dispatch.failed", "Failed to dispatch broker payload: {error}"),
    ("broker.dispatch.target_failed", "Failed to serialize broker target: {error}"),
    ("broker.dispatch.header", "Broker dispatch result:"),
    ("broker.dispatch.receipt", "Receipt: {dispatch_id} (dispatcher {dispatcher})"),
    ("broker.dispatch.target", "Target: {target}"),
    ("broker.dispatch.content_type", "Content type: {content_type}"),
    ("broker.dispatch.hash", "Content hash: {value}"),
    ("broker.dispatch.bytes", "Payload bytes: {bytes}"),
    ("i18n.lang.invalid_env", "Invalid value for {env}: {value}. Expected 'en' or 'ca'."),
    (
        "i18n.disclaimer.machine_translated",
        "Note: non-English output is machine-translated and may be inaccurate.",
    ),
];

/// Static Catalan catalog entries loaded into the localized message bundle.
const CATALOG_CA: &[(&str, &str)] = &[
    ("main.version", "decision-gate {version}"),
    (
        "serve.warn.local_only_auth",
        "Informació: s'està executant en mode només local (només stdio + HTTP/SSE de loopback). \
         Configureu server.auth per a bearer_token o mtls quan s'exposi fora de localhost.",
    ),
    (
        "serve.warn.loopback_only_transport",
        "Informació: HTTP/SSE està lligat només al loopback. Utilitzeu --allow-non-loopback o \
         {env}=1 amb TLS (o tls_termination=upstream) + auth per exposar-lo.",
    ),
    ("output.stream.stdout", "stdout"),
    ("output.stream.stderr", "stderr"),
    ("output.stream.unknown", "sortida"),
    ("output.write_failed", "No s'ha pogut escriure a {stream}: {error}"),
    (
        "input.read_too_large",
        "Es rebutja llegir {kind} a {path} perquè fa {size} bytes (límit {limit}).",
    ),
    ("config.load_failed", "No s'ha pogut carregar la configuració: {error}"),
    ("config.validate.ok", "Configuració vàlida."),
    ("serve.config.load_failed", "No s'ha pogut carregar la configuració: {error}"),
    ("serve.bind.parse_failed", "Adreça de bind no vàlida {bind}: {error}"),
    (
        "serve.bind.non_loopback_opt_in",
        "Es rebutja bind a l'adreça no-loopback {bind}. Establiu --allow-non-loopback o {env}=1 \
         per habilitar-ho.",
    ),
    (
        "serve.bind.non_loopback_auth_required",
        "Es rebutja bind a {bind}: server.auth.mode ha de ser bearer_token o mtls per a \
         no-loopback.",
    ),
    (
        "serve.bind.non_loopback_tls_required",
        "Es rebutja bind a {bind}: configureu server.tls o establiu \
         server.tls_termination=upstream.",
    ),
    (
        "serve.bind.non_loopback_mtls_client_ca_required",
        "Es rebutja bind a {bind}: mTLS requereix tls.client_ca_path.",
    ),
    (
        "serve.bind.non_loopback_mtls_client_cert_required",
        "Es rebutja bind a {bind}: mTLS requereix tls.require_client_cert=true.",
    ),
    (
        "serve.bind.allow_env_invalid",
        "Valor no vàlid per a {env}: {value}. S'esperava true/false/1/0/yes/no/on/off.",
    ),
    ("serve.warn.network.header", "AVÍS DE SEGURETAT: Decision Gate està exposat a la xarxa."),
    ("serve.warn.network.bind", "Bind: {bind}"),
    ("serve.warn.network.auth", "Mode d'autenticació: {mode}"),
    ("serve.warn.network.tls", "TLS: {tls}"),
    ("serve.warn.network.audit", "Registre d'auditoria: {status}"),
    ("serve.warn.network.rate_limit", "Limitació de taxa: {status}"),
    (
        "serve.warn.network.footer",
        "Verifiqueu les regles del tallafoc i les credencials; aquesta exposició és intencionada.",
    ),
    ("serve.warn.network.enabled", "habilitat"),
    ("serve.warn.network.disabled", "deshabilitat"),
    (
        "serve.warn.network.tls_enabled",
        "habilitat (certificat de client {client_cert}, CA de client {client_ca})",
    ),
    ("serve.warn.network.tls_disabled", "deshabilitat"),
    ("serve.warn.network.tls_upstream", "upstream (terminat per proxy/ingress)"),
    ("serve.warn.network.required", "requerit"),
    ("serve.warn.network.not_required", "no requerit"),
    ("serve.warn.network.present", "present"),
    ("serve.warn.network.missing", "manca"),
    ("serve.init_failed", "No s'ha pogut inicialitzar el servidor MCP: {error}"),
    ("serve.failed", "El servidor MCP ha fallat: {error}"),
    ("runpack.export.read_failed", "No s'ha pogut llegir el fitxer {kind} a {path}: {error}"),
    ("runpack.export.parse_failed", "No s'ha pogut analitzar el JSON {kind} a {path}: {error}"),
    ("runpack.export.spec_failed", "Validació de ScenarioSpec fallida per a {path}: {error}"),
    (
        "runpack.export.output_dir_failed",
        "No s'ha pogut crear el directori de sortida {path}: {error}",
    ),
    (
        "runpack.export.sink_failed",
        "No s'ha pogut inicialitzar el sink de runpack a {path}: {error}",
    ),
    ("runpack.export.build_failed", "No s'ha pogut construir el runpack: {error}"),
    ("runpack.export.ok", "Manifest del runpack escrit a {path}"),
    ("runpack.export.verification_status", "Estat de verificació: {status}"),
    ("runpack.export.kind.spec", "especificació d'escenari"),
    ("runpack.export.kind.state", "estat d'execució"),
    (
        "runpack.export.time.system_failed",
        "No s'ha pogut llegir l'hora del sistema per generar el runpack: {error}",
    ),
    (
        "runpack.export.time.overflow",
        "L'hora del sistema està fora de rang per generar el runpack.",
    ),
    (
        "runpack.export.time.negative",
        "generated_at ha de ser una marca de temps unix en mil·lisegons no negativa.",
    ),
    (
        "runpack.verify.read_failed",
        "No s'ha pogut llegir el manifest del runpack a {path}: {error}",
    ),
    (
        "runpack.verify.parse_failed",
        "No s'ha pogut analitzar el manifest del runpack a {path}: {error}",
    ),
    (
        "runpack.verify.reader_failed",
        "No s'ha pogut obrir el directori del runpack {path}: {error}",
    ),
    ("runpack.verify.failed", "No s'ha pogut verificar el runpack: {error}"),
    ("runpack.verify.kind.manifest", "manifest del runpack"),
    ("runpack.verify.status.pass", "aprovat"),
    ("runpack.verify.status.fail", "fallat"),
    ("runpack.verify.md.header", "# Verificació del Runpack de Decision Gate"),
    ("runpack.verify.md.status", "- Estat: {status}"),
    ("runpack.verify.md.checked", "- Fitxers comprovats: {count}"),
    ("runpack.verify.md.errors_header", "## Errors"),
    ("runpack.verify.md.error_line", "- {error}"),
    ("runpack.verify.md.no_errors", "- Cap"),
    (
        "runpack.pretty.output_dir_failed",
        "No s'ha pogut crear el directori de sortida formatada {path}: {error}",
    ),
    ("runpack.pretty.manifest_name_missing", "El manifest del runpack no té nom de fitxer: {path}"),
    (
        "runpack.pretty.manifest_render_failed",
        "No s'ha pogut renderitzar el JSON del manifest {path}: {error}",
    ),
    (
        "runpack.pretty.reader_failed",
        "No s'ha pogut obrir el directori del runpack {path}: {error}",
    ),
    (
        "runpack.pretty.sink_failed",
        "No s'ha pogut inicialitzar el directori de sortida {path}: {error}",
    ),
    ("runpack.pretty.read_failed", "No s'ha pogut llegir l'artefacte del runpack {path}: {error}"),
    (
        "runpack.pretty.parse_failed",
        "No s'ha pogut analitzar el JSON de l'artefacte {path}: {error}",
    ),
    (
        "runpack.pretty.render_failed",
        "No s'ha pogut renderitzar el JSON de l'artefacte {path}: {error}",
    ),
    ("runpack.pretty.write_failed", "No s'ha pogut escriure l'artefacte formatat {path}: {error}"),
    ("runpack.pretty.ok", "Runpack formatat escrit a {path} (json: {json}, omesos: {skipped})"),
    ("authoring.read_failed", "No s'ha pogut llegir l'entrada d'autoria a {path}: {error}"),
    ("authoring.kind.input", "entrada d'autoria"),
    (
        "authoring.format.missing",
        "No s'ha pogut determinar el format d'autoria per a {path}; especifiqueu --format.",
    ),
    ("authoring.parse_failed", "No s'ha pogut analitzar l'entrada {format} a {path}: {error}"),
    ("authoring.schema_failed", "Validació d'esquema fallida per a {path}: {error}"),
    (
        "authoring.deserialize_failed",
        "No s'ha pogut deserialitzar ScenarioSpec des de {path}: {error}",
    ),
    ("authoring.spec_failed", "Validació de ScenarioSpec fallida per a {path}: {error}"),
    (
        "authoring.canonicalize_failed",
        "No s'ha pogut canonitzar ScenarioSpec des de {path}: {error}",
    ),
    (
        "authoring.size_limit_exceeded",
        "L'entrada d'autoria a {path} supera el límit de mida ({size} > {limit}).",
    ),
    (
        "authoring.depth_limit_exceeded",
        "L'entrada d'autoria a {path} supera el límit de profunditat ({depth} > {limit}).",
    ),
    (
        "authoring.canonical_too_large",
        "El JSON canònic per a {path} supera el límit de mida ({size} > {limit}).",
    ),
    (
        "authoring.normalize.write_failed",
        "No s'ha pogut escriure la sortida normalitzada a {path}: {error}",
    ),
    ("authoring.normalize.ok", "Escenari normalitzat escrit a {path}"),
    (
        "authoring.validate.ok",
        "ScenarioSpec vàlid (scenario_id={scenario_id}, spec_hash={spec_hash})",
    ),
    ("interop.kind.spec", "especificació d'escenari"),
    ("interop.kind.run_config", "configuració d'execució"),
    ("interop.kind.trigger", "esdeveniment de desencadenament"),
    ("interop.read_failed", "No s'ha pogut llegir el fitxer {kind} a {path}: {error}"),
    ("interop.parse_failed", "No s'ha pogut analitzar el JSON {kind} a {path}: {error}"),
    ("interop.spec_failed", "Validació de ScenarioSpec fallida per a {path}: {error}"),
    ("interop.input_invalid", "Validació d'entrada d'interoperabilitat fallida: {error}"),
    ("interop.execution_failed", "Execució d'interoperabilitat fallida: {error}"),
    (
        "interop.report.serialize_failed",
        "No s'ha pogut serialitzar l'informe d'interoperabilitat: {error}",
    ),
    (
        "interop.report.write_failed",
        "No s'ha pogut escriure l'informe d'interoperabilitat a {path}: {error}",
    ),
    (
        "interop.expect_status_mismatch",
        "Desajust d'estat d'interoperabilitat (s'esperava {expected}, actual {actual}).",
    ),
    (
        "interop.timestamp.conflict",
        "S'han proporcionat {label}_unix_ms i {label}_logical; trieu-ne un.",
    ),
    ("interop.timestamp.negative", "{label}_unix_ms ha de ser no negatiu."),
    ("interop.status.active", "actiu"),
    ("interop.status.completed", "completat"),
    ("interop.status.failed", "fallat"),
    ("provider.discovery.failed", "La descoberta de proveïdors ha fallat: {error}"),
    ("provider.discovery.denied", "Descoberta de proveïdors denegada per a {provider}."),
    (
        "provider.discovery.serialize_failed",
        "No s'ha pogut serialitzar la sortida de descoberta de proveïdors: {error}",
    ),
    ("provider.list.header", "Proveïdors:"),
    ("provider.list.checks.none", "cap"),
    ("provider.list.entry", "- {provider} ({transport}) comprovacions: {checks}"),
    ("schema.invalid_id", "Valor de {field} no vàlid: {value}. Ha de ser >= 1."),
    ("mcp.client.failed", "La sol·licitud MCP ha fallat: {error}"),
    ("mcp.client.config_failed", "La configuració del client MCP ha fallat: {error}"),
    ("mcp.client.input_read_failed", "No s'ha pogut llegir l'entrada MCP {path}: {error}"),
    ("mcp.client.input_parse_failed", "No s'ha pogut analitzar l'entrada MCP: {error}"),
    (
        "mcp.client.invalid_stdio_env",
        "Variable d'entorn stdio no vàlida: {value}. S'esperava KEY=VALUE.",
    ),
    ("mcp.client.auth_profile_missing", "Perfil d'autenticació no trobat: {profile}"),
    (
        "mcp.client.auth_config_read_failed",
        "No s'ha pogut llegir la configuració d'autenticació a {path}: {error}",
    ),
    (
        "mcp.client.auth_config_too_large",
        "Fitxer de configuració d'autenticació massa gran: {path}.",
    ),
    (
        "mcp.client.auth_config_parse_failed",
        "No s'ha pogut analitzar la configuració d'autenticació: {error}",
    ),
    ("mcp.client.schema_registry_missing", "L'ID $id de l'esquema d'escenari manca."),
    (
        "mcp.client.schema_registry_failed",
        "La construcció del registre d'esquemes ha fallat: {error}",
    ),
    ("mcp.client.schema_unknown_tool", "Eina desconeguda per a validació d'esquema: {tool}"),
    ("mcp.client.schema_compile_failed", "La compilació de l'esquema ha fallat: {error}"),
    (
        "mcp.client.schema_validation_failed",
        "La validació de l'esquema ha fallat per a {tool}: {error}",
    ),
    ("mcp.client.schema_lock_failed", "El bloqueig del validador d'esquemes ha fallat."),
    ("mcp.client.json_failed", "No s'ha pogut renderitzar la sortida JSON: {error}"),
    ("contract.generate.failed", "La generació del contracte ha fallat: {error}"),
    ("contract.check.failed", "La verificació del contracte ha fallat: {error}"),
    ("sdk.generate.failed", "La generació de l'SDK ha fallat: {error}"),
    ("sdk.check.failed", "La verificació de l'SDK ha fallat: {error}"),
    ("sdk.check.drift", "S'ha detectat drift de l'SDK per a {path}."),
    ("sdk.io.failed", "L'E/S de l'SDK ha fallat: {error}"),
    ("output.signature.key_required", "La sortida de signatura requereix --signing-key."),
    (
        "output.signature.out_required",
        "S'ha proporcionat una clau de signatura sense --signature-out.",
    ),
    ("output.artifact.serialize_failed", "No s'ha pogut serialitzar l'artefacte {kind}: {error}"),
    ("output.artifact.write_failed", "No s'ha pogut escriure l'artefacte {kind} a {path}: {error}"),
    (
        "output.signature.key_read_failed",
        "No s'ha pogut llegir la clau de signatura a {path}: {error}",
    ),
    ("output.signature.key_kind", "clau de signatura"),
    ("output.signature.key_invalid", "La clau de signatura ha de ser de 32 bytes (raw o base64)."),
    ("runpack.storage.missing", "runpack_storage no està configurat al fitxer de configuració."),
    (
        "runpack.storage.init_failed",
        "No s'ha pogut inicialitzar el backend d'emmagatzematge de runpack: {error}",
    ),
    ("runpack.storage.upload_failed", "No s'ha pogut pujar el runpack a l'emmagatzematge: {error}"),
    ("runpack.export.storage_ok", "Runpack emmagatzemat a {uri}"),
    (
        "store.config.unsupported_backend",
        "run_state_store ha de ser sqlite per a les ordres de store.",
    ),
    ("store.config.missing_path", "sqlite run_state_store requereix path."),
    ("store.open_failed", "No s'ha pogut obrir la base de dades sqlite: {error}"),
    ("store.list.failed", "No s'han pogut llistar les execucions: {error}"),
    ("store.get.failed", "No s'ha pogut carregar l'estat d'execució: {error}"),
    ("store.get.not_found", "Execució no trobada: {run_id}"),
    ("store.export.write_failed", "No s'ha pogut escriure l'estat d'execució a {path}: {error}"),
    ("store.export.ok", "Estat d'execució escrit a {path}"),
    ("store.verify.failed", "No s'ha pogut verificar l'estat d'execució: {error}"),
    ("store.verify.version_missing", "Versió d'execució no trobada: {version}"),
    ("store.verify.no_versions", "No s'han trobat versions d'estat d'execució."),
    ("store.verify.hash_algorithm_invalid", "Algorisme de hash no compatible: {value}"),
    ("store.prune.keep_invalid", "keep ha de ser >= 1."),
    ("store.prune.failed", "No s'han pogut esborrar versions d'estat d'execució: {error}"),
    ("store.list.header", "Execucions emmagatzemades:"),
    ("store.list.none", "No s'han trobat execucions."),
    (
        "store.list.entry",
        "- tenant={tenant_id} namespace={namespace_id} run={run_id} version={version} \
         saved_at={saved_at}",
    ),
    ("store.verify.header", "Verificació de l'estat d'execució:"),
    ("store.verify.summary", "- Estat: {status} (versió {version}, saved_at {saved_at})"),
    ("store.verify.status.pass", "aprovat"),
    ("store.verify.status.fail", "fallat"),
    ("store.verify.hash", "- {label}: {value}"),
    ("store.verify.hash.stored", "emmagatzemat"),
    ("store.verify.hash.computed", "calculat"),
    ("store.verify.bytes", "- Bytes de l'estat: {bytes}"),
    (
        "store.prune.summary",
        "Execució {run_id}: conservar {keep}, eliminades {pruned} (dry_run={dry_run})",
    ),
    ("broker.input.kind.resolve", "entrada de resolució del broker"),
    ("broker.input.kind.dispatch", "entrada de dispatch del broker"),
    ("broker.input.read_failed", "No s'ha pogut llegir {kind} a {path}: {error}"),
    ("broker.input.parse_failed", "No s'ha pogut analitzar el JSON de {kind} a {path}: {error}"),
    ("broker.resolve.failed", "No s'ha pogut resoldre el payload del broker: {error}"),
    (
        "broker.resolve.content_type_mismatch",
        "Desajust de tipus de contingut (s'esperava {expected}, actual {actual})",
    ),
    ("broker.resolve.unsupported_scheme", "Esquema d'URI no compatible: {scheme}"),
    ("broker.http.init_failed", "No s'ha pogut inicialitzar la font HTTP del broker: {error}"),
    ("broker.resolve.json_parse_failed", "No s'ha pogut analitzar el payload JSON: {error}"),
    ("broker.resolve.hash_failed", "No s'ha pogut calcular el hash del payload: {error}"),
    (
        "broker.resolve.hash_mismatch",
        "Desajust de hash del payload (s'esperava {expected}, actual {actual})",
    ),
    ("broker.resolve.content_type.unknown", "desconegut"),
    ("broker.resolve.header", "Resultat de resolució del broker:"),
    ("broker.resolve.uri", "URI: {uri}"),
    ("broker.resolve.content_type", "Tipus de contingut: {content_type}"),
    ("broker.resolve.hash", "Hash de contingut: {value}"),
    ("broker.resolve.bytes", "Bytes del payload: {bytes}"),
    ("broker.dispatch.failed", "No s'ha pogut enviar el payload del broker: {error}"),
    ("broker.dispatch.target_failed", "No s'ha pogut serialitzar el destí del broker: {error}"),
    ("broker.dispatch.header", "Resultat de dispatch del broker:"),
    ("broker.dispatch.receipt", "Rebut: {dispatch_id} (dispatcher {dispatcher})"),
    ("broker.dispatch.target", "Destí: {target}"),
    ("broker.dispatch.content_type", "Tipus de contingut: {content_type}"),
    ("broker.dispatch.hash", "Hash de contingut: {value}"),
    ("broker.dispatch.bytes", "Bytes del payload: {bytes}"),
    ("i18n.lang.invalid_env", "Valor no vàlid per a {env}: {value}. S'esperava 'en' o 'ca'."),
    (
        "i18n.disclaimer.machine_translated",
        "Nota: la sortida que no és en anglès està traduïda automàticament i pot ser inexacta.",
    ),
];

/// Returns the message catalog for the requested locale.
pub(crate) fn catalog_for(locale: Locale) -> &'static HashMap<&'static str, &'static str> {
    static CATALOG_EN_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    static CATALOG_CA_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    match locale {
        Locale::En => CATALOG_EN_MAP.get_or_init(|| CATALOG_EN.iter().copied().collect()),
        Locale::Ca => CATALOG_CA_MAP.get_or_init(|| CATALOG_CA.iter().copied().collect()),
    }
}

// ============================================================================
// SECTION: Translation
// ============================================================================

/// Translates `key` using the selected locale while substituting `args`.
#[must_use]
pub fn translate(key: &str, args: Vec<MessageArg>) -> String {
    let locale = current_locale();
    let template = catalog_for(locale)
        .get(key)
        .copied()
        .or_else(|| catalog_for(Locale::En).get(key).copied())
        .unwrap_or(key);
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

// ============================================================================
// SECTION: Macro
// ============================================================================

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
                $crate::i18n::MessageArg::new(stringify!($name), $value.to_string()),
            )*
        ];
        $crate::i18n::translate($key, args)
    }};
}
