use clap::{ArgAction, Args};

use crate::cleanup::{ScopeMode, Target};
use crate::error::AppError;
use crate::targets::registry;

use super::target_value_parser;

#[derive(Args)]
pub struct TargetArgs {
    #[arg(
        value_name = "TARGET",
        num_args = 0..,
        conflicts_with = "all",
        value_parser = target_value_parser(),
        ignore_case = true,
        help = "Select one or more cleanup targets"
    )]
    targets: Vec<String>,

    #[arg(
        long = "all",
        action = ArgAction::SetTrue,
        help = "Select every target eligible for the resolved scope"
    )]
    all: bool,
}

pub enum TargetRequest {
    Omitted,
    Named(Vec<String>),
    All,
}

impl TargetArgs {
    pub fn into_request(self) -> TargetRequest {
        if self.all {
            TargetRequest::All
        } else if self.targets.is_empty() {
            TargetRequest::Omitted
        } else {
            TargetRequest::Named(self.targets)
        }
    }
}

impl TargetRequest {
    pub const fn is_omitted(&self) -> bool {
        matches!(self, Self::Omitted)
    }

    pub fn resolve(&self, mode: ScopeMode) -> Result<Vec<&'static Target>, AppError> {
        match self {
            Self::Omitted | Self::All => registry::eligible(mode),
            Self::Named(names) => registry::resolve(names, mode),
        }
    }
}
