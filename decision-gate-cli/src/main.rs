// decision-gate-cli/src/main.rs
// ============================================================================
// Module: Decision Gate CLI Entry Point
// Description: Command dispatcher for Decision Gate MCP and runpack workflows.
// Purpose: Provide a safe, localized CLI for server and offline runpack tasks.
// Dependencies: clap, decision-gate-core, decision-gate-mcp, serde, thiserror, tokio.
// ============================================================================

//! ## Overview
//! The Decision Gate CLI orchestrates local MCP server execution and offline
//! runpack workflows. All user-facing strings are routed through the i18n
//! catalog to prepare for future localization. Security posture: inputs are
//! untrusted and must be validated; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub(crate) mod interop;
#[cfg(test)]
mod main_tests;
pub(crate) mod mcp_client;

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use clap::ArgAction;
use clap::Args;
use clap::CommandFactory;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use decision_gate_cli::i18n::Locale;
use decision_gate_cli::i18n::set_locale;
use decision_gate_cli::serve_policy::ALLOW_NON_LOOPBACK_ENV;
use decision_gate_cli::serve_policy::BindOutcome;
use decision_gate_cli::serve_policy::enforce_local_only;
use decision_gate_cli::serve_policy::resolve_allow_non_loopback;
use decision_gate_cli::t;
use decision_gate_config as config;
use decision_gate_contract::AuthoringError;
use decision_gate_contract::AuthoringFormat;
use decision_gate_contract::authoring;
use decision_gate_contract::tooling::tool_contracts;
use decision_gate_contract::types::ToolContract;
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunConfig;
use decision_gate_core::RunState;
use decision_gate_core::RunStatus;
use decision_gate_core::RunpackManifest;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::HashError;
use decision_gate_core::hashing::canonical_json_bytes_with_limit;
use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use decision_gate_core::runtime::RunpackBuilder;
use decision_gate_core::runtime::RunpackVerifier;
use decision_gate_core::runtime::VerificationReport;
use decision_gate_core::runtime::VerificationStatus;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::FileArtifactReader;
use decision_gate_mcp::FileArtifactSink;
use decision_gate_mcp::McpServer;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::ServerAuthMode;
use decision_gate_mcp::config::ServerTransport;
use interop::InteropConfig;
use interop::InteropTransport;
use interop::run_interop;
use interop::validate_inputs;
use jsonschema::Draft;
use jsonschema::Registry;
use jsonschema::Validator;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::mcp_client::McpClient;
use crate::mcp_client::McpClientConfig;
use crate::mcp_client::McpTransport;
use crate::mcp_client::ResourceContent;
use crate::mcp_client::ResourceMetadata;
use crate::mcp_client::stdio_config_env;

// ============================================================================
// SECTION: Limits
// ============================================================================

/// Maximum size of a `ScenarioSpec` JSON input.
const MAX_SPEC_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of a `RunState` JSON input.
const MAX_RUN_STATE_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES * 8;
/// Maximum size of a runpack manifest JSON input.
const MAX_MANIFEST_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of an authoring input payload.
const MAX_AUTHORING_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of interop scenario spec inputs.
const MAX_INTEROP_SPEC_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of interop run config inputs.
const MAX_INTEROP_RUN_CONFIG_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of interop trigger inputs.
const MAX_INTEROP_TRIGGER_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of MCP tool input payloads.
const MAX_MCP_INPUT_BYTES: usize = MAX_RUNPACK_ARTIFACT_BYTES;
/// Maximum size of auth profile config files.
const MAX_AUTH_CONFIG_BYTES: usize = 1024 * 1024;
/// Environment variable for CLI locale selection.
const LANG_ENV: &str = "DECISION_GATE_LANG";

// ============================================================================
// SECTION: CLI Types
// ============================================================================

/// Top-level CLI definition.
#[derive(Parser, Debug)]
#[command(name = "decision-gate", disable_help_subcommand = true, disable_version_flag = true)]
struct Cli {
    /// Print version information and exit.
    #[arg(long = "version", action = ArgAction::SetTrue, global = true)]
    show_version: bool,
    /// Preferred output language (overrides `DECISION_GATE_LANG`).
    #[arg(long, value_enum, value_name = "LANG", global = true)]
    lang: Option<LangArg>,
    /// Selected subcommand to execute.
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Supported CLI subcommands.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Decision Gate MCP server.
    Serve(ServeCommand),
    /// Runpack export and verification utilities.
    Runpack {
        /// Selected runpack subcommand.
        #[command(subcommand)]
        command: RunpackCommand,
    },
    /// `ScenarioSpec` authoring utilities.
    Authoring {
        /// Selected authoring subcommand.
        #[command(subcommand)]
        command: AuthoringCommand,
    },
    /// Configuration utilities.
    Config {
        /// Selected config subcommand.
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Provider discovery utilities.
    Provider {
        /// Selected provider subcommand.
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Schema registry utilities.
    Schema {
        /// Selected schema subcommand.
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Documentation utilities.
    Docs {
        /// Selected docs subcommand.
        #[command(subcommand)]
        command: DocsCommand,
    },
    /// Interop evaluation utilities.
    Interop {
        /// Selected interop subcommand.
        #[command(subcommand)]
        command: InteropCommand,
    },
    /// MCP client utilities.
    Mcp {
        /// Selected MCP client subcommand.
        #[command(subcommand)]
        command: McpCommand,
    },
    /// Contract generation utilities.
    Contract {
        /// Selected contract subcommand.
        #[command(subcommand)]
        command: ContractCommand,
    },
    /// SDK generation utilities.
    Sdk {
        /// Selected SDK subcommand.
        #[command(subcommand)]
        command: SdkCommand,
    },
}

/// Configuration for the `serve` command.
#[derive(Args, Debug)]
struct ServeCommand {
    /// Optional config file path (defaults to decision-gate.toml or env override).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Allow binding HTTP/SSE transports to non-loopback addresses (requires TLS + auth).
    #[arg(long, action = ArgAction::SetTrue)]
    allow_non_loopback: bool,
}

/// Runpack subcommands.
#[derive(Subcommand, Debug)]
enum RunpackCommand {
    /// Export a runpack from a spec and run state.
    Export(RunpackExportCommand),
    /// Verify a runpack manifest against its artifacts.
    Verify(RunpackVerifyCommand),
}

/// Authoring subcommands.
#[derive(Subcommand, Debug)]
enum AuthoringCommand {
    /// Validate a `ScenarioSpec` authoring input.
    Validate(AuthoringValidateCommand),
    /// Normalize a `ScenarioSpec` authoring input to canonical JSON.
    Normalize(AuthoringNormalizeCommand),
}

/// Config subcommands.
#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Validate a Decision Gate configuration file.
    Validate(ConfigValidateCommand),
}

/// Provider discovery subcommands.
#[derive(Subcommand, Debug)]
enum ProviderCommand {
    /// Provider contract operations.
    Contract {
        /// Selected contract subcommand.
        #[command(subcommand)]
        command: ProviderContractCommand,
    },
    /// Provider schema operations.
    CheckSchema {
        /// Selected check schema subcommand.
        #[command(subcommand)]
        command: ProviderCheckSchemaCommand,
    },
    /// List configured providers and checks.
    List(ProviderListCommand),
}

/// Schema registry subcommands.
#[derive(Subcommand, Debug)]
enum SchemaCommand {
    /// Register a schema record.
    Register(SchemaRegisterCommand),
    /// List schema records.
    List(SchemaListCommand),
    /// Fetch a schema record by id/version.
    Get(SchemaGetCommand),
}

/// Documentation subcommands.
#[derive(Subcommand, Debug)]
enum DocsCommand {
    /// Search documentation sections.
    Search(DocsSearchCommand),
    /// List documentation resources.
    List(DocsListCommand),
    /// Read a documentation resource.
    Read(DocsReadCommand),
}

/// Provider contract subcommands.
#[derive(Subcommand, Debug)]
enum ProviderContractCommand {
    /// Fetch provider contract JSON.
    Get(ProviderContractGetCommand),
}

/// Provider schema subcommands.
#[derive(Subcommand, Debug)]
enum ProviderCheckSchemaCommand {
    /// Fetch provider check schema metadata.
    Get(ProviderCheckSchemaGetCommand),
}

/// Arguments for `provider contract get`.
#[derive(Args, Debug)]
struct ProviderContractGetCommand {
    /// Provider identifier.
    #[arg(long, value_name = "PROVIDER")]
    provider: String,
    /// Optional config file path (defaults to decision-gate.toml or env override).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
}

/// Arguments for `provider check-schema get`.
#[derive(Args, Debug)]
struct ProviderCheckSchemaGetCommand {
    /// Provider identifier.
    #[arg(long, value_name = "PROVIDER")]
    provider: String,
    /// Check identifier.
    #[arg(long = "check-id", value_name = "CHECK_ID")]
    check_id: String,
    /// Optional config file path (defaults to decision-gate.toml or env override).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
}

/// Arguments for `provider list`.
#[derive(Args, Debug)]
struct ProviderListCommand {
    /// Optional config file path (defaults to decision-gate.toml or env override).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Output format for provider listings.
    #[arg(long, value_enum, default_value_t = ProviderListFormat::Json)]
    format: ProviderListFormat,
}

/// Arguments for `schema register`.
#[derive(Args, Debug)]
struct SchemaRegisterCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Schema register input source.
    #[command(flatten)]
    input: McpToolInputArgs,
    /// Disable schema validation for tool input.
    #[arg(long, action = ArgAction::SetTrue)]
    no_validate: bool,
}

/// Arguments for `schema list`.
#[derive(Args, Debug)]
struct SchemaListCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Tenant identifier.
    #[arg(long, value_name = "TENANT_ID")]
    tenant_id: u64,
    /// Namespace identifier.
    #[arg(long, value_name = "NAMESPACE_ID")]
    namespace_id: u64,
    /// Pagination cursor.
    #[arg(long, value_name = "CURSOR")]
    cursor: Option<String>,
    /// Maximum number of records to return.
    #[arg(long, value_name = "LIMIT")]
    limit: Option<usize>,
}

/// Arguments for `schema get`.
#[derive(Args, Debug)]
struct SchemaGetCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Tenant identifier.
    #[arg(long, value_name = "TENANT_ID")]
    tenant_id: u64,
    /// Namespace identifier.
    #[arg(long, value_name = "NAMESPACE_ID")]
    namespace_id: u64,
    /// Schema identifier.
    #[arg(long, value_name = "SCHEMA_ID")]
    schema_id: String,
    /// Schema version.
    #[arg(long = "schema-version", value_name = "VERSION")]
    version: String,
}

/// Arguments for `docs search`.
#[derive(Args, Debug)]
struct DocsSearchCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Query string for documentation search.
    #[arg(long, value_name = "QUERY")]
    query: Option<String>,
    /// Maximum number of sections to return.
    #[arg(long = "max-sections", value_name = "COUNT")]
    max_sections: Option<u32>,
}

/// Arguments for `docs list`.
#[derive(Args, Debug)]
struct DocsListCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
}

/// Arguments for `docs read`.
#[derive(Args, Debug)]
struct DocsReadCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Resource URI to read.
    #[arg(long, value_name = "URI")]
    uri: String,
}

/// Output formats for provider lists.
#[derive(ValueEnum, Copy, Clone, Debug)]
enum ProviderListFormat {
    /// Canonical JSON output.
    Json,
    /// Human-readable text output.
    Text,
}

/// Interop subcommands.
#[derive(Subcommand, Debug)]
enum InteropCommand {
    /// Execute an interop evaluation against an MCP server.
    Eval(InteropEvalCommand),
}

/// MCP client subcommands.
#[derive(Subcommand, Debug)]
enum McpCommand {
    /// MCP tools commands.
    Tools {
        /// Selected tools subcommand.
        #[command(subcommand)]
        command: McpToolsCommand,
    },
    /// MCP resources commands.
    Resources {
        /// Selected resources subcommand.
        #[command(subcommand)]
        command: McpResourcesCommand,
    },
    /// Typed MCP tool wrappers.
    Tool {
        /// Selected tool subcommand.
        #[command(subcommand)]
        command: McpToolCommand,
    },
}

/// MCP tools subcommands.
#[derive(Subcommand, Debug)]
enum McpToolsCommand {
    /// List MCP tool definitions.
    List(McpToolsListCommand),
    /// Call an MCP tool by name.
    Call(McpToolCallCommand),
}

/// MCP resources subcommands.
#[derive(Subcommand, Debug)]
enum McpResourcesCommand {
    /// List MCP resources.
    List(McpResourcesListCommand),
    /// Read an MCP resource by URI.
    Read(McpResourcesReadCommand),
}

/// Typed MCP tool wrappers.
#[derive(Subcommand, Debug)]
enum McpToolCommand {
    /// `scenario_define` tool.
    ScenarioDefine(McpToolInputCommand),
    /// `scenario_start` tool.
    ScenarioStart(McpToolInputCommand),
    /// `scenario_status` tool.
    ScenarioStatus(McpToolInputCommand),
    /// `scenario_next` tool.
    ScenarioNext(McpToolInputCommand),
    /// `scenario_submit` tool.
    ScenarioSubmit(McpToolInputCommand),
    /// `scenario_trigger` tool.
    ScenarioTrigger(McpToolInputCommand),
    /// `scenarios_list` tool.
    ScenariosList(McpToolInputCommand),
    /// `evidence_query` tool.
    EvidenceQuery(McpToolInputCommand),
    /// `runpack_export` tool.
    RunpackExport(McpToolInputCommand),
    /// `runpack_verify` tool.
    RunpackVerify(McpToolInputCommand),
    /// `providers_list` tool.
    ProvidersList(McpToolInputCommand),
    /// `provider_contract_get` tool.
    ProviderContractGet(McpToolInputCommand),
    /// `provider_check_schema_get` tool.
    ProviderCheckSchemaGet(McpToolInputCommand),
    /// `schemas_register` tool.
    SchemasRegister(McpToolInputCommand),
    /// `schemas_list` tool.
    SchemasList(McpToolInputCommand),
    /// `schemas_get` tool.
    SchemasGet(McpToolInputCommand),
    /// `precheck` tool.
    Precheck(McpToolInputCommand),
    /// `decision_gate_docs_search` tool.
    DecisionGateDocsSearch(McpToolInputCommand),
}

/// Contract subcommands.
#[derive(Subcommand, Debug)]
enum ContractCommand {
    /// Generate Decision Gate contract artifacts.
    Generate(ContractGenerateCommand),
    /// Verify Decision Gate contract artifacts.
    Check(ContractCheckCommand),
}

/// SDK generation subcommands.
#[derive(Subcommand, Debug)]
enum SdkCommand {
    /// Generate SDK artifacts from tooling.json.
    Generate(SdkGenerateCommand),
    /// Verify SDK artifacts match the generated output.
    Check(SdkCheckCommand),
}

/// Expected run status for interop evaluation.
#[derive(ValueEnum, Copy, Clone, Debug)]
enum ExpectedRunStatusArg {
    /// Run remains active.
    Active,
    /// Run completes successfully.
    Completed,
    /// Run fails.
    Failed,
}

/// Supported CLI language selections.
#[derive(ValueEnum, Copy, Clone, Debug)]
enum LangArg {
    /// English.
    En,
    /// Catalan.
    Ca,
}

/// Arguments for interop evaluation.
#[derive(Args, Debug)]
struct InteropEvalCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Path to the scenario spec JSON file.
    #[arg(long, value_name = "PATH")]
    spec: PathBuf,
    /// Path to the run config JSON file.
    #[arg(long, value_name = "PATH")]
    run_config: PathBuf,
    /// Path to the trigger event JSON file.
    #[arg(long, value_name = "PATH")]
    trigger: PathBuf,
    /// Timestamp for scenario start (unix milliseconds).
    #[arg(long, value_name = "UNIX_MS", conflicts_with = "started_at_logical")]
    started_at_unix_ms: Option<i64>,
    /// Timestamp for scenario start (logical).
    #[arg(long, value_name = "LOGICAL", conflicts_with = "started_at_unix_ms")]
    started_at_logical: Option<u64>,
    /// Timestamp for status request (unix milliseconds).
    #[arg(long, value_name = "UNIX_MS", conflicts_with = "status_requested_at_logical")]
    status_requested_at_unix_ms: Option<i64>,
    /// Timestamp for status request (logical).
    #[arg(long, value_name = "LOGICAL", conflicts_with = "status_requested_at_unix_ms")]
    status_requested_at_logical: Option<u64>,
    /// Issue entry packets immediately on scenario start.
    #[arg(long, action = ArgAction::SetTrue)]
    issue_entry_packets: bool,
    /// Expected run status for exit code evaluation.
    #[arg(long, value_enum, value_name = "STATUS")]
    expect_status: Option<ExpectedRunStatusArg>,
    /// Optional output path for the interop report (defaults to stdout).
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
}

/// Shared MCP client connection arguments.
#[derive(Args, Debug, Clone)]
struct McpClientArgs {
    /// MCP transport to use.
    #[arg(long, value_enum, default_value_t = McpTransportArg::Http)]
    transport: McpTransportArg,
    /// MCP HTTP/SSE endpoint URL (e.g., <http://127.0.0.1:8080/rpc>).
    #[arg(long, value_name = "URL", alias = "mcp-url")]
    endpoint: Option<String>,
    /// Stdio MCP command to spawn (for stdio transport).
    #[arg(long, value_name = "COMMAND")]
    stdio_command: Option<String>,
    /// Stdio MCP command arguments (repeatable).
    #[arg(long, value_name = "ARG", action = ArgAction::Append)]
    stdio_args: Vec<String>,
    /// Stdio MCP environment variables (KEY=VALUE, repeatable).
    #[arg(long, value_name = "KEY=VALUE", action = ArgAction::Append)]
    stdio_env: Vec<String>,
    /// Convenience stdio config path (sets `DECISION_GATE_CONFIG`).
    #[arg(long, value_name = "PATH")]
    stdio_config: Option<PathBuf>,
    /// MCP request timeout in milliseconds.
    #[arg(long, value_name = "MS", default_value_t = 5_000)]
    timeout_ms: u64,
    /// Optional bearer token for MCP authentication.
    #[arg(long, value_name = "TOKEN")]
    bearer_token: Option<String>,
    /// Optional client subject header for mTLS proxy auth.
    #[arg(long, value_name = "SUBJECT")]
    client_subject: Option<String>,
    /// Optional auth profile name to load from config.
    #[arg(long, value_name = "PROFILE")]
    auth_profile: Option<String>,
    /// Optional config path for auth profiles (defaults to decision-gate.toml).
    #[arg(long, value_name = "PATH")]
    auth_config: Option<PathBuf>,
}

/// Arguments for `mcp tools list`.
#[derive(Args, Debug)]
struct McpToolsListCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
}

/// Arguments for `mcp tools call`.
#[derive(Args, Debug)]
struct McpToolCallCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Tool name to invoke.
    #[arg(long, value_enum, value_name = "TOOL")]
    tool: McpToolNameArg,
    /// Tool input source.
    #[command(flatten)]
    input: McpToolInputArgs,
    /// Disable schema validation for tool input.
    #[arg(long, action = ArgAction::SetTrue)]
    no_validate: bool,
}

/// Arguments for `mcp resources list`.
#[derive(Args, Debug)]
struct McpResourcesListCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
}

/// Arguments for `mcp resources read`.
#[derive(Args, Debug)]
struct McpResourcesReadCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Resource URI to read.
    #[arg(long, value_name = "URI")]
    uri: String,
}

/// Arguments shared by typed tool wrappers.
#[derive(Args, Debug)]
struct McpToolInputCommand {
    /// MCP client connection settings.
    #[command(flatten)]
    client: McpClientArgs,
    /// Tool input source.
    #[command(flatten)]
    input: McpToolInputArgs,
    /// Disable schema validation for tool input.
    #[arg(long, action = ArgAction::SetTrue)]
    no_validate: bool,
}

/// Tool input arguments for MCP tool calls.
#[derive(Args, Debug, Clone)]
struct McpToolInputArgs {
    /// JSON input string for the tool payload.
    #[arg(long, value_name = "JSON", conflicts_with = "input")]
    json: Option<String>,
    /// Path to a JSON file containing the tool payload.
    #[arg(long, value_name = "PATH", conflicts_with = "json")]
    input: Option<PathBuf>,
}

/// MCP transport selection for CLI client commands.
#[derive(ValueEnum, Copy, Clone, Debug)]
enum McpTransportArg {
    /// HTTP JSON-RPC transport.
    Http,
    /// SSE JSON-RPC transport.
    Sse,
    /// Stdio JSON-RPC transport.
    Stdio,
}

/// MCP tool name selection for generic tool calls.
#[derive(ValueEnum, Copy, Clone, Debug)]
#[value(rename_all = "snake_case")]
enum McpToolNameArg {
    /// `scenario_define`
    ScenarioDefine,
    /// `scenario_start`
    ScenarioStart,
    /// `scenario_status`
    ScenarioStatus,
    /// `scenario_next`
    ScenarioNext,
    /// `scenario_submit`
    ScenarioSubmit,
    /// `scenario_trigger`
    ScenarioTrigger,
    /// `evidence_query`
    EvidenceQuery,
    /// `runpack_export`
    RunpackExport,
    /// `runpack_verify`
    RunpackVerify,
    /// `providers_list`
    ProvidersList,
    /// `provider_contract_get`
    ProviderContractGet,
    /// `provider_check_schema_get`
    ProviderCheckSchemaGet,
    /// `schemas_register`
    SchemasRegister,
    /// `schemas_list`
    SchemasList,
    /// `schemas_get`
    SchemasGet,
    /// `scenarios_list`
    ScenariosList,
    /// precheck
    Precheck,
    /// `decision_gate_docs_search`
    DecisionGateDocsSearch,
}

/// Arguments for contract generation.
#[derive(Args, Debug)]
struct ContractGenerateCommand {
    /// Output directory for generated artifacts.
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
}

/// Arguments for contract verification.
#[derive(Args, Debug)]
struct ContractCheckCommand {
    /// Output directory containing generated artifacts.
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
}

/// Arguments for SDK generation.
#[derive(Args, Debug)]
struct SdkGenerateCommand {
    /// Path to tooling.json input.
    #[arg(long, value_name = "FILE", default_value = decision_gate_sdk_gen::DEFAULT_TOOLING_PATH)]
    tooling: PathBuf,
    /// Python SDK output file.
    #[arg(long, value_name = "FILE", default_value = "sdks/python/decision_gate/_generated.py")]
    python_out: PathBuf,
    /// TypeScript SDK output file.
    #[arg(long, value_name = "FILE", default_value = "sdks/typescript/src/_generated.ts")]
    typescript_out: PathBuf,
    /// `OpenAPI` output file.
    #[arg(long, value_name = "FILE", default_value = "Docs/generated/openapi/decision-gate.json")]
    openapi_out: PathBuf,
}

/// Arguments for SDK verification.
#[derive(Args, Debug)]
struct SdkCheckCommand {
    /// Path to tooling.json input.
    #[arg(long, value_name = "FILE", default_value = decision_gate_sdk_gen::DEFAULT_TOOLING_PATH)]
    tooling: PathBuf,
    /// Python SDK output file.
    #[arg(long, value_name = "FILE", default_value = "sdks/python/decision_gate/_generated.py")]
    python_out: PathBuf,
    /// TypeScript SDK output file.
    #[arg(long, value_name = "FILE", default_value = "sdks/typescript/src/_generated.ts")]
    typescript_out: PathBuf,
    /// `OpenAPI` output file.
    #[arg(long, value_name = "FILE", default_value = "Docs/generated/openapi/decision-gate.json")]
    openapi_out: PathBuf,
}

/// Supported authoring formats for `ScenarioSpec` inputs.
#[derive(ValueEnum, Copy, Clone, Debug)]
enum AuthoringFormatArg {
    /// Canonical JSON authoring format.
    Json,
    /// Human-friendly RON authoring format.
    Ron,
}

/// Arguments for authoring validation.
#[derive(Args, Debug)]
struct AuthoringValidateCommand {
    /// Path to the `ScenarioSpec` authoring input.
    #[arg(long, value_name = "PATH")]
    input: PathBuf,
    /// Explicit authoring format override.
    #[arg(long, value_enum, value_name = "FORMAT")]
    format: Option<AuthoringFormatArg>,
}

/// Arguments for authoring normalization.
#[derive(Args, Debug)]
struct AuthoringNormalizeCommand {
    /// Path to the `ScenarioSpec` authoring input.
    #[arg(long, value_name = "PATH")]
    input: PathBuf,
    /// Explicit authoring format override.
    #[arg(long, value_enum, value_name = "FORMAT")]
    format: Option<AuthoringFormatArg>,
    /// Output path for canonical JSON (defaults to stdout).
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
}

/// Arguments for config validation.
#[derive(Args, Debug)]
struct ConfigValidateCommand {
    /// Optional config file path (defaults to decision-gate.toml or env override).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
}

/// Arguments for runpack export.
#[derive(Args, Debug)]
struct RunpackExportCommand {
    /// Path to the scenario spec JSON file.
    #[arg(long, value_name = "PATH")]
    spec: PathBuf,
    /// Path to the run state JSON file.
    #[arg(long, value_name = "PATH")]
    state: PathBuf,
    /// Output directory for runpack artifacts.
    #[arg(long, value_name = "DIR")]
    output_dir: PathBuf,
    /// Manifest filename to write inside the output directory.
    #[arg(long, value_name = "FILE", default_value = "runpack.json")]
    manifest_name: String,
    /// Include an offline verification report artifact.
    #[arg(long, action = ArgAction::SetTrue)]
    with_verification: bool,
    /// Override `generated_at` timestamp (unix milliseconds).
    #[arg(long, value_name = "UNIX_MS")]
    generated_at_unix_ms: Option<i64>,
}

/// Arguments for runpack verification.
#[derive(Args, Debug)]
struct RunpackVerifyCommand {
    /// Path to the runpack manifest JSON file.
    #[arg(long, value_name = "PATH")]
    manifest: PathBuf,
    /// Root directory for runpack artifacts (defaults to manifest directory).
    #[arg(long, value_name = "DIR")]
    runpack_dir: Option<PathBuf>,
    /// Output format for the verification report.
    #[arg(long, value_enum, default_value_t = VerifyFormat::Json)]
    format: VerifyFormat,
}

/// Output formats for verification reports.
#[derive(ValueEnum, Copy, Clone, Debug)]
enum VerifyFormat {
    /// Canonical JSON output.
    Json,
    /// Markdown summary output.
    Markdown,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// CLI error wrapper for localized error messages.
#[derive(Debug, Error)]
#[error("{message}")]
struct CliError {
    /// Human-readable error message.
    message: String,
}

impl CliError {
    /// Constructs a new [`CliError`] from a localized message.
    const fn new(message: String) -> Self {
        Self {
            message,
        }
    }
}

/// CLI result alias for fallible operations.
type CliResult<T> = Result<T, CliError>;

// ============================================================================
// SECTION: Entry Point
// ============================================================================

/// CLI entry point returning an exit code.
#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(err) => emit_error(&err.to_string()),
    }
}

/// Executes the CLI command dispatcher.
async fn run() -> CliResult<ExitCode> {
    let cli = Cli::parse();
    let env_lang = std::env::var(LANG_ENV).ok();
    let locale = resolve_locale(cli.lang, env_lang.as_deref())?;
    set_locale(locale);
    if locale != Locale::En {
        write_stderr_line(&t!("i18n.disclaimer.machine_translated"))
            .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    }

    if cli.show_version {
        let version = env!("CARGO_PKG_VERSION");
        write_stdout_line(&t!("main.version", version = version))
            .map_err(|err| CliError::new(output_error("stdout", &err)))?;
        return Ok(ExitCode::SUCCESS);
    }

    let Some(command) = cli.command else {
        show_help()?;
        return Ok(ExitCode::SUCCESS);
    };

    match command {
        Commands::Serve(command) => command_serve(command).await,
        Commands::Runpack {
            command,
        } => command_runpack(command),
        Commands::Authoring {
            command,
        } => command_authoring(command),
        Commands::Config {
            command,
        } => command_config(command),
        Commands::Provider {
            command,
        } => command_provider(command),
        Commands::Schema {
            command,
        } => command_schema(command).await,
        Commands::Docs {
            command,
        } => command_docs(command).await,
        Commands::Interop {
            command,
        } => command_interop(command).await,
        Commands::Mcp {
            command,
        } => command_mcp(command).await,
        Commands::Contract {
            command,
        } => command_contract(command),
        Commands::Sdk {
            command,
        } => command_sdk(command),
    }
}

// ============================================================================
// SECTION: Serve Command
// ============================================================================

/// Executes the `serve` command.
async fn command_serve(command: ServeCommand) -> CliResult<ExitCode> {
    let config = DecisionGateConfig::load(command.config.as_deref())
        .map_err(|err| CliError::new(t!("serve.config.load_failed", error = err)))?;
    let allow_non_loopback = resolve_allow_non_loopback(command.allow_non_loopback)
        .map_err(|err| CliError::new(err.to_string()))?;
    let bind_outcome = enforce_local_only(&config, allow_non_loopback)
        .map_err(|err| CliError::new(err.to_string()))?;
    warn_local_only(&config)?;
    warn_loopback_only_transport(&bind_outcome, allow_non_loopback)?;
    if bind_outcome.network_exposed {
        warn_network_exposure(&bind_outcome)?;
    }

    let server = tokio::task::spawn_blocking(move || McpServer::from_config(config))
        .await
        .map_err(|err| {
            CliError::new(t!("serve.init_failed", error = format!("init join failed: {err}")))
        })?
        .map_err(|err| CliError::new(t!("serve.init_failed", error = err)))?;
    server.serve().await.map_err(|err: decision_gate_mcp::server::McpServerError| {
        CliError::new(t!("serve.failed", error = err))
    })?;

    Ok(ExitCode::SUCCESS)
}

/// Emits local-only warnings for the MCP server.
fn warn_local_only(config: &DecisionGateConfig) -> CliResult<()> {
    let auth_mode = config.server.auth.as_ref().map_or(ServerAuthMode::LocalOnly, |auth| auth.mode);
    if auth_mode != ServerAuthMode::LocalOnly {
        return Ok(());
    }
    write_stderr_line(&t!("serve.warn.local_only_auth"))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    Ok(())
}

/// Warns when HTTP/SSE are bound to loopback only.
fn warn_loopback_only_transport(outcome: &BindOutcome, allow_non_loopback: bool) -> CliResult<()> {
    if !matches!(outcome.transport, ServerTransport::Http | ServerTransport::Sse) {
        return Ok(());
    }
    let Some(addr) = outcome.bind_addr else {
        return Ok(());
    };
    if !addr.ip().is_loopback() || allow_non_loopback {
        return Ok(());
    }
    write_stderr_line(&t!("serve.warn.loopback_only_transport", env = ALLOW_NON_LOOPBACK_ENV))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    Ok(())
}

/// Emits a security warning banner when the server is network-exposed.
fn warn_network_exposure(outcome: &BindOutcome) -> CliResult<()> {
    let Some(addr) = outcome.bind_addr else {
        return Ok(());
    };
    let enabled = t!("serve.warn.network.enabled");
    let disabled = t!("serve.warn.network.disabled");
    let audit_status = if outcome.audit_enabled { enabled.clone() } else { disabled.clone() };
    let rate_limit_status = if outcome.rate_limit_enabled { enabled } else { disabled };
    let tls_status = outcome.tls.as_ref().map_or_else(
        || t!("serve.warn.network.tls_disabled"),
        |tls| {
            let client_cert = if tls.require_client_cert {
                t!("serve.warn.network.required")
            } else {
                t!("serve.warn.network.not_required")
            };
            let client_ca = if tls.client_ca_path.is_some() {
                t!("serve.warn.network.present")
            } else {
                t!("serve.warn.network.missing")
            };
            t!("serve.warn.network.tls_enabled", client_cert = client_cert, client_ca = client_ca)
        },
    );
    let auth_mode = match outcome.auth_mode {
        ServerAuthMode::LocalOnly => "local_only",
        ServerAuthMode::BearerToken => "bearer_token",
        ServerAuthMode::Mtls => "mtls",
    };
    write_stderr_line(&t!("serve.warn.network.header"))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    write_stderr_line(&t!("serve.warn.network.bind", bind = addr))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    write_stderr_line(&t!("serve.warn.network.auth", mode = auth_mode))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    write_stderr_line(&t!("serve.warn.network.tls", tls = tls_status))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    write_stderr_line(&t!("serve.warn.network.audit", status = audit_status))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    write_stderr_line(&t!("serve.warn.network.rate_limit", status = rate_limit_status))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    write_stderr_line(&t!("serve.warn.network.footer"))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    Ok(())
}

/// Emits the top-level help message for the CLI.
fn show_help() -> CliResult<()> {
    let mut command = Cli::command();
    command.print_help().map_err(|err| CliError::new(output_error("stdout", &err)))?;
    write_stdout_line("").map_err(|err| CliError::new(output_error("stdout", &err)))?;
    Ok(())
}

// ============================================================================
// SECTION: Runpack Commands
// ============================================================================

/// Dispatches runpack subcommands.
fn command_runpack(command: RunpackCommand) -> CliResult<ExitCode> {
    match command {
        RunpackCommand::Export(command) => command_runpack_export(&command),
        RunpackCommand::Verify(command) => command_runpack_verify(command),
    }
}

// ============================================================================
// SECTION: Authoring Commands
// ============================================================================

/// Dispatches authoring subcommands.
fn command_authoring(command: AuthoringCommand) -> CliResult<ExitCode> {
    match command {
        AuthoringCommand::Validate(command) => command_authoring_validate(&command),
        AuthoringCommand::Normalize(command) => command_authoring_normalize(&command),
    }
}

// ============================================================================
// SECTION: Config Commands
// ============================================================================

/// Dispatches config subcommands.
fn command_config(command: ConfigCommand) -> CliResult<ExitCode> {
    match command {
        ConfigCommand::Validate(command) => command_config_validate(&command),
    }
}

/// Executes the config validation command.
fn command_config_validate(command: &ConfigValidateCommand) -> CliResult<ExitCode> {
    let _config = DecisionGateConfig::load(command.config.as_deref())
        .map_err(|err| CliError::new(t!("config.load_failed", error = err)))?;
    write_stdout_line(&t!("config.validate.ok"))
        .map_err(|err| CliError::new(output_error("stdout", &err)))?;
    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// SECTION: Provider Discovery Commands
// ============================================================================

/// Dispatches provider discovery subcommands.
fn command_provider(command: ProviderCommand) -> CliResult<ExitCode> {
    match command {
        ProviderCommand::Contract {
            command,
        } => match command {
            ProviderContractCommand::Get(command) => command_provider_contract_get(&command),
        },
        ProviderCommand::CheckSchema {
            command,
        } => match command {
            ProviderCheckSchemaCommand::Get(command) => command_provider_check_schema_get(&command),
        },
        ProviderCommand::List(command) => command_provider_list(&command),
    }
}

/// Executes `provider contract get`.
fn command_provider_contract_get(command: &ProviderContractGetCommand) -> CliResult<ExitCode> {
    let config = DecisionGateConfig::load(command.config.as_deref())
        .map_err(|err| CliError::new(t!("config.load_failed", error = err)))?;
    if !config.provider_discovery.is_allowed(&command.provider) {
        return Err(CliError::new(t!("provider.discovery.denied", provider = command.provider)));
    }
    let registry = CapabilityRegistry::from_config(&config)
        .map_err(|err| CliError::new(t!("provider.discovery.failed", error = err)))?;
    let view = registry
        .provider_contract_view(&command.provider)
        .map_err(|err| CliError::new(t!("provider.discovery.failed", error = err)))?;
    let response = decision_gate_mcp::tools::ProviderContractGetResponse {
        provider_id: view.provider_id,
        contract: view.contract,
        contract_hash: view.contract_hash,
        source: view.source,
        version: view.version,
    };
    write_canonical_json(&response, config.provider_discovery.max_response_bytes)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `provider check-schema get`.
fn command_provider_check_schema_get(
    command: &ProviderCheckSchemaGetCommand,
) -> CliResult<ExitCode> {
    let config = DecisionGateConfig::load(command.config.as_deref())
        .map_err(|err| CliError::new(t!("config.load_failed", error = err)))?;
    if !config.provider_discovery.is_allowed(&command.provider) {
        return Err(CliError::new(t!("provider.discovery.denied", provider = command.provider)));
    }
    let registry = CapabilityRegistry::from_config(&config)
        .map_err(|err| CliError::new(t!("provider.discovery.failed", error = err)))?;
    let view = registry
        .check_schema_view(&command.provider, &command.check_id)
        .map_err(|err| CliError::new(t!("provider.discovery.failed", error = err)))?;
    let response = decision_gate_mcp::tools::ProviderCheckSchemaGetResponse {
        provider_id: view.provider_id,
        check_id: view.check_id,
        params_required: view.params_required,
        params_schema: view.params_schema,
        result_schema: view.result_schema,
        allowed_comparators: view.allowed_comparators,
        determinism: view.determinism,
        anchor_types: view.anchor_types,
        content_types: view.content_types,
        examples: view.examples,
        contract_hash: view.contract_hash,
    };
    write_canonical_json(&response, config.provider_discovery.max_response_bytes)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `provider list`.
fn command_provider_list(command: &ProviderListCommand) -> CliResult<ExitCode> {
    let config = DecisionGateConfig::load(command.config.as_deref())
        .map_err(|err| CliError::new(t!("config.load_failed", error = err)))?;
    let registry = CapabilityRegistry::from_config(&config)
        .map_err(|err| CliError::new(t!("provider.discovery.failed", error = err)))?;
    let mut providers = Vec::new();
    for (provider_id, checks) in registry.list_providers() {
        let view = registry
            .provider_contract_view(&provider_id)
            .map_err(|err| CliError::new(t!("provider.discovery.failed", error = err)))?;
        let transport = match view.source {
            decision_gate_mcp::capabilities::ProviderContractSource::Builtin => {
                decision_gate_mcp::tools::ProviderTransport::Builtin
            }
            decision_gate_mcp::capabilities::ProviderContractSource::File => {
                decision_gate_mcp::tools::ProviderTransport::Mcp
            }
        };
        providers.push(decision_gate_mcp::tools::ProviderSummary {
            provider_id,
            transport,
            checks,
        });
    }
    providers.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    let response = decision_gate_mcp::tools::ProvidersListResponse {
        providers,
    };

    match command.format {
        ProviderListFormat::Json => {
            write_canonical_json(&response, config.provider_discovery.max_response_bytes)?;
        }
        ProviderListFormat::Text => {
            render_provider_list_text(&response)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// SECTION: Schema Registry Commands
// ============================================================================

/// Dispatches schema registry subcommands.
async fn command_schema(command: SchemaCommand) -> CliResult<ExitCode> {
    match command {
        SchemaCommand::Register(command) => command_schema_register(command).await,
        SchemaCommand::List(command) => command_schema_list(command).await,
        SchemaCommand::Get(command) => command_schema_get(command).await,
    }
}

/// Executes `schema register`.
async fn command_schema_register(command: SchemaRegisterCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let input = read_mcp_tool_input(&command.input)?;
    if !command.no_validate {
        validate_mcp_tool_input(decision_gate_core::ToolName::SchemasRegister, &input)?;
    }
    let result = client
        .call_tool(decision_gate_core::ToolName::SchemasRegister, input)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    write_json_value(&result)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `schema list`.
async fn command_schema_list(command: SchemaListCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let tenant_id = parse_tenant_id(command.tenant_id)?;
    let namespace_id = parse_namespace_id(command.namespace_id)?;
    let mut payload = serde_json::Map::new();
    payload
        .insert("tenant_id".to_string(), Value::Number(serde_json::Number::from(tenant_id.get())));
    payload.insert(
        "namespace_id".to_string(),
        Value::Number(serde_json::Number::from(namespace_id.get())),
    );
    if let Some(cursor) = &command.cursor {
        payload.insert("cursor".to_string(), Value::String(cursor.clone()));
    }
    if let Some(limit) = command.limit {
        payload.insert("limit".to_string(), Value::Number(serde_json::Number::from(limit as u64)));
    }
    let input = Value::Object(payload);
    validate_mcp_tool_input(decision_gate_core::ToolName::SchemasList, &input)?;
    let result = client
        .call_tool(decision_gate_core::ToolName::SchemasList, input)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    write_json_value(&result)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `schema get`.
async fn command_schema_get(command: SchemaGetCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let tenant_id = parse_tenant_id(command.tenant_id)?;
    let namespace_id = parse_namespace_id(command.namespace_id)?;
    let request = decision_gate_mcp::tools::SchemasGetRequest {
        tenant_id,
        namespace_id,
        schema_id: DataShapeId::from(command.schema_id.as_str()),
        version: DataShapeVersion::from(command.version.as_str()),
    };
    let input = serde_json::to_value(&request)
        .map_err(|err| CliError::new(t!("mcp.client.input_parse_failed", error = err)))?;
    validate_mcp_tool_input(decision_gate_core::ToolName::SchemasGet, &input)?;
    let result = client
        .call_tool(decision_gate_core::ToolName::SchemasGet, input)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    write_json_value(&result)?;
    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// SECTION: Docs Commands
// ============================================================================

/// Dispatches docs subcommands.
async fn command_docs(command: DocsCommand) -> CliResult<ExitCode> {
    match command {
        DocsCommand::Search(command) => command_docs_search(command).await,
        DocsCommand::List(command) => command_docs_list(command).await,
        DocsCommand::Read(command) => command_docs_read(command).await,
    }
}

/// Executes `docs search`.
async fn command_docs_search(command: DocsSearchCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let mut payload = serde_json::Map::new();
    payload.insert("query".to_string(), Value::String(command.query.unwrap_or_default()));
    if let Some(max_sections) = command.max_sections {
        payload.insert(
            "max_sections".to_string(),
            Value::Number(serde_json::Number::from(max_sections)),
        );
    }
    let input = Value::Object(payload);
    validate_mcp_tool_input(decision_gate_core::ToolName::DecisionGateDocsSearch, &input)?;
    let result = client
        .call_tool(decision_gate_core::ToolName::DecisionGateDocsSearch, input)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    write_json_value(&result)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `docs list`.
async fn command_docs_list(command: DocsListCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let resources: Vec<ResourceMetadata> = client
        .list_resources()
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    let output = serde_json::json!({ "resources": resources });
    write_json_value(&output)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `docs read`.
async fn command_docs_read(command: DocsReadCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let contents: Vec<ResourceContent> = client
        .read_resource(&command.uri)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    let output = serde_json::json!({ "contents": contents });
    write_json_value(&output)?;
    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// SECTION: Interop Commands
// ============================================================================

/// Dispatches interop subcommands.
async fn command_interop(command: InteropCommand) -> CliResult<ExitCode> {
    match command {
        InteropCommand::Eval(command) => command_interop_eval(command).await,
    }
}

/// Executes the interop evaluation command.
async fn command_interop_eval(command: InteropEvalCommand) -> CliResult<ExitCode> {
    let spec_label = t!("interop.kind.spec");
    let run_config_label = t!("interop.kind.run_config");
    let trigger_label = t!("interop.kind.trigger");

    let spec: ScenarioSpec = read_interop_json(&command.spec, &spec_label, MAX_INTEROP_SPEC_BYTES)?;
    spec.validate().map_err(|err| {
        CliError::new(t!("interop.spec_failed", path = command.spec.display(), error = err))
    })?;
    let run_config: RunConfig =
        read_interop_json(&command.run_config, &run_config_label, MAX_INTEROP_RUN_CONFIG_BYTES)?;
    let trigger: TriggerEvent =
        read_interop_json(&command.trigger, &trigger_label, MAX_INTEROP_TRIGGER_BYTES)?;

    validate_inputs(&spec, &run_config, &trigger)
        .map_err(|err| CliError::new(t!("interop.input_invalid", error = err)))?;

    let started_at = resolve_interop_timestamp(
        command.started_at_unix_ms,
        command.started_at_logical,
        trigger.time,
        "started_at",
    )?;
    let status_requested_at = resolve_interop_timestamp(
        command.status_requested_at_unix_ms,
        command.status_requested_at_logical,
        trigger.time,
        "status_requested_at",
    )?;

    let auth = resolve_auth(&command.client)?;
    let mut stdio_env = parse_stdio_env(&command.client.stdio_env)?;
    if let Some(path) = &command.client.stdio_config {
        stdio_env.push(stdio_config_env(path));
    }
    let timeout = Duration::from_millis(command.client.timeout_ms);
    let report = run_interop(InteropConfig {
        transport: command.client.transport.into(),
        endpoint: command.client.endpoint.clone(),
        stdio_command: command.client.stdio_command.clone(),
        stdio_args: command.client.stdio_args.clone(),
        stdio_env,
        spec,
        run_config,
        trigger,
        started_at,
        status_requested_at,
        issue_entry_packets: command.issue_entry_packets,
        bearer_token: auth.bearer_token,
        client_subject: auth.client_subject,
        timeout,
    })
    .await
    .map_err(|err| CliError::new(t!("interop.execution_failed", error = err)))?;

    let mut report_bytes = serde_jcs::to_vec(&report)
        .map_err(|err| CliError::new(t!("interop.report.serialize_failed", error = err)))?;
    report_bytes.push(b'\n');

    if let Some(output) = &command.output {
        fs::write(output, &report_bytes).map_err(|err| {
            CliError::new(t!("interop.report.write_failed", path = output.display(), error = err))
        })?;
    } else {
        write_stdout_bytes(&report_bytes)
            .map_err(|err| CliError::new(output_error("stdout", &err)))?;
    }

    if let Some(expected) = command.expect_status {
        let expected_status = run_status_from_arg(expected);
        if report.status.status != expected_status {
            return Err(CliError::new(t!(
                "interop.expect_status_mismatch",
                expected = format_run_status(expected_status),
                actual = format_run_status(report.status.status)
            )));
        }
    }

    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// SECTION: MCP Client Commands
// ============================================================================

/// Dispatches MCP client subcommands.
async fn command_mcp(command: McpCommand) -> CliResult<ExitCode> {
    match command {
        McpCommand::Tools {
            command,
        } => match command {
            McpToolsCommand::List(command) => command_mcp_tools_list(command).await,
            McpToolsCommand::Call(command) => command_mcp_tools_call(command).await,
        },
        McpCommand::Resources {
            command,
        } => match command {
            McpResourcesCommand::List(command) => command_mcp_resources_list(command).await,
            McpResourcesCommand::Read(command) => command_mcp_resources_read(command).await,
        },
        McpCommand::Tool {
            command,
        } => command_mcp_tool(command).await,
    }
}

/// Executes `mcp tools list`.
async fn command_mcp_tools_list(command: McpToolsListCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let tools = client
        .list_tools()
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    let output = serde_json::json!({ "tools": tools });
    write_json_value(&output)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `mcp tools call`.
async fn command_mcp_tools_call(command: McpToolCallCommand) -> CliResult<ExitCode> {
    let tool = decision_gate_core::ToolName::from(command.tool);
    command_mcp_tool_with_args(&command.client, tool, &command.input, command.no_validate).await
}

/// Executes `mcp resources list`.
async fn command_mcp_resources_list(command: McpResourcesListCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let resources: Vec<ResourceMetadata> = client
        .list_resources()
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    let output = serde_json::json!({ "resources": resources });
    write_json_value(&output)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes `mcp resources read`.
async fn command_mcp_resources_read(command: McpResourcesReadCommand) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(&command.client)?;
    let contents: Vec<ResourceContent> = client
        .read_resource(&command.uri)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    let output = serde_json::json!({ "contents": contents });
    write_json_value(&output)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes a typed MCP tool wrapper.
async fn command_mcp_tool(command: McpToolCommand) -> CliResult<ExitCode> {
    let (tool, args) = match command {
        McpToolCommand::ScenarioDefine(args) => {
            (decision_gate_core::ToolName::ScenarioDefine, args)
        }
        McpToolCommand::ScenarioStart(args) => (decision_gate_core::ToolName::ScenarioStart, args),
        McpToolCommand::ScenarioStatus(args) => {
            (decision_gate_core::ToolName::ScenarioStatus, args)
        }
        McpToolCommand::ScenarioNext(args) => (decision_gate_core::ToolName::ScenarioNext, args),
        McpToolCommand::ScenarioSubmit(args) => {
            (decision_gate_core::ToolName::ScenarioSubmit, args)
        }
        McpToolCommand::ScenarioTrigger(args) => {
            (decision_gate_core::ToolName::ScenarioTrigger, args)
        }
        McpToolCommand::ScenariosList(args) => (decision_gate_core::ToolName::ScenariosList, args),
        McpToolCommand::EvidenceQuery(args) => (decision_gate_core::ToolName::EvidenceQuery, args),
        McpToolCommand::RunpackExport(args) => (decision_gate_core::ToolName::RunpackExport, args),
        McpToolCommand::RunpackVerify(args) => (decision_gate_core::ToolName::RunpackVerify, args),
        McpToolCommand::ProvidersList(args) => (decision_gate_core::ToolName::ProvidersList, args),
        McpToolCommand::ProviderContractGet(args) => {
            (decision_gate_core::ToolName::ProviderContractGet, args)
        }
        McpToolCommand::ProviderCheckSchemaGet(args) => {
            (decision_gate_core::ToolName::ProviderCheckSchemaGet, args)
        }
        McpToolCommand::SchemasRegister(args) => {
            (decision_gate_core::ToolName::SchemasRegister, args)
        }
        McpToolCommand::SchemasList(args) => (decision_gate_core::ToolName::SchemasList, args),
        McpToolCommand::SchemasGet(args) => (decision_gate_core::ToolName::SchemasGet, args),
        McpToolCommand::Precheck(args) => (decision_gate_core::ToolName::Precheck, args),
        McpToolCommand::DecisionGateDocsSearch(args) => {
            (decision_gate_core::ToolName::DecisionGateDocsSearch, args)
        }
    };
    command_mcp_tool_with_args(&args.client, tool, &args.input, args.no_validate).await
}

/// Executes an MCP tool call with shared client/input handling.
async fn command_mcp_tool_with_args(
    client_args: &McpClientArgs,
    tool: decision_gate_core::ToolName,
    input_args: &McpToolInputArgs,
    no_validate: bool,
) -> CliResult<ExitCode> {
    let mut client = build_mcp_client(client_args)?;
    let input = read_mcp_tool_input(input_args)?;
    if !no_validate {
        validate_mcp_tool_input(tool, &input)?;
    }
    let result = client
        .call_tool(tool, input)
        .await
        .map_err(|err| CliError::new(t!("mcp.client.failed", error = err)))?;
    write_json_value(&result)?;
    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// SECTION: Contract + SDK Commands
// ============================================================================

/// Dispatches contract commands.
fn command_contract(command: ContractCommand) -> CliResult<ExitCode> {
    match command {
        ContractCommand::Generate(command) => command_contract_generate(command),
        ContractCommand::Check(command) => command_contract_check(command),
    }
}

/// Dispatches SDK commands.
fn command_sdk(command: SdkCommand) -> CliResult<ExitCode> {
    match command {
        SdkCommand::Generate(command) => command_sdk_generate(&command),
        SdkCommand::Check(command) => command_sdk_check(&command),
    }
}

/// Executes contract generation.
fn command_contract_generate(command: ContractGenerateCommand) -> CliResult<ExitCode> {
    let output_dir =
        command.out.unwrap_or_else(decision_gate_contract::ContractBuilder::default_output_dir);
    let builder = decision_gate_contract::ContractBuilder::new(output_dir.clone());
    builder
        .write_to(&output_dir)
        .map_err(|err| CliError::new(t!("contract.generate.failed", error = err)))?;
    config::write_config_docs(None)
        .map_err(|err| CliError::new(t!("contract.generate.failed", error = err)))?;
    Ok(ExitCode::SUCCESS)
}

/// Executes contract verification.
fn command_contract_check(command: ContractCheckCommand) -> CliResult<ExitCode> {
    let output_dir =
        command.out.unwrap_or_else(decision_gate_contract::ContractBuilder::default_output_dir);
    let builder = decision_gate_contract::ContractBuilder::new(output_dir.clone());
    builder
        .verify_output(&output_dir)
        .map_err(|err| CliError::new(t!("contract.check.failed", error = err)))?;
    config::verify_config_docs(None)
        .map_err(|err| CliError::new(t!("contract.check.failed", error = err)))?;
    Ok(ExitCode::SUCCESS)
}

/// Executes SDK generation.
fn command_sdk_generate(command: &SdkGenerateCommand) -> CliResult<ExitCode> {
    let generator = decision_gate_sdk_gen::SdkGenerator::load(&command.tooling)
        .map_err(|err| CliError::new(t!("sdk.generate.failed", error = err)))?;
    let python = generator
        .generate_python()
        .map_err(|err| CliError::new(t!("sdk.generate.failed", error = err)))?;
    let typescript = generator
        .generate_typescript()
        .map_err(|err| CliError::new(t!("sdk.generate.failed", error = err)))?;
    let openapi = generator
        .generate_openapi()
        .map_err(|err| CliError::new(t!("sdk.generate.failed", error = err)))?;
    write_sdk_output(&command.python_out, &python)?;
    write_sdk_output(&command.typescript_out, &typescript)?;
    write_sdk_output(&command.openapi_out, &openapi)?;
    Ok(ExitCode::SUCCESS)
}

/// Executes SDK verification.
fn command_sdk_check(command: &SdkCheckCommand) -> CliResult<ExitCode> {
    let generator = decision_gate_sdk_gen::SdkGenerator::load(&command.tooling)
        .map_err(|err| CliError::new(t!("sdk.check.failed", error = err)))?;
    check_sdk_output(
        &command.python_out,
        &generator
            .generate_python()
            .map_err(|err| CliError::new(t!("sdk.check.failed", error = err)))?,
    )?;
    check_sdk_output(
        &command.typescript_out,
        &generator
            .generate_typescript()
            .map_err(|err| CliError::new(t!("sdk.check.failed", error = err)))?,
    )?;
    check_sdk_output(
        &command.openapi_out,
        &generator
            .generate_openapi()
            .map_err(|err| CliError::new(t!("sdk.check.failed", error = err)))?,
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Writes generated SDK output to disk with a temporary file.
fn write_sdk_output(path: &Path, contents: &str) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CliError::new(t!("sdk.io.failed", error = err)))?;
    }
    let temp_path = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp_path)
        .map_err(|err| CliError::new(t!("sdk.io.failed", error = err)))?;
    file.write_all(contents.as_bytes())
        .map_err(|err| CliError::new(t!("sdk.io.failed", error = err)))?;
    file.sync_all().map_err(|err| CliError::new(t!("sdk.io.failed", error = err)))?;
    fs::rename(&temp_path, path).map_err(|err| CliError::new(t!("sdk.io.failed", error = err)))?;
    Ok(())
}

/// Checks generated SDK output against the on-disk file.
fn check_sdk_output(path: &Path, contents: &str) -> CliResult<()> {
    let existing =
        fs::read_to_string(path).map_err(|err| CliError::new(t!("sdk.io.failed", error = err)))?;
    if existing != contents {
        return Err(CliError::new(t!("sdk.check.drift", path = path.display())));
    }
    Ok(())
}

/// Executes the authoring validation command.
fn command_authoring_validate(command: &AuthoringValidateCommand) -> CliResult<ExitCode> {
    let normalized = normalize_authoring_input(&command.input, command.format)?;
    let summary = t!(
        "authoring.validate.ok",
        scenario_id = normalized.spec.scenario_id.as_str(),
        spec_hash = format_hash_digest(&normalized.spec_hash)
    );
    write_stdout_line(&summary).map_err(|err| CliError::new(output_error("stdout", &err)))?;
    Ok(ExitCode::SUCCESS)
}

/// Executes the authoring normalization command.
fn command_authoring_normalize(command: &AuthoringNormalizeCommand) -> CliResult<ExitCode> {
    let normalized = normalize_authoring_input(&command.input, command.format)?;
    let summary = t!(
        "authoring.validate.ok",
        scenario_id = normalized.spec.scenario_id.as_str(),
        spec_hash = format_hash_digest(&normalized.spec_hash)
    );

    if let Some(output) = &command.output {
        fs::write(output, &normalized.canonical_json).map_err(|err| {
            CliError::new(t!(
                "authoring.normalize.write_failed",
                path = output.display(),
                error = err
            ))
        })?;
        write_stdout_line(&t!("authoring.normalize.ok", path = output.display()))
            .map_err(|err| CliError::new(output_error("stdout", &err)))?;
        write_stdout_line(&summary).map_err(|err| CliError::new(output_error("stdout", &err)))?;
        return Ok(ExitCode::SUCCESS);
    }

    write_stdout_bytes(&normalized.canonical_json)
        .map_err(|err| CliError::new(output_error("stdout", &err)))?;
    write_stderr_line(&summary).map_err(|err| CliError::new(output_error("stderr", &err)))?;
    Ok(ExitCode::SUCCESS)
}

/// Executes the runpack export command.
fn command_runpack_export(command: &RunpackExportCommand) -> CliResult<ExitCode> {
    let spec_label = t!("runpack.export.kind.spec");
    let state_label = t!("runpack.export.kind.state");
    let spec: ScenarioSpec = read_export_json(&command.spec, &spec_label, MAX_SPEC_BYTES)?;
    spec.validate().map_err(|err| {
        CliError::new(t!("runpack.export.spec_failed", path = command.spec.display(), error = err))
    })?;
    let state: RunState = read_export_json(&command.state, &state_label, MAX_RUN_STATE_BYTES)?;
    let generated_at = resolve_generated_at(command.generated_at_unix_ms)?;

    fs::create_dir_all(&command.output_dir).map_err(|err| {
        CliError::new(t!(
            "runpack.export.output_dir_failed",
            path = command.output_dir.display(),
            error = err
        ))
    })?;

    let manifest_path = command.output_dir.join(&command.manifest_name);
    let mut sink = FileArtifactSink::new(command.output_dir.clone(), &command.manifest_name)
        .map_err(|err| {
            CliError::new(t!(
                "runpack.export.sink_failed",
                path = command.output_dir.display(),
                error = err
            ))
        })?;
    let builder = RunpackBuilder::default();
    if command.with_verification {
        let reader = FileArtifactReader::new(command.output_dir.clone()).map_err(|err| {
            CliError::new(t!(
                "runpack.verify.reader_failed",
                path = command.output_dir.display(),
                error = err
            ))
        })?;
        let (_manifest, report) = builder
            .build_with_verification(&mut sink, &reader, &spec, &state, generated_at)
            .map_err(|err| CliError::new(t!("runpack.export.build_failed", error = err)))?;
        let status = format_verification_status(report.status);
        write_stdout_line(&t!("runpack.export.verification_status", status = status))
            .map_err(|err| CliError::new(output_error("stdout", &err)))?;
    } else {
        let _manifest = builder
            .build(&mut sink, &spec, &state, generated_at)
            .map_err(|err| CliError::new(t!("runpack.export.build_failed", error = err)))?;
    }

    write_stdout_line(&t!("runpack.export.ok", path = manifest_path.display()))
        .map_err(|err| CliError::new(output_error("stdout", &err)))?;
    Ok(ExitCode::SUCCESS)
}

/// Executes the runpack verification command.
fn command_runpack_verify(command: RunpackVerifyCommand) -> CliResult<ExitCode> {
    let manifest: RunpackManifest = read_manifest_json(&command.manifest, MAX_MANIFEST_BYTES)?;
    let runpack_dir = resolve_runpack_dir(&command.manifest, command.runpack_dir)?;
    let reader = FileArtifactReader::new(runpack_dir.clone()).map_err(|err| {
        CliError::new(t!("runpack.verify.reader_failed", path = runpack_dir.display(), error = err))
    })?;

    let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
    let report = verifier
        .verify_manifest(&reader, &manifest)
        .map_err(|err| CliError::new(t!("runpack.verify.failed", error = err)))?;

    let output = render_verification_report(command.format, &report)?;
    write_stdout_line(&output).map_err(|err| CliError::new(output_error("stdout", &err)))?;

    let exit_code = match report.status {
        VerificationStatus::Pass => ExitCode::SUCCESS,
        VerificationStatus::Fail => ExitCode::FAILURE,
    };

    Ok(exit_code)
}

// ============================================================================
// SECTION: Runpack Helpers
// ============================================================================

/// Errors returned by bounded file reads.
#[derive(Debug)]
enum ReadLimitError {
    /// File I/O failure.
    Io(std::io::Error),
    /// File size exceeds the configured limit.
    TooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Allowed limit in bytes.
        limit: usize,
    },
}

/// Reads a file from disk while enforcing a hard size limit.
fn read_bytes_with_limit(path: &Path, max_bytes: usize) -> Result<Vec<u8>, ReadLimitError> {
    let file = File::open(path).map_err(ReadLimitError::Io)?;
    let metadata = file.metadata().map_err(ReadLimitError::Io)?;
    let size = metadata.len();
    let limit = u64::try_from(max_bytes).map_err(|_| ReadLimitError::TooLarge {
        size,
        limit: max_bytes,
    })?;
    if size > limit {
        return Err(ReadLimitError::TooLarge {
            size,
            limit: max_bytes,
        });
    }

    let read_limit = limit.saturating_add(1);
    let mut limited = file.take(read_limit);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes).map_err(ReadLimitError::Io)?;
    if bytes.len() > max_bytes {
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        return Err(ReadLimitError::TooLarge {
            size: actual,
            limit: max_bytes,
        });
    }
    Ok(bytes)
}

/// Reads a JSON file for runpack export inputs.
fn read_export_json<T: DeserializeOwned>(
    path: &Path,
    kind: &str,
    max_bytes: usize,
) -> CliResult<T> {
    let bytes = read_bytes_with_limit(path, max_bytes).map_err(|err| match err {
        ReadLimitError::Io(err) => CliError::new(t!(
            "runpack.export.read_failed",
            kind = kind,
            path = path.display(),
            error = err
        )),
        ReadLimitError::TooLarge {
            size,
            limit,
        } => CliError::new(t!(
            "input.read_too_large",
            kind = kind,
            path = path.display(),
            size = size,
            limit = limit
        )),
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        CliError::new(t!(
            "runpack.export.parse_failed",
            kind = kind,
            path = path.display(),
            error = err
        ))
    })
}

/// Reads a JSON file for interop inputs.
fn read_interop_json<T: DeserializeOwned>(
    path: &Path,
    kind: &str,
    max_bytes: usize,
) -> CliResult<T> {
    let bytes = read_bytes_with_limit(path, max_bytes).map_err(|err| match err {
        ReadLimitError::Io(err) => CliError::new(t!(
            "interop.read_failed",
            kind = kind,
            path = path.display(),
            error = err
        )),
        ReadLimitError::TooLarge {
            size,
            limit,
        } => CliError::new(t!(
            "input.read_too_large",
            kind = kind,
            path = path.display(),
            size = size,
            limit = limit
        )),
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        CliError::new(t!("interop.parse_failed", kind = kind, path = path.display(), error = err))
    })
}

/// Reads a JSON manifest file for runpack verification.
fn read_manifest_json<T: DeserializeOwned>(path: &Path, max_bytes: usize) -> CliResult<T> {
    let bytes = read_bytes_with_limit(path, max_bytes).map_err(|err| match err {
        ReadLimitError::Io(err) => {
            CliError::new(t!("runpack.verify.read_failed", path = path.display(), error = err))
        }
        ReadLimitError::TooLarge {
            size,
            limit,
        } => CliError::new(t!(
            "input.read_too_large",
            kind = t!("runpack.verify.kind.manifest"),
            path = path.display(),
            size = size,
            limit = limit
        )),
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        CliError::new(t!("runpack.verify.parse_failed", path = path.display(), error = err))
    })
}

/// Resolves the runpack directory for verification.
fn resolve_runpack_dir(manifest: &Path, override_dir: Option<PathBuf>) -> CliResult<PathBuf> {
    if let Some(dir) = override_dir {
        return Ok(dir);
    }

    if let Some(parent) = manifest.parent() {
        return Ok(parent.to_path_buf());
    }

    std::env::current_dir().map_err(|err| {
        CliError::new(t!("runpack.verify.reader_failed", path = manifest.display(), error = err))
    })
}

/// Determines the `generated_at` timestamp for runpack export.
fn resolve_generated_at(override_unix_ms: Option<i64>) -> CliResult<Timestamp> {
    if let Some(value) = override_unix_ms {
        if value < 0 {
            return Err(CliError::new(t!("runpack.export.time.negative")));
        }
        return Ok(Timestamp::UnixMillis(value));
    }

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| CliError::new(t!("runpack.export.time.system_failed", error = err)))?;
    let millis = i64::try_from(duration.as_millis())
        .map_err(|_| CliError::new(t!("runpack.export.time.overflow")))?;
    Ok(Timestamp::UnixMillis(millis))
}

/// Renders a verification report in the requested format.
fn render_verification_report(
    format: VerifyFormat,
    report: &VerificationReport,
) -> CliResult<String> {
    match format {
        VerifyFormat::Json => {
            let bytes = serde_jcs::to_vec(report)
                .map_err(|err| CliError::new(t!("runpack.verify.failed", error = err)))?;
            String::from_utf8(bytes)
                .map_err(|err| CliError::new(t!("runpack.verify.failed", error = err)))
        }
        VerifyFormat::Markdown => Ok(render_verification_markdown(report)),
    }
}

/// Formats a verification report as markdown.
fn render_verification_markdown(report: &VerificationReport) -> String {
    let mut output = String::new();
    output.push_str(&t!("runpack.verify.md.header"));
    output.push('\n');
    output.push('\n');
    output.push_str(&t!(
        "runpack.verify.md.status",
        status = format_verification_status(report.status)
    ));
    output.push('\n');
    output.push_str(&t!("runpack.verify.md.checked", count = report.checked_files));
    output.push('\n');
    output.push('\n');
    output.push_str(&t!("runpack.verify.md.errors_header"));
    output.push('\n');

    if report.errors.is_empty() {
        output.push_str(&t!("runpack.verify.md.no_errors"));
        output.push('\n');
        return output;
    }

    for error in &report.errors {
        output.push_str(&t!("runpack.verify.md.error_line", error = error));
        output.push('\n');
    }

    output
}

/// Converts verification status to localized text.
fn format_verification_status(status: VerificationStatus) -> String {
    match status {
        VerificationStatus::Pass => t!("runpack.verify.status.pass"),
        VerificationStatus::Fail => t!("runpack.verify.status.fail"),
    }
}

/// Resolves an interop timestamp from CLI inputs and fallback values.
fn resolve_interop_timestamp(
    unix_ms: Option<i64>,
    logical: Option<u64>,
    fallback: Timestamp,
    label: &str,
) -> CliResult<Timestamp> {
    match (unix_ms, logical) {
        (Some(_), Some(_)) => Err(CliError::new(t!("interop.timestamp.conflict", label = label))),
        (Some(value), None) => {
            if value < 0 {
                return Err(CliError::new(t!("interop.timestamp.negative", label = label)));
            }
            Ok(Timestamp::UnixMillis(value))
        }
        (None, Some(value)) => Ok(Timestamp::Logical(value)),
        (None, None) => Ok(fallback),
    }
}

/// Maps CLI run status selections to core run status values.
const fn run_status_from_arg(status: ExpectedRunStatusArg) -> RunStatus {
    match status {
        ExpectedRunStatusArg::Active => RunStatus::Active,
        ExpectedRunStatusArg::Completed => RunStatus::Completed,
        ExpectedRunStatusArg::Failed => RunStatus::Failed,
    }
}

/// Formats run status values for interop output.
fn format_run_status(status: RunStatus) -> String {
    match status {
        RunStatus::Active => t!("interop.status.active"),
        RunStatus::Completed => t!("interop.status.completed"),
        RunStatus::Failed => t!("interop.status.failed"),
    }
}

// ============================================================================
// SECTION: MCP Client Helpers
// ============================================================================

/// Resolved auth settings for MCP client requests.
struct ResolvedAuth {
    /// Optional bearer token header value.
    bearer_token: Option<String>,
    /// Optional client subject header value.
    client_subject: Option<String>,
}

/// Builds an MCP client from CLI arguments.
fn build_mcp_client(args: &McpClientArgs) -> CliResult<McpClient> {
    let auth = resolve_auth(args)?;
    let mut stdio_env = parse_stdio_env(&args.stdio_env)?;
    if let Some(path) = &args.stdio_config {
        stdio_env.push(stdio_config_env(path));
    }
    let config = McpClientConfig {
        transport: args.transport.into(),
        endpoint: args.endpoint.clone(),
        stdio_command: args.stdio_command.clone(),
        stdio_args: args.stdio_args.clone(),
        stdio_env,
        timeout: Duration::from_millis(args.timeout_ms),
        bearer_token: auth.bearer_token,
        client_subject: auth.client_subject,
    };
    McpClient::new(config).map_err(|err| CliError::new(t!("mcp.client.config_failed", error = err)))
}

/// Reads MCP tool input JSON from flags or files.
fn read_mcp_tool_input(args: &McpToolInputArgs) -> CliResult<Value> {
    if let Some(json) = &args.json {
        return serde_json::from_str(json)
            .map_err(|err| CliError::new(t!("mcp.client.input_parse_failed", error = err)));
    }
    if let Some(path) = &args.input {
        let bytes = read_bytes_with_limit(path, MAX_MCP_INPUT_BYTES).map_err(|err| match err {
            ReadLimitError::Io(err) => CliError::new(t!(
                "mcp.client.input_read_failed",
                path = path.display(),
                error = err
            )),
            ReadLimitError::TooLarge {
                size,
                limit,
            } => CliError::new(t!(
                "input.read_too_large",
                kind = "mcp tool input",
                path = path.display(),
                size = size,
                limit = limit
            )),
        })?;
        return serde_json::from_slice(&bytes)
            .map_err(|err| CliError::new(t!("mcp.client.input_parse_failed", error = err)));
    }
    Ok(serde_json::json!({}))
}

/// Validates MCP tool input against the canonical JSON schema.
fn validate_mcp_tool_input(tool: decision_gate_core::ToolName, input: &Value) -> CliResult<()> {
    let mut validator = tool_schema_validator()?;
    validator.validate(tool, input)
}

/// Parses stdio environment variables from CLI inputs.
fn parse_stdio_env(values: &[String]) -> CliResult<Vec<(String, String)>> {
    let mut env = Vec::new();
    for entry in values {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| CliError::new(t!("mcp.client.invalid_stdio_env", value = entry)))?;
        env.push((key.to_string(), value.to_string()));
    }
    Ok(env)
}

/// Parses a tenant identifier from CLI inputs.
fn parse_tenant_id(value: u64) -> CliResult<TenantId> {
    TenantId::from_raw(value)
        .ok_or_else(|| CliError::new(t!("schema.invalid_id", field = "tenant_id", value = value)))
}

/// Parses a namespace identifier from CLI inputs.
fn parse_namespace_id(value: u64) -> CliResult<NamespaceId> {
    NamespaceId::from_raw(value).ok_or_else(|| {
        CliError::new(t!("schema.invalid_id", field = "namespace_id", value = value))
    })
}

/// Resolves bearer token and client subject headers for MCP client requests.
fn resolve_auth(args: &McpClientArgs) -> CliResult<ResolvedAuth> {
    if args.auth_profile.is_none() {
        return Ok(ResolvedAuth {
            bearer_token: args.bearer_token.clone(),
            client_subject: args.client_subject.clone(),
        });
    }

    let profile_name = args.auth_profile.as_deref().unwrap_or_default();
    let config_path = resolve_auth_config_path(args.auth_config.as_deref());
    let profiles = load_auth_profiles(&config_path)?;
    let profile = profiles.get(profile_name).ok_or_else(|| {
        CliError::new(t!("mcp.client.auth_profile_missing", profile = profile_name))
    })?;

    Ok(ResolvedAuth {
        bearer_token: args.bearer_token.clone().or_else(|| profile.bearer_token.clone()),
        client_subject: args.client_subject.clone().or_else(|| profile.client_subject.clone()),
    })
}

/// Resolves the config path for auth profile loading.
fn resolve_auth_config_path(path: Option<&Path>) -> PathBuf {
    if let Some(path) = path {
        return path.to_path_buf();
    }
    if let Ok(env_path) = std::env::var("DECISION_GATE_CONFIG") {
        return PathBuf::from(env_path);
    }
    PathBuf::from("decision-gate.toml")
}

/// Auth profile configuration parsed from TOML.
#[derive(Debug, Clone, Deserialize)]
struct AuthProfileConfig {
    /// Optional bearer token value.
    bearer_token: Option<String>,
    /// Optional client subject header value.
    client_subject: Option<String>,
}

/// CLI config container parsed from TOML.
#[derive(Debug, Clone, Deserialize)]
struct CliConfig {
    /// Optional client configuration section.
    client: Option<CliClientConfig>,
}

/// Client configuration parsed from TOML.
#[derive(Debug, Clone, Deserialize)]
struct CliClientConfig {
    /// Optional named auth profiles.
    auth_profiles: Option<BTreeMap<String, AuthProfileConfig>>,
}

/// Loads auth profiles from a config file.
fn load_auth_profiles(path: &Path) -> CliResult<BTreeMap<String, AuthProfileConfig>> {
    let bytes = fs::read(path).map_err(|err| {
        CliError::new(t!("mcp.client.auth_config_read_failed", path = path.display(), error = err))
    })?;
    if bytes.len() > MAX_AUTH_CONFIG_BYTES {
        return Err(CliError::new(t!("mcp.client.auth_config_too_large", path = path.display())));
    }
    let content = std::str::from_utf8(&bytes)
        .map_err(|err| CliError::new(t!("mcp.client.auth_config_parse_failed", error = err)))?;
    let parsed: CliConfig = toml::from_str(content)
        .map_err(|err| CliError::new(t!("mcp.client.auth_config_parse_failed", error = err)))?;
    Ok(parsed.client.and_then(|client| client.auth_profiles).unwrap_or_default())
}

/// Tool schema validator used by MCP client commands.
struct ToolSchemaValidator {
    /// JSON schema registry for tool inputs.
    registry: Registry,
    /// Tool contracts keyed by tool name.
    contracts: BTreeMap<decision_gate_core::ToolName, ToolContract>,
    /// Cached validators keyed by tool name.
    validators: BTreeMap<decision_gate_core::ToolName, Validator>,
}

impl ToolSchemaValidator {
    /// Builds a tool schema validator from canonical contracts.
    fn new() -> CliResult<Self> {
        let scenario_schema = decision_gate_contract::schemas::scenario_schema();
        let id = scenario_schema
            .get("$id")
            .and_then(Value::as_str)
            .ok_or_else(|| CliError::new(t!("mcp.client.schema_registry_missing")))?;
        let registry =
            Registry::try_new(id, Draft::Draft202012.create_resource(scenario_schema.clone()))
                .map_err(|err| {
                    CliError::new(t!("mcp.client.schema_registry_failed", error = err))
                })?;
        let mut contracts = BTreeMap::new();
        for contract in tool_contracts() {
            contracts.insert(contract.name, contract);
        }
        Ok(Self {
            registry,
            contracts,
            validators: BTreeMap::new(),
        })
    }

    /// Validates an input payload against the tool schema.
    fn validate(&mut self, tool: decision_gate_core::ToolName, input: &Value) -> CliResult<()> {
        let contract = self.contracts.get(&tool).ok_or_else(|| {
            CliError::new(t!("mcp.client.schema_unknown_tool", tool = tool.as_str()))
        })?;
        let validator = if let Some(existing) = self.validators.get(&tool) {
            existing
        } else {
            let compiled = compile_schema(&contract.input_schema, &self.registry)?;
            self.validators.insert(tool, compiled);
            self.validators.get(&tool).ok_or_else(|| {
                CliError::new(t!("mcp.client.schema_compile_failed", error = "validator missing"))
            })?
        };
        if !validator.is_valid(input) {
            let mut errors = validator.iter_errors(input);
            let message = errors
                .next()
                .map_or_else(|| "schema validation failed".to_string(), |err| err.to_string());
            return Err(CliError::new(t!(
                "mcp.client.schema_validation_failed",
                tool = tool.as_str(),
                error = message
            )));
        }
        Ok(())
    }
}

/// Returns the shared tool schema validator instance.
fn tool_schema_validator() -> CliResult<std::sync::MutexGuard<'static, ToolSchemaValidator>> {
    static VALIDATOR: OnceLock<std::sync::Mutex<ToolSchemaValidator>> = OnceLock::new();
    let mutex = if let Some(mutex) = VALIDATOR.get() {
        mutex
    } else {
        let validator = ToolSchemaValidator::new()?;
        let _ = VALIDATOR.set(std::sync::Mutex::new(validator));
        VALIDATOR.get().ok_or_else(|| {
            CliError::new(t!("mcp.client.schema_compile_failed", error = "validator missing"))
        })?
    };
    mutex.lock().map_err(|_| CliError::new(t!("mcp.client.schema_lock_failed")))
}

/// Compiles a JSON schema validator with the shared registry.
fn compile_schema(schema: &Value, registry: &Registry) -> CliResult<Validator> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(registry.clone())
        .build(schema)
        .map_err(|err| CliError::new(t!("mcp.client.schema_compile_failed", error = err)))
}

/// Resolves the CLI locale from flags or environment.
fn resolve_locale(lang: Option<LangArg>, env_lang: Option<&str>) -> CliResult<Locale> {
    if let Some(lang) = lang {
        return Ok(lang.into());
    }
    if let Some(value) = env_lang {
        return Locale::parse(value).ok_or_else(|| {
            CliError::new(t!("i18n.lang.invalid_env", env = LANG_ENV, value = value))
        });
    }
    Ok(Locale::En)
}

// ============================================================================
// SECTION: Authoring Helpers
// ============================================================================

/// Resolves an authoring format from flags or file extension.
fn resolve_authoring_format(
    path: &Path,
    format: Option<AuthoringFormatArg>,
) -> CliResult<AuthoringFormat> {
    if let Some(format) = format {
        return Ok(format.into());
    }
    authoring::detect_format(path)
        .ok_or_else(|| CliError::new(t!("authoring.format.missing", path = path.display())))
}

/// Reads authoring input from disk.
fn read_authoring_input(path: &Path) -> CliResult<String> {
    let bytes = read_bytes_with_limit(path, MAX_AUTHORING_BYTES).map_err(|err| match err {
        ReadLimitError::Io(err) => {
            CliError::new(t!("authoring.read_failed", path = path.display(), error = err))
        }
        ReadLimitError::TooLarge {
            size,
            limit,
        } => CliError::new(t!(
            "input.read_too_large",
            kind = t!("authoring.kind.input"),
            path = path.display(),
            size = size,
            limit = limit
        )),
    })?;
    String::from_utf8(bytes).map_err(|err| {
        CliError::new(t!("authoring.read_failed", path = path.display(), error = err))
    })
}

/// Normalizes `ScenarioSpec` authoring input and maps errors to CLI messages.
fn normalize_authoring_input(
    path: &Path,
    format: Option<AuthoringFormatArg>,
) -> CliResult<decision_gate_contract::NormalizedScenario> {
    let input = read_authoring_input(path)?;
    let format = resolve_authoring_format(path, format)?;
    authoring::normalize_scenario(&input, format).map_err(|err| map_authoring_error(err, path))
}

/// Maps authoring errors into localized CLI messages.
fn map_authoring_error(error: AuthoringError, path: &Path) -> CliError {
    let message = match error {
        AuthoringError::Parse {
            format,
            error,
        } => {
            t!("authoring.parse_failed", format = format, path = path.display(), error = error)
        }
        AuthoringError::Schema {
            error,
        } => {
            t!("authoring.schema_failed", path = path.display(), error = error)
        }
        AuthoringError::Deserialize {
            error,
        } => {
            t!("authoring.deserialize_failed", path = path.display(), error = error)
        }
        AuthoringError::Spec {
            error,
        } => {
            t!("authoring.spec_failed", path = path.display(), error = error)
        }
        AuthoringError::Canonicalization {
            error,
        } => {
            t!("authoring.canonicalize_failed", path = path.display(), error = error)
        }
    };
    CliError::new(message)
}

/// Formats a hash digest for CLI output.
fn format_hash_digest(digest: &decision_gate_core::HashDigest) -> String {
    let algorithm = match digest.algorithm {
        HashAlgorithm::Sha256 => "sha256",
    };
    format!("{algorithm}:{}", digest.value)
}

/// Converts CLI format selection to authoring formats.
impl From<AuthoringFormatArg> for AuthoringFormat {
    fn from(value: AuthoringFormatArg) -> Self {
        match value {
            AuthoringFormatArg::Json => Self::Json,
            AuthoringFormatArg::Ron => Self::Ron,
        }
    }
}

/// Converts CLI language selections into locales.
impl From<LangArg> for Locale {
    fn from(value: LangArg) -> Self {
        match value {
            LangArg::En => Self::En,
            LangArg::Ca => Self::Ca,
        }
    }
}

/// Converts CLI transport selections into interop transport variants.
impl From<McpTransportArg> for InteropTransport {
    fn from(value: McpTransportArg) -> Self {
        match value {
            McpTransportArg::Http => Self::Http,
            McpTransportArg::Sse => Self::Sse,
            McpTransportArg::Stdio => Self::Stdio,
        }
    }
}

/// Converts CLI transport selections into MCP transport variants.
impl From<McpTransportArg> for McpTransport {
    fn from(value: McpTransportArg) -> Self {
        match value {
            McpTransportArg::Http => Self::Http,
            McpTransportArg::Sse => Self::Sse,
            McpTransportArg::Stdio => Self::Stdio,
        }
    }
}

/// Converts CLI tool selections into canonical tool names.
impl From<McpToolNameArg> for decision_gate_core::ToolName {
    fn from(value: McpToolNameArg) -> Self {
        match value {
            McpToolNameArg::ScenarioDefine => Self::ScenarioDefine,
            McpToolNameArg::ScenarioStart => Self::ScenarioStart,
            McpToolNameArg::ScenarioStatus => Self::ScenarioStatus,
            McpToolNameArg::ScenarioNext => Self::ScenarioNext,
            McpToolNameArg::ScenarioSubmit => Self::ScenarioSubmit,
            McpToolNameArg::ScenarioTrigger => Self::ScenarioTrigger,
            McpToolNameArg::EvidenceQuery => Self::EvidenceQuery,
            McpToolNameArg::RunpackExport => Self::RunpackExport,
            McpToolNameArg::RunpackVerify => Self::RunpackVerify,
            McpToolNameArg::ProvidersList => Self::ProvidersList,
            McpToolNameArg::ProviderContractGet => Self::ProviderContractGet,
            McpToolNameArg::ProviderCheckSchemaGet => Self::ProviderCheckSchemaGet,
            McpToolNameArg::SchemasRegister => Self::SchemasRegister,
            McpToolNameArg::SchemasList => Self::SchemasList,
            McpToolNameArg::SchemasGet => Self::SchemasGet,
            McpToolNameArg::ScenariosList => Self::ScenariosList,
            McpToolNameArg::Precheck => Self::Precheck,
            McpToolNameArg::DecisionGateDocsSearch => Self::DecisionGateDocsSearch,
        }
    }
}

// ============================================================================
// SECTION: Output Helpers
// ============================================================================

/// Writes a single line to stdout.
fn write_stdout_line(message: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    writeln!(&mut stdout, "{message}")
}

/// Writes raw bytes to stdout without adding a newline.
fn write_stdout_bytes(bytes: &[u8]) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(bytes)
}

/// Writes canonical JSON to stdout with a size limit.
fn write_canonical_json<T: Serialize>(value: &T, max_bytes: usize) -> CliResult<()> {
    let mut bytes = canonical_json_bytes_with_limit(value, max_bytes).map_err(|err| {
        let message = match err {
            HashError::Canonicalization(error) => {
                t!("provider.discovery.serialize_failed", error = error)
            }
            HashError::SizeLimitExceeded {
                limit,
                actual,
            } => t!(
                "provider.discovery.serialize_failed",
                error = format!("response exceeds size limit ({actual} > {limit})")
            ),
        };
        CliError::new(message)
    })?;
    bytes.push(b'\n');
    write_stdout_bytes(&bytes).map_err(|err| CliError::new(output_error("stdout", &err)))
}

/// Writes a canonical JSON value to stdout.
fn write_json_value(value: &Value) -> CliResult<()> {
    let mut bytes = serde_jcs::to_vec(value)
        .map_err(|err| CliError::new(t!("mcp.client.json_failed", error = err)))?;
    bytes.push(b'\n');
    write_stdout_bytes(&bytes).map_err(|err| CliError::new(output_error("stdout", &err)))
}

/// Renders provider list output in text form.
fn render_provider_list_text(
    response: &decision_gate_mcp::tools::ProvidersListResponse,
) -> CliResult<()> {
    let mut output = String::new();
    output.push_str(&t!("provider.list.header"));
    output.push('\n');
    for provider in &response.providers {
        let checks = if provider.checks.is_empty() {
            t!("provider.list.checks.none")
        } else {
            provider.checks.join(", ")
        };
        output.push_str(&t!(
            "provider.list.entry",
            provider = provider.provider_id.as_str(),
            transport = format!("{:?}", provider.transport).to_lowercase(),
            checks = checks
        ));
        output.push('\n');
    }
    write_stdout_bytes(output.as_bytes()).map_err(|err| CliError::new(output_error("stdout", &err)))
}

/// Writes a single line to stderr.
fn write_stderr_line(message: &str) -> std::io::Result<()> {
    let mut stderr = std::io::stderr();
    writeln!(&mut stderr, "{message}")
}

/// Formats a localized output error message.
fn output_error(stream: &str, error: &std::io::Error) -> String {
    let stream_label = match stream {
        "stdout" => t!("output.stream.stdout"),
        "stderr" => t!("output.stream.stderr"),
        _ => t!("output.stream.unknown"),
    };
    t!("output.write_failed", stream = stream_label, error = error)
}

/// Emits an error message to stderr and returns a failure exit code.
fn emit_error(message: &str) -> ExitCode {
    let _ = write_stderr_line(message);
    ExitCode::FAILURE
}
