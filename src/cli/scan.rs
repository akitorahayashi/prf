use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::cleanup::{ScopeMode, Target};
use crate::error::AppError;
use crate::targets::registry;

#[derive(Args)]
pub struct ScanArgs {
    #[arg(short = 't', long = "type", value_name = "TARGET", action = ArgAction::Append, conflicts_with = "all")]
    pub targets: Vec<String>,

    #[arg(long = "all", action = ArgAction::SetTrue)]
    pub all: bool,

    #[arg(short, long, action = ArgAction::SetTrue)]
    pub verbose: bool,

    #[arg(long = "list", action = ArgAction::SetTrue)]
    pub list: bool,

    #[arg(short = 'c', long = "current", action = ArgAction::SetTrue, conflicts_with = "paths")]
    pub current: bool,

    #[arg(value_name = "PATH", num_args = 0..)]
    pub paths: Vec<PathBuf>,
}

impl ScanArgs {
    pub fn resolve_targets(&self, mode: ScopeMode) -> Result<Vec<&'static Target>, AppError> {
        registry::resolve(&self.targets, self.all, mode)
    }
}
