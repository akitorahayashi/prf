use std::io::{self, Write};

use clap::builder::PossibleValuesParser;
use clap::{Parser, Subcommand};

use crate::app;
use crate::error::AppError;
use crate::targets::registry;

pub mod clean;
pub mod scan;
mod scope;
mod target;

fn target_value_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(registry::names())
}

#[derive(Parser)]
#[command(
    name = "prf",
    version,
    about = "Safely clean development caches and generated artifacts on macOS."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Perform a dry-run scan to see what can be removed.
    #[command(visible_alias = "sc")]
    Scan(scan::ScanArgs),
    /// Scan, select, and delete development caches and generated artifacts.
    #[command(visible_alias = "cln")]
    Clean(clean::CleanArgs),
}

pub fn execute() {
    if let Err(error) = try_execute() {
        if matches!(&error, AppError::Io(source) if source.kind() == io::ErrorKind::BrokenPipe) {
            return;
        }
        let write_result = writeln!(io::stderr().lock(), "Error: {error}");
        if write_result.is_err_and(|source| source.kind() != io::ErrorKind::BrokenPipe) {
            std::process::exit(1);
        }
        std::process::exit(1);
    }
}

fn try_execute() -> Result<(), AppError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan(args) => {
            let inputs = args.scope.resolve()?;
            let request = args.targets.into_request();
            let targets = request.resolve(inputs.scope().mode())?;
            let options = app::scan::ScanOptions { targets, inputs, verbose: args.verbose };
            if args.list {
                app::scan::list_targets(options)?;
            } else {
                app::scan::execute(options)?;
            }
        }
        Commands::Clean(args) => {
            let inputs = args.scope.resolve()?;
            let request = args.targets.into_request();
            let prompt_for_targets = request.is_omitted();
            let targets = request.resolve(inputs.scope().mode())?;
            let selection = if prompt_for_targets {
                app::clean::CleanSelection::PromptFrom(targets)
            } else {
                app::clean::CleanSelection::Fixed(targets)
            };
            let options = app::clean::CleanOptions {
                selection,
                inputs,
                verbose: args.verbose,
                assume_yes: args.yes,
            };
            app::clean::execute(options)?;
        }
    }

    Ok(())
}
