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
// SECTION: Imports
// ============================================================================

use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use clap::ArgAction;
use clap::Args;
use clap::CommandFactory;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use decision_gate_cli::t;
use decision_gate_contract::AuthoringError;
use decision_gate_contract::AuthoringFormat;
use decision_gate_contract::authoring;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::RunState;
use decision_gate_core::RunpackManifest;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES;
use decision_gate_core::runtime::RunpackBuilder;
use decision_gate_core::runtime::RunpackVerifier;
use decision_gate_core::runtime::VerificationReport;
use decision_gate_core::runtime::VerificationStatus;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::FileArtifactReader;
use decision_gate_mcp::FileArtifactSink;
use decision_gate_mcp::McpServer;
use decision_gate_mcp::config::ServerTransport;
use serde::de::DeserializeOwned;
use thiserror::Error;

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
}

/// Configuration for the `serve` command.
#[derive(Args, Debug)]
struct ServeCommand {
    /// Optional config file path (defaults to decision-gate.toml or env override).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
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
    }
}

// ============================================================================
// SECTION: Serve Command
// ============================================================================

/// Executes the `serve` command.
async fn command_serve(command: ServeCommand) -> CliResult<ExitCode> {
    let config = DecisionGateConfig::load(command.config.as_deref())
        .map_err(|err| CliError::new(t!("serve.config.load_failed", error = err)))?;
    enforce_local_only(&config)?;
    warn_local_only(&config)?;

    let server = McpServer::from_config(config)
        .map_err(|err| CliError::new(t!("serve.init_failed", error = err)))?;
    server.serve().await.map_err(|err: decision_gate_mcp::server::McpServerError| {
        CliError::new(t!("serve.failed", error = err))
    })?;

    Ok(ExitCode::SUCCESS)
}

/// Enforces local-only transport restrictions for the MCP server.
fn enforce_local_only(config: &DecisionGateConfig) -> CliResult<()> {
    match config.server.transport {
        ServerTransport::Stdio => Ok(()),
        ServerTransport::Http | ServerTransport::Sse => {
            let bind = config.server.bind.as_deref().unwrap_or_default();
            let addr: SocketAddr = bind.parse().map_err(|err: std::net::AddrParseError| {
                CliError::new(t!("serve.bind.parse_failed", bind = bind, error = err))
            })?;
            if !addr.ip().is_loopback() {
                return Err(CliError::new(t!("serve.bind.non_loopback", bind = bind)));
            }
            Ok(())
        }
    }
}

/// Emits local-only warnings for the MCP server.
fn warn_local_only(config: &DecisionGateConfig) -> CliResult<()> {
    write_stderr_line(&t!("serve.warn.local_only"))
        .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    if config.server.transport != ServerTransport::Stdio {
        write_stderr_line(&t!("serve.warn.transport_local_only"))
            .map_err(|err| CliError::new(output_error("stderr", &err)))?;
    }
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
    let metadata = fs::metadata(path).map_err(ReadLimitError::Io)?;
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
    fs::read(path).map_err(ReadLimitError::Io)
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

#[cfg(test)]
mod tests {
    //! Test-only lint relaxations for panic-based assertions and debug output.
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
}
