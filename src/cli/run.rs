use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::error::AppError;
use crate::targets::catalog;
use crate::targets::category::Category;

#[derive(Args)]
pub struct RunArgs {
    #[arg(short = 't', long = "type", value_name = "CATEGORY", value_enum, action = ArgAction::Append, conflicts_with = "all", help = "Clean only the selected category; repeatable")]
    pub categories: Vec<Category>,

    #[arg(long = "all", action = ArgAction::SetTrue, help = "Explicitly clean every category available in this mode")]
    pub all: bool,

    #[arg(short = 'y', long = "yes", action = ArgAction::SetTrue, help = "Skip final confirmation without expanding cleanup scope")]
    pub yes: bool,

    #[arg(short, long, action = ArgAction::SetTrue, help = "Show candidate sizes and diagnostics")]
    pub verbose: bool,

    #[arg(short = 'c', long = "current", action = ArgAction::SetTrue, conflicts_with = "paths", help = "Clean only the current directory and exclude user or external targets")]
    pub current: bool,

    #[arg(value_name = "PATH", num_args = 0.., help = "Local root to scan; defaults to $HOME/Desktop")]
    pub paths: Vec<PathBuf>,
}

impl RunArgs {
    pub fn resolve_categories(&self) -> Result<catalog::CategorySelection, AppError> {
        catalog::resolve(&self.categories, self.all, self.current)
    }

    pub fn interactive(&self) -> bool {
        !self.all && self.categories.is_empty()
    }
}
