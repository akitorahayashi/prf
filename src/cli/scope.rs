use clap::{ArgAction, Args};

use crate::cleanup::InspectionInputs;
use crate::error::AppError;

#[derive(Args)]
pub struct ScopeArgs {
    #[arg(
        short = 'c',
        long = "current",
        action = ArgAction::SetTrue,
        help = "Use only the current directory; disable home discovery and default-only targets"
    )]
    current: bool,
}

impl ScopeArgs {
    pub fn resolve(self) -> Result<InspectionInputs, AppError> {
        InspectionInputs::from_environment(self.current)
    }
}
