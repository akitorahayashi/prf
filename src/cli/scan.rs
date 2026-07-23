use std::path::PathBuf;

use clap::{ArgAction, Args};

use crate::error::AppError;
use crate::targets::catalog;
use crate::targets::category::Category;

#[derive(Args)]
pub struct ScanArgs {
    #[arg(short = 't', long = "type", value_name = "CATEGORY", value_enum, action = ArgAction::Append, conflicts_with = "all", help = "Scan only the selected category; repeatable")]
    pub categories: Vec<Category>,

    #[arg(long = "all", action = ArgAction::SetTrue, help = "Explicitly scan every category available in this mode")]
    pub all: bool,

    #[arg(short, long, action = ArgAction::SetTrue, help = "Show candidate paths and diagnostics")]
    pub verbose: bool,

    #[arg(long = "list", action = ArgAction::SetTrue, help = "List discovered target types without measuring size")]
    pub list: bool,

    #[arg(short = 'c', long = "current", action = ArgAction::SetTrue, conflicts_with = "paths", help = "Scan only the current directory and exclude user or external targets")]
    pub current: bool,

    #[arg(value_name = "PATH", num_args = 0.., help = "Local root to scan; defaults to $HOME/Desktop")]
    pub paths: Vec<PathBuf>,
}

impl ScanArgs {
    pub fn resolve_categories(&self) -> Result<catalog::CategorySelection, AppError> {
        catalog::resolve(&self.categories, self.all, self.current)
    }
}
