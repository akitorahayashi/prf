use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::error::AppError;
use crate::targets::catalog;
use crate::targets::category::Category;

#[derive(Args)]
pub struct RunArgs {
    #[arg(short = 't', long = "type", value_name = "CATEGORY", value_enum, action = ArgAction::Append, conflicts_with = "all")]
    pub categories: Vec<Category>,

    #[arg(long = "all", action = ArgAction::SetTrue, help = "Scan all supported categories (respects --current)")]
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
    pub fn resolve_categories(&self) -> Result<catalog::CategorySelection, AppError> {
        catalog::resolve(&self.categories, self.all, self.current)
    }

    pub fn interactive(&self) -> bool {
        !self.all && self.categories.is_empty()
    }
}
