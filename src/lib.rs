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
