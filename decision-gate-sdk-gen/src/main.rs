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
//!
//! ### Security Posture
//! Tooling inputs and output paths are treated as untrusted. IO failures and
//! validation errors fail closed. See `Docs/security/threat_model.md`.

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use clap::Parser;
use clap::Subcommand;
use decision_gate_sdk_gen::DEFAULT_TOOLING_PATH;
use decision_gate_sdk_gen::SdkGenError;
use decision_gate_sdk_gen::SdkGenerator;

// ============================================================================
// SECTION: CLI Types
// ============================================================================

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

// ============================================================================
// SECTION: Command Dispatch
// ============================================================================

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
/// Parent directories are created automatically when missing. Outputs are
/// written to a temporary file and then moved into place.
fn generate(
    tooling: PathBuf,
    python_out: &Path,
    typescript_out: &Path,
    openapi_out: &Path,
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
    python_out: &Path,
    typescript_out: &Path,
    openapi_out: &Path,
) -> Result<(), SdkGenError> {
    let generator = SdkGenerator::load(tooling)?;
    check_output(python_out, &generator.generate_python()?)?;
    check_output(typescript_out, &generator.generate_typescript()?)?;
    check_output(openapi_out, &generator.generate_openapi()?)?;
    Ok(())
}

/// Writes the generated contents to the specified path.
///
/// On platforms without atomic replace, this falls back to remove-and-rename.
fn write_output(path: &Path, contents: &str) -> Result<(), SdkGenError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| SdkGenError::Io(err.to_string()))?;
    }
    let (temp_path, mut file) = create_temp_output(path)?;
    if let Err(err) = file.write_all(contents.as_bytes()) {
        let _ = fs::remove_file(&temp_path);
        return Err(SdkGenError::Io(err.to_string()));
    }
    if let Err(err) = file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(SdkGenError::Io(err.to_string()));
    }
    persist_temp_output(&temp_path, path)
}

/// Compares the generated contents against the existing file.
///
/// This is used by CI to ensure generated outputs stay in sync.
fn check_output(path: &Path, contents: &str) -> Result<(), SdkGenError> {
    let existing = fs::read_to_string(path).map_err(|err| SdkGenError::Io(err.to_string()))?;
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

// ============================================================================
// SECTION: Output Helpers
// ============================================================================

// ============================================================================
// CONSTANTS: Temporary output file handling
// ============================================================================

const TEMP_ATTEMPTS: usize = 16;
static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Creates a unique temporary output file alongside the destination.
fn create_temp_output(path: &Path) -> Result<(PathBuf, fs::File), SdkGenError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SdkGenError::Io("output path does not include a file name".to_string()))?;
    for _ in 0 .. TEMP_ATTEMPTS {
        let attempt = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_name = format!(".{file_name}.tmp.{}.{}", std::process::id(), attempt);
        let temp_path = parent.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(SdkGenError::Io(err.to_string())),
        }
    }
    Err(SdkGenError::Io("failed to allocate temporary output path".to_string()))
}

/// Persists the temporary output file to the final destination.
fn persist_temp_output(temp_path: &Path, path: &Path) -> Result<(), SdkGenError> {
    match fs::rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            if path.exists() {
                fs::remove_file(path).map_err(|err| SdkGenError::Io(err.to_string()))?;
                fs::rename(temp_path, path).map_err(|err| SdkGenError::Io(err.to_string()))?;
                return Ok(());
            }
            let _ = fs::remove_file(temp_path);
            Err(SdkGenError::Io(err.to_string()))
        }
    }
}
