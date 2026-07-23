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

pub fn run() {
    if let Err(err) = run_inner() {
        eprintln!("Error: {}", err);
        std::process::exit(err.exit_code());
    }
}

fn run_inner() -> Result<(), AppError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan(args) => {
            let selection = args.resolve_categories()?;
            let options = app::scan::ScanOptions {
                categories: selection.categories,
                request_origin: selection.origin,
                roots: resolve_roots_with_current(&args.paths, args.current)?,
                verbose: args.verbose,
                list: args.list,
                current: args.current,
            };
            app::scan::execute(options)?;
        }
        Commands::Run(args) => {
            let interactive = args.interactive();
            let selection = args.resolve_categories()?;
            let options = app::run::RunOptions {
                categories: selection.categories,
                request_origin: selection.origin,
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
