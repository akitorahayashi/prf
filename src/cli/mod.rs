use std::io::{self, Write};

use clap::{Parser, Subcommand};

use crate::app;
use crate::error::AppError;
use crate::fs::roots::resolve_roots_with_current;

pub mod run;
pub mod scan;

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
            let targets = args.resolve_targets()?;
            let options = app::scan::ScanOptions {
                targets,
                roots: resolve_roots_with_current(&args.paths, args.current)?,
                verbose: args.verbose,
                current: args.current,
            };
            if args.list {
                app::scan::list_targets(options)?;
            } else {
                app::scan::execute(options)?;
            }
        }
        Commands::Run(args) => {
            let interactive = args.interactive();
            let targets = args.resolve_targets()?;
            let options = app::run::RunOptions {
                targets,
                interactive,
                roots: resolve_roots_with_current(&args.paths, args.current)?,
                verbose: args.verbose,
                assume_yes: args.yes,
                current: args.current,
            };
            app::run::execute(options)?;
        }
    }

    Ok(())
}
