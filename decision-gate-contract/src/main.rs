// decision-gate-contract/src/main.rs
// ============================================================================
// Module: Contract CLI
// Description: CLI entrypoint for generating Decision Gate contract artifacts.
// Purpose: Provide deterministic artifact generation for docs and tooling.
// Dependencies: clap, decision-gate-contract
// ============================================================================

//! ## Overview
//! The contract CLI emits deterministic, human-readable contract artifacts and
//! can verify that on-disk artifacts match the generated bundle.
//! Security posture: inputs are local CLI flags; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use clap::Subcommand;
use decision_gate_config as config;
use decision_gate_contract::ContractBuilder;
use decision_gate_contract::ContractError;

// ============================================================================
// SECTION: CLI Definition
// ============================================================================

/// Contract generator CLI arguments.
#[derive(Debug, Parser)]
#[command(name = "decision-gate-contract", about = "Generate Decision Gate contract artifacts.")]
struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    command: Command,
}

/// Supported CLI subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Generate the contract artifacts.
    Generate {
        /// Output directory for generated artifacts.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
    },
    /// Verify generated artifacts match the canonical contract.
    Check {
        /// Output directory containing generated artifacts.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
    },
}

// ============================================================================
// SECTION: CLI Execution
// ============================================================================

/// CLI entrypoint.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => report_error(&err),
    }
}

/// Executes the CLI command.
fn run() -> Result<(), ContractError> {
    let cli = Cli::parse();
    let output_dir = cli.output_dir().unwrap_or_else(ContractBuilder::default_output_dir);
    let builder = ContractBuilder::new(output_dir.clone());
    match cli.command {
        Command::Generate {
            ..
        } => {
            builder.write_to(&output_dir)?;
            config::write_config_docs(None)
                .map_err(|err| ContractError::Generation(err.to_string()))?;
            Ok(())
        }
        Command::Check {
            ..
        } => {
            builder.verify_output(&output_dir)?;
            config::verify_config_docs(None)
                .map_err(|err| ContractError::Generation(err.to_string()))?;
            Ok(())
        }
    }
}

/// Reports CLI errors to stderr and returns a failure exit code.
fn report_error(err: &ContractError) -> ExitCode {
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "{err}");
    ExitCode::FAILURE
}

// ============================================================================
// SECTION: CLI Helpers
// ============================================================================

impl Cli {
    /// Returns the resolved output directory from flags, if any.
    #[must_use]
    fn output_dir(&self) -> Option<PathBuf> {
        match &self.command {
            Command::Generate {
                out,
            }
            | Command::Check {
                out,
            } => out.clone(),
        }
    }
}
