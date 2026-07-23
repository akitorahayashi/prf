use std::io::{self, Write};

use clap::builder::PossibleValuesParser;
use clap::{Parser, Subcommand};

use crate::app;
use crate::cleanup::Scope;
use crate::error::AppError;
use crate::targets::registry;

pub mod run;
pub mod scan;

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
    /// Delete files discovered by a scan.
    #[command(visible_alias = "rn")]
    Run(run::RunArgs),
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
            let scope = Scope::from_environment(&args.paths, args.current)?;
            let targets = args.resolve_targets(scope.mode())?;
            let options = app::scan::ScanOptions { targets, scope, verbose: args.verbose };
            if args.list {
                app::scan::list_targets(options)?;
            } else {
                app::scan::execute(options)?;
            }
        }
        Commands::Run(args) => {
            let scope = Scope::from_environment(&args.paths, args.current)?;
            let interactive = args.interactive();
            let targets = args.resolve_targets(scope.mode())?;
            let options = app::run::RunOptions {
                targets,
                interactive,
                scope,
                verbose: args.verbose,
                assume_yes: args.yes,
            };
            app::run::execute(options)?;
        }
    }

    Ok(())
}
