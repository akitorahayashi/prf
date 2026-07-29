use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::cleanup::{ScopeMode, Target};
use crate::error::AppError;
use crate::targets::registry;

use super::target_value_parser;

#[derive(Args)]
pub struct ScanArgs {
    #[arg(
        short = 't',
        long = "type",
        value_name = "TARGET",
        action = ArgAction::Append,
        conflicts_with = "all",
        value_parser = target_value_parser(),
        ignore_case = true,
        help = "Scan only this target; repeat to select more targets"
    )]
    pub targets: Vec<String>,

    #[arg(
        long = "all",
        action = ArgAction::SetTrue,
        help = "Scan every target eligible for the resolved scope"
    )]
    pub all: bool,

    #[arg(
        short,
        long,
        action = ArgAction::SetTrue,
        help = "Show every discovered cleanup action and its estimate"
    )]
    pub verbose: bool,

    #[arg(
        long = "list",
        action = ArgAction::SetTrue,
        help = "List cleanup locations without measuring their footprints"
    )]
    pub list: bool,

    #[arg(
        short = 'c',
        long = "current",
        action = ArgAction::SetTrue,
        conflicts_with = "paths",
        help = "Scan only the current directory; disable home discovery, Brew, and Docker"
    )]
    pub current: bool,

    #[arg(
        value_name = "PATH",
        num_args = 0..,
        help = "Replace the default ~/Desktop scan root; home discovery remains enabled"
    )]
    pub paths: Vec<PathBuf>,
}

impl ScanArgs {
    pub fn resolve_targets(&self, mode: ScopeMode) -> Result<Vec<&'static Target>, AppError> {
        registry::resolve(&self.targets, self.all, mode)
    }
}
