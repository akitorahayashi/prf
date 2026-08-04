use clap::{ArgAction, Args};

use crate::cleanup::Scope;
use crate::error::AppError;

#[derive(Args)]
pub struct ScopeArgs {
    #[arg(
        short = 'c',
        long = "current",
        action = ArgAction::SetTrue,
        help = "Use only the current directory; disable home discovery, Brew, and Docker"
    )]
    current: bool,
}

impl ScopeArgs {
    pub fn resolve(self) -> Result<Scope, AppError> {
        Scope::from_environment(self.current)
    }
}
