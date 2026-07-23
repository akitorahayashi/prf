use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::cleanup::{ScopeMode, Target};
use crate::error::AppError;
use crate::targets::registry;

use super::target_value_parser;

#[derive(Args)]
pub struct RunArgs {
    #[arg(
        short = 't',
        long = "type",
        value_name = "TARGET",
        action = ArgAction::Append,
        conflicts_with = "all",
        value_parser = target_value_parser(),
        ignore_case = true,
        help = "Clean only this target; repeat to select more and skip target selection"
    )]
    pub targets: Vec<String>,

    #[arg(
        long = "all",
        action = ArgAction::SetTrue,
        help = "Select every eligible target and skip target selection, but still confirm deletion"
    )]
    pub all: bool,

    #[arg(
        short = 'y',
        long = "yes",
        action = ArgAction::SetTrue,
        help = "Skip deletion confirmation; target selection still appears when required"
    )]
    pub yes: bool,

    #[arg(
        short,
        long,
        action = ArgAction::SetTrue,
        help = "Show every selected cleanup action and its estimate"
    )]
    pub verbose: bool,

    #[arg(
        short = 'c',
        long = "current",
        action = ArgAction::SetTrue,
        conflicts_with = "paths",
        help = "Clean only the current directory; disable home discovery, Brew, and Docker"
    )]
    pub current: bool,

    #[arg(
        value_name = "PATH",
        num_args = 0..,
        help = "Replace the default ~/Desktop scan root; home discovery remains enabled"
    )]
    pub paths: Vec<PathBuf>,
}

impl RunArgs {
    pub fn resolve_targets(&self, mode: ScopeMode) -> Result<Vec<&'static Target>, AppError> {
        registry::resolve(&self.targets, self.all, mode)
    }

    pub fn interactive(&self) -> bool {
        !self.all && self.targets.is_empty()
    }
}
