use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::cleanup::Target;
use crate::error::AppError;
use crate::targets::registry;

#[derive(Args)]
pub struct RunArgs {
    #[arg(short = 't', long = "type", value_name = "TARGET", action = ArgAction::Append, conflicts_with = "all")]
    pub targets: Vec<String>,

    #[arg(long = "all", action = ArgAction::SetTrue, help = "Scan all supported targets (respects --current)")]
    pub all: bool,

    #[arg(short = 'y', long = "yes", action = ArgAction::SetTrue)]
    pub yes: bool,

    #[arg(short, long, action = ArgAction::SetTrue)]
    pub verbose: bool,

    #[arg(short = 'c', long = "current", action = ArgAction::SetTrue, conflicts_with = "paths", help = "Limit cleanup to current directory only (skips Brew, Docker)")]
    pub current: bool,

    #[arg(value_name = "PATH", num_args = 0..)]
    pub paths: Vec<PathBuf>,
}

impl RunArgs {
    pub fn resolve_targets(&self) -> Result<Vec<&'static Target>, AppError> {
        registry::resolve(&self.targets, self.all, self.current)
    }

    pub fn interactive(&self) -> bool {
        !self.all && self.targets.is_empty()
    }
}
