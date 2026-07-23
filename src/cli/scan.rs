use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::error::AppError;
use crate::targets::catalog;
use crate::targets::category::Category;

#[derive(Args)]
pub struct ScanArgs {
    #[arg(short = 't', long = "type", value_name = "CATEGORY", value_enum, action = ArgAction::Append, conflicts_with = "all")]
    pub categories: Vec<Category>,

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
    pub fn resolve_categories(&self) -> Result<catalog::CategorySelection, AppError> {
        catalog::resolve(&self.categories, self.all, self.current)
    }
}
