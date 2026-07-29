//! Library entry point for the prf CLI.

mod app;
mod cleanup;
mod cli;
mod error;
mod footprint;
mod fs;
mod output;
mod targets;

pub fn execute() {
    cli::execute();
}
