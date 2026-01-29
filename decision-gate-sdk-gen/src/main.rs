// decision-gate-sdk-gen/src/main.rs
// ============================================================================
// Module: SDK Generator CLI
// Description: CLI entrypoint for SDK + OpenAPI generation.
// Purpose: Produce deterministic client SDK artifacts from tooling.json.
// Dependencies: clap, decision-gate-sdk-gen
// ============================================================================

//! ## Overview
//! The SDK generator CLI renders Python/TypeScript SDK artifacts and the
//! `OpenAPI` JSON view. It can also verify that on-disk outputs match the
//! generated content.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use clap::Subcommand;
use decision_gate_sdk_gen::DEFAULT_TOOLING_PATH;
use decision_gate_sdk_gen::SdkGenError;
use decision_gate_sdk_gen::SdkGenerator;

/// CLI arguments for SDK generation.
#[derive(Debug, Parser)]
#[command(name = "decision-gate-sdk-gen", about = "Generate Decision Gate client SDK artifacts.")]
struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    command: Command,
}

/// Supported CLI subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Generate SDK artifacts.
    Generate {
        /// Path to tooling.json input.
        #[arg(long, value_name = "FILE", default_value = DEFAULT_TOOLING_PATH)]
        tooling: PathBuf,
        /// Python SDK output file.
        #[arg(
            long,
            value_name = "FILE",
            default_value = "sdks/python/decision_gate/_generated.py"
        )]
        python_out: PathBuf,
        /// TypeScript SDK output file.
        #[arg(long, value_name = "FILE", default_value = "sdks/typescript/src/_generated.ts")]
        typescript_out: PathBuf,
        /// `OpenAPI` output file.
        #[arg(
            long,
            value_name = "FILE",
            default_value = "Docs/generated/openapi/decision-gate.json"
        )]
        openapi_out: PathBuf,
    },
    /// Verify SDK artifacts match the generated output.
    Check {
        /// Path to tooling.json input.
        #[arg(long, value_name = "FILE", default_value = DEFAULT_TOOLING_PATH)]
        tooling: PathBuf,
        /// Python SDK output file.
        #[arg(
            long,
            value_name = "FILE",
            default_value = "sdks/python/decision_gate/_generated.py"
        )]
        python_out: PathBuf,
        /// TypeScript SDK output file.
        #[arg(long, value_name = "FILE", default_value = "sdks/typescript/src/_generated.ts")]
        typescript_out: PathBuf,
        /// `OpenAPI` output file.
        #[arg(
            long,
            value_name = "FILE",
            default_value = "Docs/generated/openapi/decision-gate.json"
        )]
        openapi_out: PathBuf,
    },
}

/// CLI entrypoint.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => report_error(&err),
    }
}

/// Dispatches the CLI command.
fn run() -> Result<(), SdkGenError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Generate {
            tooling,
            python_out,
            typescript_out,
            openapi_out,
        } => generate(tooling, &python_out, &typescript_out, &openapi_out),
        Command::Check {
            tooling,
            python_out,
            typescript_out,
            openapi_out,
        } => check(tooling, &python_out, &typescript_out, &openapi_out),
    }
}

/// Writes SDK outputs to the configured paths.
///
/// Parent directories are created automatically when missing.
fn generate(
    tooling: PathBuf,
    python_out: &PathBuf,
    typescript_out: &PathBuf,
    openapi_out: &PathBuf,
) -> Result<(), SdkGenError> {
    let generator = SdkGenerator::load(tooling)?;
    let python = generator.generate_python()?;
    let typescript = generator.generate_typescript()?;
    let openapi = generator.generate_openapi()?;
    write_output(python_out, &python)?;
    write_output(typescript_out, &typescript)?;
    write_output(openapi_out, &openapi)?;
    Ok(())
}

/// Verifies SDK outputs match the generated content.
///
/// Returns a tooling error when drift is detected.
fn check(
    tooling: PathBuf,
    python_out: &PathBuf,
    typescript_out: &PathBuf,
    openapi_out: &PathBuf,
) -> Result<(), SdkGenError> {
    let generator = SdkGenerator::load(tooling)?;
    check_output(python_out, &generator.generate_python()?)?;
    check_output(typescript_out, &generator.generate_typescript()?)?;
    check_output(openapi_out, &generator.generate_openapi()?)?;
    Ok(())
}

/// Writes the generated contents to the specified path.
///
/// The file is overwritten atomically by the OS when possible.
fn write_output(path: &PathBuf, contents: &str) -> Result<(), SdkGenError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| SdkGenError::Io(err.to_string()))?;
    }
    std::fs::write(path, contents).map_err(|err| SdkGenError::Io(err.to_string()))
}

/// Compares the generated contents against the existing file.
///
/// This is used by CI to ensure generated outputs stay in sync.
fn check_output(path: &PathBuf, contents: &str) -> Result<(), SdkGenError> {
    let existing = std::fs::read_to_string(path).map_err(|err| SdkGenError::Io(err.to_string()))?;
    if existing != contents {
        return Err(SdkGenError::Tooling(format!(
            "SDK drift detected for {}. Run decision-gate-sdk-gen generate.",
            path.display()
        )));
    }
    Ok(())
}

/// Reports a CLI error to stderr.
fn report_error(err: &SdkGenError) -> ExitCode {
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "{err}");
    ExitCode::FAILURE
}
