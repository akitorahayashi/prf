use clap::{ArgAction, Args};

use super::scope::ScopeArgs;
use super::target::TargetArgs;

#[derive(Args)]
pub struct CleanArgs {
    #[command(flatten)]
    pub targets: TargetArgs,

    #[command(flatten)]
    pub scope: ScopeArgs,

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
}
